use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, SystemTime};
use zeroize::Zeroizing;

use crate::deflate::{ContainerIndex, FileEntry, VaultReader, create_container};
use crate::ui::ProgressMessage;

pub enum WorkerCommand {
    OpenVault {
        path: PathBuf,
        password: Zeroizing<String>,
    },
    ReadFile {
        entry: FileEntry,
    },
    EncryptFolder {
        source_dir: PathBuf,
        output_path: PathBuf,
        password: Zeroizing<String>,
        progress_tx: mpsc::Sender<ProgressMessage>,
    },
    GarbageCollect,
    CloseVault,
}

pub enum WorkerResponse {
    VaultOpened {
        index: ContainerIndex,
    },
    FileDecryptedToTemp {
        temp_path: PathBuf,
    },
    FileUpdated {
        entry: FileEntry,
    },
    GarbageCollected,
    EncryptionDone,
    Error {
        message: String,
    },
}

pub fn spawn() -> (mpsc::Sender<WorkerCommand>, mpsc::Receiver<WorkerResponse>) {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (resp_tx, resp_rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("vault-worker".into())
        .spawn(move || {
            let mut vault: Option<VaultReader> = None;
            let mut watched_files: HashMap<PathBuf, (FileEntry, SystemTime)> = HashMap::new();
            let poll_interval = Duration::from_secs(2);

            loop {
                let cmd = match cmd_rx.recv_timeout(poll_interval) {
                    Ok(cmd) => cmd,
                    Err(RecvTimeoutError::Timeout) => {
                        let changed: Vec<_> = watched_files
                            .iter()
                            .filter_map(|(path, (entry, last_mod))| {
                                std::fs::metadata(path)
                                    .ok()
                                    .and_then(|m| m.modified().ok())
                                    .filter(|&m| m > *last_mod)
                                    .map(|m| (path.clone(), entry.clone(), m))
                            })
                            .collect();

                        for (temp_path, entry, new_mod) in changed {
                            if let Some(ref mut reader) = vault {
                                match std::fs::read(&temp_path) {
                                    Ok(new_data) => {
                                        match reader.update_entry(&entry, &new_data) {
                                            Ok(new_entry) => {
                                                watched_files
                                                    .insert(temp_path, (entry, new_mod));
                                                let _ = resp_tx
                                                    .send(WorkerResponse::FileUpdated {
                                                        entry: new_entry,
                                                    });
                                            }
                                            Err(e) => {
                                                let _ = resp_tx.send(WorkerResponse::Error {
                                                    message: format!(
                                                        "Failed to update file: {}",
                                                        e
                                                    ),
                                                });
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = resp_tx.send(WorkerResponse::Error {
                                            message: format!(
                                                "Failed to read updated temp file: {}",
                                                e
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                };

                let result = match cmd {
                    WorkerCommand::OpenVault { path, password } => {
                        match VaultReader::open(&path, &password) {
                            Ok(reader) => {
                                let index = reader.index.clone();
                                vault = Some(reader);
                                WorkerResponse::VaultOpened { index }
                            }
                            Err(e) => WorkerResponse::Error {
                                message: format!("Failed to open vault: {}", e),
                            },
                        }
                    }
                    WorkerCommand::ReadFile { entry } => {
                        let name = std::path::Path::new(&entry.path)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "unknown".to_string());

                        match vault.as_mut() {
                            Some(reader) => match reader.read_file_content(&entry) {
                                Ok(bytes) => {
                                    let temp =
                                        std::env::temp_dir().join(format!("evfs_{}", name));
                                    match std::fs::write(&temp, &bytes) {
                                        Ok(_) => {
                                            if let Ok(meta) = std::fs::metadata(&temp) {
                                                if let Ok(modified) = meta.modified() {
                                                    watched_files.insert(
                                                        temp.clone(),
                                                        (entry, modified),
                                                    );
                                                }
                                            }
                                            WorkerResponse::FileDecryptedToTemp { temp_path: temp }
                                        }
                                        Err(e) => WorkerResponse::Error {
                                            message: format!("Failed to write temp file: {}", e),
                                        },
                                    }
                                }
                                Err(e) => WorkerResponse::Error {
                                    message: format!("Failed to decrypt file: {}", e),
                                },
                            },
                            None => WorkerResponse::Error {
                                message: "No vault is open".into(),
                            },
                        }
                    }
                    WorkerCommand::EncryptFolder {
                        source_dir,
                        output_path,
                        password,
                        progress_tx,
                    } => {
                        match create_container(&source_dir, &output_path, &progress_tx, &password)
                        {
                            Ok(_) => WorkerResponse::EncryptionDone,
                            Err(e) => WorkerResponse::Error {
                                message: format!("Encryption failed: {}", e),
                            },
                        }
                    }
                    WorkerCommand::GarbageCollect => {
                        match vault.as_mut() {
                            Some(reader) => match reader.garbage_collect() {
                                Ok(true) => WorkerResponse::GarbageCollected,
                                Ok(false) => WorkerResponse::Error {
                                    message: "Garbage is below 10% threshold, no collection needed".into(),
                                },
                                Err(e) => WorkerResponse::Error {
                                    message: format!("Garbage collection failed: {}", e),
                                },
                            },
                            None => WorkerResponse::Error {
                                message: "No vault is open".into(),
                            },
                        }
                    }
                    WorkerCommand::CloseVault => {
                        if let Some(ref mut reader) = vault {
                            let _ = reader.garbage_collect();
                        }
                        vault = None;
                        watched_files.clear();
                        continue;
                    }
                };

                if resp_tx.send(result).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn vault worker thread");

    (cmd_tx, resp_rx)
}
