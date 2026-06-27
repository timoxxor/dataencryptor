use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use image::EncodableLayout;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;
//use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::crypto::{self, Aes, hkdf_derive};
use crate::gui;

pub const MAGIC: &[u8; 4] = b"EVFS";
pub const FORMAT_VERSION: u8 = 1;
pub const MIN_COMPRESS_SIZE: u64 = 256;

#[derive(Default)]
pub enum CompressionRequirements {
    Required,
    #[default]
    None,
}

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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContainerIndex {
    pub entries: Vec<FileEntry>,
}

pub fn compress<W: Write>(data: &[u8], writer: W) -> io::Result<W> {
    let mut encoder = DeflateEncoder::new(writer, Compression::default());
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

pub fn write_compressed_to_file<W: Write + Seek>(
    data: &[u8],
    uuid: [u8; 16],
    salt: &[u8; 16],
    ikm: &[u8],
    c_req: CompressionRequirements,
    writer: &mut W,
) -> io::Result<u64> {
    let start_pos = writer.stream_position()?;
    let output = match c_req {
        CompressionRequirements::Required => compress(data, Vec::new())?,
        CompressionRequirements::None => data.to_vec(),
    };

    let key = crypto::hkdf_derive(Some(salt), &uuid, ikm)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "hkdf derive failed"))?;
    let alg = Aes::new(key.to_vec(), None);
    let encrypted = alg
        .encrypt(&output)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "aes encryption failed"))?;

    writer.write_all(&encrypted)?;
    let end_pos = writer.stream_position()?;
    Ok(end_pos - start_pos)
}

pub struct VaultReader {
    pub file: File,
    pub index: ContainerIndex,
    pub master_key: Vec<u8>,
    pub salt: [u8; 16]
}

impl VaultReader {
    pub fn open(pack_path: &Path, mut password: String) -> io::Result<Self> {
        let mut file = File::open(pack_path)?;

        let header: Header = bincode::deserialize_from(&mut file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if &header.magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Magic mismatch"));
        }

        let salt = header.salt;

        file.seek(SeekFrom::Start(header.index_offset))?;
        let mut encrypted_index = vec![0u8; header.index_size as usize];
        file.read_exact(&mut encrypted_index)?;

        let master_key = crypto::argon2id(password.as_bytes(), salt)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Argon2id kdf failed"))?;
        password.zeroize();

        let key = crypto::hkdf_derive(Some(&salt), b"index_key", &master_key)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "hkdf derive failed"))?;
        let alg = Aes::new(key.to_vec(), None);
        let decrypted_index = alg
            .decrypt(&encrypted_index)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "aes decryption failed"))?;

        let index_bytes = decompress(&decrypted_index)?;
        let index: ContainerIndex = bincode::deserialize(&index_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // let mut offset_map = HashMap::new();
        // for (i, entry) in index.entries.iter().enumerate() {
        //     offset_map.insert(entry.offset, i);
        // }

        Ok(Self {
            file,
            index,
            master_key, 
            salt
            //offset_map,
        })
    }

    pub fn read_file_content(&mut self, entry: &FileEntry) -> io::Result<Vec<u8>> {
        // 1. Читаем зашифрованные (и возможно сжатые) данные из файла контейнера
        self.file.seek(SeekFrom::Start(entry.offset))?;
        let mut encrypted_data = vec![0u8; entry.stored_size as usize];
        self.file.read_exact(&mut encrypted_data)?;

        // 2. Деривация уникального ключа для конкретного файла (точно так же, как при записи)
        // Используем master_key.as_bytes() или прямо &*self.master_key в зависимости от вашего крипто-модуля
        let file_key = crypto::hkdf_derive(
            Some(&self.salt),
            &entry.id,
            self.master_key.as_bytes(), // или как вы передавали в write_compressed_to_file
        )
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "File HKDF derive failed"))?;

        // 3. Расшифровка AES
        let alg = Aes::new(file_key.to_vec(), None);
        let decrypted_data = alg
            .decrypt(&encrypted_data)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "File AES decryption failed"))?;

        if decrypted_data.len() as u64 == entry.original_size {
            Ok(decrypted_data)
        } else {
            decompress(&decrypted_data)
        }
    }

    pub fn open_file_in_system(&mut self, entry: &FileEntry) -> io::Result<()> {
        let name = Path::new(&entry.path)
            .file_name()
            .unwrap()
            .to_string_lossy();
        let temp = std::env::temp_dir().join(format!("evfs_{}", name));

        let bytes = self.read_file_content(entry)?;
        fs::write(&temp, bytes)?;

        opener::open(&temp).map_err(io::Error::other)?;
        Ok(())
    }
}

pub fn create_container(
    source_dir: &Path,
    output_pack: &Path,
    tx: &std::sync::mpsc::Sender<gui::ProgressMessage>,
    mut password: String,
) -> io::Result<()> {
    let salt = crypto::rand_salt();

    let master_key = crypto::argon2id(password.as_bytes(), salt)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Argon2id kdf failed"))?;
    password.zeroize();

    let mut pack_file = File::create(output_pack)?;

    let header_bytes = bincode::serialize(&Header::new(&salt))
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    pack_file.write_all(&header_bytes)?;

    let mut entries = Vec::new();
    let walker: Vec<_> = WalkDir::new(source_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    let total_files = walker.len();

    for (i, entry) in walker.iter().enumerate() {
        let relative_path = entry
            .path()
            .strip_prefix(source_dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let mut raw_data = Vec::new();
        File::open(entry.path())?.read_to_end(&mut raw_data)?;

        let original_size = raw_data.len() as u64;
        let offset = pack_file.stream_position()?;
        let size;
        let uuid = Uuid::new_v4().to_bytes_le();

        let c_req = (original_size >= MIN_COMPRESS_SIZE)
            .then_some(CompressionRequirements::Required)
            .unwrap_or_default();

        size = write_compressed_to_file(
            &raw_data,
            uuid,
            &salt,
            master_key.as_bytes(),
            c_req,
            &mut pack_file,
        )?;

        entries.push(FileEntry {
            id: uuid,
            path: relative_path,
            offset,
            stored_size: size,
            original_size,
        });

        let _ = tx.send(gui::ProgressMessage::Progress {
            current: i + 1,
            total: total_files,
            message: format!("Processing file: {}", entry.file_name().to_string_lossy()),
        });
    }

    let index_offset = pack_file.stream_position()?;
    let index_data = ContainerIndex { entries };
    let index_bytes =
        bincode::serialize(&index_data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let compressed_index = compress(&index_bytes, Vec::new())?;

    let index_key = hkdf_derive(Some(&salt), b"index_key", &master_key)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "HKDF index key derivation failed"))?;

    let alg = Aes::new(index_key.to_vec(), None);
    let encrypted_index = alg
        .encrypt(&compressed_index)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "aes encryption failed"))?;

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

    println!("Контейнер успешно создан: {:?}", output_pack);
    Ok(())
}
