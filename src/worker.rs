use std::path::PathBuf;
use std::sync::mpsc;
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
    CloseVault,
}

pub enum WorkerResponse {
    VaultOpened {
        index: ContainerIndex,
    },
    FileDecryptedToTemp {
        temp_path: PathBuf,
    },
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

            while let Ok(cmd) = cmd_rx.recv() {
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
                                    let temp = std::env::temp_dir().join(format!("evfs_{}", name));
                                    match std::fs::write(&temp, &bytes) {
                                        Ok(_) => WorkerResponse::FileDecryptedToTemp {
                                            temp_path: temp,
                                        },
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
                    } => match create_container(&source_dir, &output_path, &progress_tx, &password) {
                        Ok(_) => WorkerResponse::EncryptionDone,
                        Err(e) => WorkerResponse::Error {
                            message: format!("Encryption failed: {}", e),
                        },
                    },
                    WorkerCommand::CloseVault => {
                        vault = None;
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
