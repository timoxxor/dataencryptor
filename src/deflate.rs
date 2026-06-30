use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::crypto::{self, ChunkDecryptReader, ChunkEncryptWriter, HkdfContext};
use crate::ui::ProgressMessage;

pub const MAGIC: &[u8; 4] = b"EVFS";
pub const FORMAT_VERSION: u8 = 2;
pub const MIN_COMPRESS_SIZE: u64 = 256;

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub magic: [u8; 4],
    pub version: u8,
    pub salt: [u8; 16],
    pub index_offset: u64,
    pub index_size: u64,
}

impl Header {
    pub fn new(salt: &[u8; 16]) -> Self {
        Header {
            magic: *MAGIC,
            version: FORMAT_VERSION,
            salt: *salt,
            index_offset: 0,
            index_size: 0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileEntry {
    pub id: [u8; 16],
    pub path: String,
    pub offset: u64,
    pub stored_size: u64,
    pub original_size: u64,
    pub compressed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerIndex {
    pub entries: Vec<FileEntry>,
}

pub fn compress<W: Write>(data: &[u8], writer: W) -> io::Result<W> {
    let mut encoder = DeflateEncoder::new(writer, Compression::new(3));
    encoder.write_all(data)?;

    encoder
        .finish()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

pub fn decompress(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut decompressed = Vec::new();
    DeflateDecoder::new(data).read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn should_compress(path: &Path, file_size: u64) -> bool {
    if file_size < MIN_COMPRESS_SIZE {
        return false;
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some(
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "ico" | "heic"
            | "avif",
        ) => false,
        Some("mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v") => false,
        Some("mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" | "m4a") => false,
        Some("zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst") => false,
        Some("pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx") => false,
        _ => true,
    }
}

struct FileWork {
    data: Vec<u8>,
    id: [u8; 16],
    path: String,
    original_size: u64,
    compressed: bool,
}

const MAX_POOL_BUFFER: usize = 64 * 1024 * 1024;

struct BufferPool {
    inner: Mutex<Vec<Vec<u8>>>,
}

impl BufferPool {
    fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
        }
    }

    fn acquire(&self, capacity: usize) -> Vec<u8> {
        let mut inner = self.inner.lock().unwrap();
        inner
            .pop()
            .map(|mut v| {
                v.clear();
                if v.capacity() < capacity {
                    v.reserve(capacity - v.capacity());
                }
                v
            })
            .unwrap_or_else(|| Vec::with_capacity(capacity))
    }

    fn release(&self, buf: Vec<u8>) {
        if buf.capacity() > MAX_POOL_BUFFER {
            return;
        }
        let mut inner = self.inner.lock().unwrap();
        inner.push(buf);
    }
}

pub struct VaultReader {
    pub file: BufReader<File>,
    pub index: ContainerIndex,
    pub hkdf: HkdfContext,
}

impl VaultReader {
    pub fn open(pack_path: &Path, password: &str) -> io::Result<Self> {
        let raw = File::open(pack_path)?;
        let mut file = BufReader::new(raw);

        let header: Header = bincode::deserialize_from(&mut file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if &header.magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Magic mismatch"));
        }

        let salt = header.salt;

        let master_key = crypto::argon2id(password.as_bytes(), salt)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Argon2id kdf failed"))?;
        let hkdf = HkdfContext::new(&salt, &master_key);

        let index_key = hkdf
            .derive(b"index_key")
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "hkdf derive failed"))?;

        file.seek(SeekFrom::Start(header.index_offset))?;
        let mut encrypted_index = vec![0u8; header.index_size as usize];
        file.read_exact(&mut encrypted_index)?;

        let cursor = std::io::Cursor::new(encrypted_index);
        let mut reader = ChunkDecryptReader::new(cursor, &index_key, None)?;
        let mut decrypted_index = Vec::new();
        reader.read_to_end(&mut decrypted_index)?;

        let index_bytes = decompress(&decrypted_index)?;
        let index: ContainerIndex = bincode::deserialize(&index_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Self { file, index, hkdf })
    }

    pub fn extract_file<W: Write>(&mut self, entry: &FileEntry, mut writer: W) -> io::Result<()> {
        let file_key = self
            .hkdf
            .derive(&entry.id)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "File HKDF derive failed"))?;

        self.file.seek(SeekFrom::Start(entry.offset))?;

        let limited = (&mut self.file).take(entry.stored_size);
        let mut reader = ChunkDecryptReader::new(limited, &file_key, Some(entry.id))?;

        if entry.compressed {
            let mut decoder = DeflateDecoder::new(reader);
            io::copy(&mut decoder, &mut writer)?;
        } else {
            io::copy(&mut reader, &mut writer)?;
        }

        Ok(())
    }

