use argon2::Argon2;
use ring::aead::{AES_256_GCM, Aad, BoundKey, NONCE_LEN, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use ring::error::Unspecified;
use ring::hkdf::{self, HKDF_SHA256, Salt};
use ring::rand::{SecureRandom, SystemRandom};

struct CounterNonceSequence(u64);

impl NonceSequence for CounterNonceSequence {
    // called once for each seal operation
    fn advance(&mut self) -> Result<Nonce, Unspecified> {
        let mut nonce_bytes = vec![0; NONCE_LEN];

        let bytes = self.0.to_be_bytes();
        nonce_bytes[4..].copy_from_slice(&bytes);

        self.0 += 1;
        match Nonce::try_assume_unique_for_key(&nonce_bytes) {
            Ok(nonce) => Ok(nonce),
            Err(e) => Err(e),
        }
    }
}

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

pub fn hkdf_derive(
    salt: Option<&[u8]>, // ставить None для случайного
    info: &[u8], // индекс файла или какая угодно о нём информация, будь то даже имя
    ikm: &[u8] // ключ из которого дерайвить
) -> Result<[u8; 32], Unspecified> { // работает только под HKDF_SHA256
    let salt_obj = match salt {
        Some(s) => Salt::new(HKDF_SHA256, &s),
        None => {
            let mut s = [0u8; 32];
            match SystemRandom::new().fill(&mut s) {
                Ok(()) => Salt::new(HKDF_SHA256, &s),
                Err(_) => return Err(Unspecified),
            }
        }
    };
    let prk = salt_obj.extract(ikm);
    
    let mut okm: [u8; 32] = [0; 32];
    let info_slice = &[info];
    let expand = match prk.expand(info_slice, OkmLength(okm.len())) {
        Ok(exp) => exp,
        Err(e) => return Err(e),
    };
    
    match expand.fill(&mut okm) {
        Ok(()) => Ok(okm),
        Err(e) => Err(e),
    }
}

pub struct Aes {
    kb: Vec<u8>,
    aad: Option<Vec<u8>>
}

impl Aes {
    pub fn new(kb: Vec<u8>, aad: Option<Vec<u8>>) -> Self {
        Self { kb, aad }
    }

    pub fn encrypt(&self, payload: &[u8]) -> Result<Vec<u8>, Unspecified> {
        let uk = match UnboundKey::new(&AES_256_GCM, &self.kb) {
            Ok(key) => key,
            Err(e) => return Err(e),
        };
        let mut sk = SealingKey::new(uk, CounterNonceSequence(1));
        
        let ad = match &self.aad {
            Some(aad) => Aad::from(aad.as_slice()),
            None => Aad::from(&[] as &[u8])
        };
        
        let mut in_out = payload.to_vec();
        let tag = match sk.seal_in_place_separate_tag(ad, &mut in_out) {
            Ok(t) => t,
            Err(e) => return Err(e),
        };
        
        Ok([&in_out, tag.as_ref()].concat())
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, Unspecified> {
        let unbound_key = match UnboundKey::new(&AES_256_GCM, &self.kb) {
            Ok(key) => key,
            Err(e) => return Err(e),
        };

        let ad = match &self.aad {
            Some(aad) => Aad::from(aad.as_slice()),
            None => Aad::from(&[] as &[u8])
        };

        let mut opening_key = OpeningKey::new(unbound_key, CounterNonceSequence(1));

        let mut out = data.to_vec();
        match opening_key.open_in_place(ad, &mut out) {
            Ok(plaintext) => Ok(plaintext.to_vec()),
            Err(e) => Err(e),
        }
    }
}