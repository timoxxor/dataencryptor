use zeroize::Zeroizing;

use crate::deflate::FileEntry;

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