    pub fn read_file_content(&mut self, entry: &FileEntry) -> io::Result<Vec<u8>> {
        let mut data = Vec::with_capacity(entry.original_size as usize);
        self.extract_file(entry, &mut data)?;
        Ok(data)
    }
}

pub fn create_container(
    source_dir: &Path,
    output_pack: &Path,
    tx: &std::sync::mpsc::Sender<ProgressMessage>,
    password: &str,
) -> io::Result<()> {
    let salt = crypto::rand_salt();

    let master_key = crypto::argon2id(password.as_bytes(), salt)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Argon2id kdf failed"))?;
    let hkdf = HkdfContext::new(&salt, &master_key);

    let walker: Vec<_> = WalkDir::new(source_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    let total_files = walker.len();

    let _ = tx.send(ProgressMessage::Progress {
        current: 0,
        total: total_files,
        message: "Processing files...".into(),
    });

    let (work_tx, work_rx) = sync_channel::<FileWork>(total_files.min(8).max(1));
    let pool = Arc::new(BufferPool::new());
    let counter = Arc::new(AtomicUsize::new(0));

    let writer_thread = {
        let output_pack = output_pack.to_path_buf();
        let tx = tx.clone();
        let pool = Arc::clone(&pool);
        std::thread::Builder::new()
            .name("vault-writer".into())
            .spawn(move || -> io::Result<(BufWriter<File>, Vec<FileEntry>)> {
                let raw = File::create(&output_pack)?;
                let mut pack_file = BufWriter::new(raw);

                let header_bytes = bincode::serialize(&Header::new(&salt))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                pack_file.write_all(&header_bytes)?;

                let mut entries = Vec::with_capacity(total_files);

                while let Ok(work) = work_rx.recv() {
                    let offset = pack_file.stream_position()?;
                    pack_file.write_all(&work.data)?;
                    let stored_size = work.data.len() as u64;

                    entries.push(FileEntry {
                        id: work.id,
                        path: work.path,
                        offset,
                        stored_size,
                        original_size: work.original_size,
                        compressed: work.compressed,
                    });

                    let _ = tx.send(ProgressMessage::Progress {
                        current: entries.len(),
                        total: total_files,
                        message: format!("Writing file: {}", entries.last().unwrap().path),
                    });

                    pool.release(work.data);
                }

                Ok((pack_file, entries))
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
    };

    walker.par_iter().try_for_each(|entry| -> io::Result<()> {
        let full_path = entry.path();
        let relative_path = full_path
            .strip_prefix(source_dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let file_size = entry.metadata()?.len();
        let uuid = Uuid::new_v4().to_bytes_le();
        let compressed = should_compress(full_path, file_size);

        let file_key = hkdf
            .derive(&uuid)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "hkdf derive failed"))?;

        let _ = counter.fetch_add(1, Ordering::Relaxed) + 1;

        let inner = pool.acquire(file_size as usize);
        let ew = ChunkEncryptWriter::new(inner, &file_key, Some(uuid))?;

        let mut file = File::open(full_path)?;
        let data = if compressed {
            let mut encoder = DeflateEncoder::new(ew, Compression::new(3));
            io::copy(&mut file, &mut encoder)?;
            encoder
                .finish()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .finish()?
        } else {
            let mut ew = ew;
            io::copy(&mut file, &mut ew)?;
            ew.finish()?
        };

        let _ = work_tx.send(FileWork {
            data,
            id: uuid,
            path: relative_path,
            original_size: file_size,
            compressed,
        });

        Ok(())
    })?;

    drop(work_tx);

    let (mut pack_file, entries) = writer_thread
        .join()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "writer thread panicked"))??;

    let index_offset = pack_file.stream_position()?;
    let index_data = ContainerIndex { entries };
    let index_bytes =
        bincode::serialize(&index_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    drop(index_data);

    let compressed_index = compress(&index_bytes, Vec::new())?;

    let index_key = hkdf
        .derive(b"index_key")
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "HKDF index key derivation failed"))?;

    let mut ew = ChunkEncryptWriter::new(Vec::new(), &index_key, None)?;
    ew.write_all(&compressed_index)?;
    let encrypted_index = ew.finish()?;

    pack_file.write_all(&encrypted_index)?;
    let index_size = encrypted_index.len() as u64;

    let final_header = Header {
        magic: *MAGIC,
        version: FORMAT_VERSION,
        salt,
        index_offset,
        index_size,
    };
    let final_header_bytes =
        bincode::serialize(&final_header).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    pack_file.seek(SeekFrom::Start(0))?;
    pack_file.write_all(&final_header_bytes)?;
    pack_file.flush()?;

    println!("Контейнер успешно создан: {:?}", output_pack);
    Ok(())
}
