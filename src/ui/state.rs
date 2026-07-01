use std::path::PathBuf;

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
