use std::path::PathBuf;
use zeroize::Zeroizing;

use crate::deflate::FileEntry;

#[derive(Default, PartialEq)]
pub enum AppState {
    #[default]
    Home,
    Browser,
    Loading,
}

pub enum ProgressMessage {
    Progress {
        current: usize,
        total: usize,
        message: String,
    },
}

pub enum DialogMessage {
    FolderPicked(PathBuf),
    SaveLocationPicked(PathBuf),
    OpenLocationPicked(PathBuf),
}

pub enum BrowserEvent {
    NavigateTo(String),
    SelectFile(FileEntry),
    OpenFile(FileEntry),
    DeleteFile(FileEntry),
    ContextMenu(FileEntry),
    RenameCancel,
    RenameSubmit {
        old_path: String,
        new_path: String,
    },
    CloseVault,
}

pub enum HomeEvent {
    EncryptFolder,
    OpenVault,
}

pub enum PasswordEvent {
    Submitted(Zeroizing<String>),
    Cancelled,
}

pub enum AppEvent {
    Browser(BrowserEvent),
    Home(HomeEvent),
    Password(PasswordEvent),
}
