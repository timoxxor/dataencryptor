use argon2::Argon2;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey, NONCE_LEN};
use ring::error::Unspecified;
use ring::hkdf::{self, HKDF_SHA256, Salt};
use ring::rand::{SecureRandom, SystemRandom};
use std::io::{self, Read, Write};

const CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Clone, Copy)]
pub struct OkmLength(pub usize);

impl hkdf::KeyType for OkmLength {
    fn len(&self) -> usize {
        self.0
    }
}

pub fn rand_salt() -> [u8; 16] {
    let mut s = [0u8; 16];
    match SystemRandom::new().fill(&mut s) {
        Ok(()) => s,
        Err(_) => panic!("Failed to generate random salt"),
    }
}

pub fn argon2id(password: &[u8], salt: [u8; 16]) -> Result<Vec<u8>, Unspecified> {
    let mut output_key_material = [0u8; 32];
    let _ = Argon2::default().hash_password_into(password, &salt, &mut output_key_material);

    Ok(output_key_material.to_vec())
}

pub struct HkdfContext {
    prk: hkdf::Prk,
}

impl HkdfContext {
    pub fn new(salt: &[u8], ikm: &[u8]) -> Self {
        let salt_obj = Salt::new(HKDF_SHA256, salt);
        let prk = salt_obj.extract(ikm);
        Self { prk }
    }

    pub fn derive(&self, info: &[u8]) -> Result<[u8; 32], Unspecified> {
        let mut okm = [0u8; 32];
        let info_slice = &[info];
        let expand = self.prk.expand(info_slice, OkmLength(okm.len()))?;
        expand.fill(&mut okm)?;
        Ok(okm)
    }
}

fn build_nonce(prefix: &[u8; 8], counter: u32) -> Nonce {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes[..8].copy_from_slice(prefix);
    nonce_bytes[8..].copy_from_slice(&counter.to_be_bytes());
    Nonce::try_assume_unique_for_key(&nonce_bytes).unwrap()
}

fn build_aad(aad_prefix: Option<[u8; 16]>, chunk_index: u32) -> Aad<Vec<u8>> {
    let mut aad = Vec::with_capacity(20);
    if let Some(prefix) = aad_prefix {
        aad.extend_from_slice(&prefix);
    }
    aad.extend_from_slice(&chunk_index.to_be_bytes());
    Aad::from(aad)
}

pub struct ChunkEncryptWriter<W: Write> {
    inner: W,
    key: LessSafeKey,
    nonce_prefix: [u8; 8],
    chunk_counter: u32,
    buf: Vec<u8>,
    aad_prefix: Option<[u8; 16]>,
}

impl<W: Write> ChunkEncryptWriter<W> {
    pub fn new(
        mut inner: W,
        key: &[u8; 32],
        aad_prefix: Option<[u8; 16]>,
    ) -> io::Result<Self> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid aes key"))?;
        let key = LessSafeKey::new(unbound_key);

        let mut nonce_prefix = [0u8; 8];
        SystemRandom::new()
            .fill(&mut nonce_prefix)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "rng failed"))?;

        inner.write_all(&nonce_prefix)?;

        Ok(Self {
            inner: inner,
            key,
            nonce_prefix,
            chunk_counter: 0,
            buf: Vec::with_capacity(CHUNK_SIZE),
            aad_prefix,
        })
    }

    fn encrypt_and_write_chunk(&mut self, mut chunk: Vec<u8>) -> io::Result<()> {
        let nonce = build_nonce(&self.nonce_prefix, self.chunk_counter);
        let aad = build_aad(self.aad_prefix, self.chunk_counter);
        self.chunk_counter += 1;

        self.key
            .seal_in_place_append_tag(nonce, aad, &mut chunk)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "encrypt chunk failed"))?;

        let inner = self.inner.by_ref();
        inner.write_all(&(chunk.len() as u32).to_le_bytes())?;
        inner.write_all(&chunk)
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for ChunkEncryptWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        while self.buf.len() >= CHUNK_SIZE {
            let tail = self.buf.split_off(CHUNK_SIZE);
            let chunk = std::mem::take(&mut self.buf);
            self.buf = tail;
            self.encrypt_and_write_chunk(chunk)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buf.is_empty() {
            let chunk = std::mem::take(&mut self.buf);
            self.encrypt_and_write_chunk(chunk)?;
        }
        self.inner.flush()
    }
}

pub struct ChunkDecryptReader<R: Read> {
    inner: R,
    key: LessSafeKey,
    nonce_prefix: [u8; 8],
    chunk_counter: u32,
    buf: Vec<u8>,
    pos: usize,
    aad_prefix: Option<[u8; 16]>,
    exhausted: bool,
}

impl<R: Read> ChunkDecryptReader<R> {
    pub fn new(
        mut inner: R,
        key: &[u8; 32],
        aad_prefix: Option<[u8; 16]>,
    ) -> io::Result<Self> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid aes key"))?;
        let key = LessSafeKey::new(unbound_key);

        let mut nonce_prefix = [0u8; 8];
        inner.read_exact(&mut nonce_prefix)?;

        Ok(Self {
            inner,
            key,
            nonce_prefix,
            chunk_counter: 0,
            buf: Vec::new(),
            pos: 0,
            aad_prefix,
            exhausted: false,
        })
    }

    fn refill(&mut self) -> io::Result<()> {
        if self.exhausted {
            return Ok(());
        }

        let mut len_buf = [0u8; 4];
        match self.inner.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                self.exhausted = true;
                return Ok(());
            }
            Err(e) => return Err(e),
        }
        let chunk_len = u32::from_le_bytes(len_buf) as usize;

        let mut encrypted = vec![0u8; chunk_len];
        self.inner.read_exact(&mut encrypted)?;

        let nonce = build_nonce(&self.nonce_prefix, self.chunk_counter);
        let aad = build_aad(self.aad_prefix, self.chunk_counter);
        self.chunk_counter += 1;

        let pt_len = {
            let plaintext = self
                .key
                .open_in_place(nonce, aad, &mut encrypted)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "decrypt chunk failed"))?;
            plaintext.len()
        };
        encrypted.truncate(pt_len);
        self.buf = encrypted;
        self.pos = 0;
        Ok(())
    }
}

impl<R: Read> Read for ChunkDecryptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.buf.len() {
            self.refill()?;
            if self.buf.is_empty() {
                return Ok(0);
            }
        }

        let available = self.buf.len() - self.pos;
        let to_copy = std::cmp::min(buf.len(), available);
        buf[..to_copy].copy_from_slice(&self.buf[self.pos..self.pos + to_copy]);
        self.pos += to_copy;
        Ok(to_copy)
    }
}
