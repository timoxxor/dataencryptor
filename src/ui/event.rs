use zeroize::Zeroizing;

use crate::deflate::FileEntry;

pub enum BrowserEvent {
    NavigateTo(String),
    SelectFile(FileEntry),
    OpenFile(FileEntry),
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
