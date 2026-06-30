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
    FolderPicked(PathBuf),
    SaveLocationPicked(PathBuf),
    OpenLocationPicked(PathBuf),
}
