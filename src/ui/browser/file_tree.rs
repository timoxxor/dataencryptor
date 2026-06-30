use std::collections::BTreeSet;
use std::path::Path;

use egui::{ScrollArea, Ui};

use crate::deflate::FileEntry;

use super::file_row::{FileRow, RenameAction};
use super::folder_row::FolderRow;

#[derive(Debug, Clone)]
pub enum DirectoryTreeAction {
    None,
    SelectFolder(String),
    SelectFile(FileEntry),
    ExecuteFile(FileEntry),
    ContextMenu(FileEntry),
    RenameCancel,
    RenameSubmit {
        old_path: String,
        new_path: String,
    },
}

pub struct FileTree<'a> {
    entries: &'a [FileEntry],
    directories: &'a [String],
    current_vfs_dir: &'a str,
    selected_file: Option<&'a FileEntry>,
    directories_first: bool,
    show_icons: bool,
    allow_double_click: bool,
    rename_path: Option<&'a str>,
    rename_buffer: &'a mut String,
}

impl<'a> FileTree<'a> {
    pub fn new(
        entries: &'a [FileEntry],
        directories: &'a [String],
        current_vfs_dir: &'a str,
        rename_buffer: &'a mut String,
    ) -> Self {
        Self {
            entries,
            directories,
            current_vfs_dir,
            selected_file: None,
            directories_first: true,
            show_icons: true,
            allow_double_click: true,
            rename_path: None,
            rename_buffer,
        }
    }

    pub fn selected(mut self, file: Option<&'a FileEntry>) -> Self {
        self.selected_file = file;
        self
    }

    pub fn directories_first(mut self, val: bool) -> Self {
        self.directories_first = val;
        self
    }

    pub fn show_icons(mut self, val: bool) -> Self {
        self.show_icons = val;
        self
    }

    pub fn allow_double_click(mut self, val: bool) -> Self {
        self.allow_double_click = val;
        self
    }

    pub fn renaming(mut self, path: Option<&'a str>) -> Self {
        self.rename_path = path;
        self
    }

    pub fn show(self, ui: &mut Ui) -> DirectoryTreeAction {
        let (mut sub_dirs, files) = categorize_entries(self.current_vfs_dir, self.entries);
        for dir in self.directories {
            if let Some(child) = get_child_dir(self.current_vfs_dir, dir) {
                sub_dirs.insert(child);
            }
        }

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut action = DirectoryTreeAction::None;

                for dir_name in sub_dirs {
                    let next_dir = if self.current_vfs_dir.is_empty() {
                        dir_name.clone()
                    } else {
                        format!("{}/{}", self.current_vfs_dir, dir_name)
                    };
                    let current = next_dir == self.current_vfs_dir;

                    let resp = FolderRow::new(&dir_name)
                        .selected(current)
                        .show_icon(self.show_icons)
                        .show(ui);

                    if resp.clicked() {
                        action = DirectoryTreeAction::SelectFolder(next_dir);
                    }
                }

                if !files.is_empty() {
                    ui.separator();
                }

                for file in files {
                    let file_name = Path::new(&file.path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();

                    let is_selected = self
                        .selected_file
                        .map(|f| f.offset == file.offset)
                        .unwrap_or(false);

                    let is_renaming = self
                        .rename_path
                        .map(|p| p == file.path.as_str())
                        .unwrap_or(false);

                    let (resp, rename_result) = FileRow::new(&file_name, self.rename_buffer)
                        .selected(is_selected)
                        .show_icon(self.show_icons)
                        .renaming(is_renaming)
                        .show(ui);

                    if let Some(rename_action) = rename_result {
                        match rename_action {
                            RenameAction::Cancel => {
                                action = DirectoryTreeAction::RenameCancel;
                            }
                            RenameAction::Submit(new_name) => {
                                if new_name != file_name.as_ref() {
                                    let parent_dir = Path::new(&file.path)
                                        .parent()
                                        .map(|p| p.to_string_lossy().into_owned())
                                        .unwrap_or_default();
                                    let new_path = if parent_dir.is_empty() {
                                        new_name
                                    } else {
                                        format!("{}/{}", parent_dir, new_name)
                                    };
                                    action = DirectoryTreeAction::RenameSubmit {
                                        old_path: file.path.clone(),
                                        new_path,
                                    };
                                }
                            }
                        }
                    } else if resp.clicked() {
                        action = DirectoryTreeAction::SelectFile(file.clone());
                    } else if self.allow_double_click && resp.double_clicked() {
                        action = DirectoryTreeAction::ExecuteFile(file.clone());
                    } else if resp.secondary_clicked() {
                        action = DirectoryTreeAction::ContextMenu(file.clone());
                    }
                }

                action
            })
            .inner
    }
}

fn categorize_entries<'a>(
    current_vfs_dir: &str,
    entries: &'a [FileEntry],
) -> (BTreeSet<String>, Vec<&'a FileEntry>) {
    let mut sub_dirs = BTreeSet::new();
    let mut current_files = Vec::new();

    for entry in entries {
        let path = Path::new(&entry.path);

        if is_entry_in_current_dir(current_vfs_dir, path) {
            current_files.push(entry);
        } else if let Some(dir) = get_subdirectory_from_path(current_vfs_dir, path) {
            sub_dirs.insert(dir);
        }
    }

    (sub_dirs, current_files)
}

fn get_child_dir(current_vfs_dir: &str, dir: &str) -> Option<String> {
    if current_vfs_dir.is_empty() {
        dir.split('/').next().map(|s| s.to_owned())
    } else {
        let prefix = format!("{}/", current_vfs_dir);
        if dir.starts_with(&prefix) {
            dir.strip_prefix(&prefix)
                .and_then(|rest| rest.split('/').next())
                .map(|s| s.to_owned())
        } else if dir == current_vfs_dir {
            None
        } else {
            None
        }
    }
}

fn is_entry_in_current_dir(current_vfs_dir: &str, path: &Path) -> bool {
    if current_vfs_dir.is_empty() {
        path.parent() == Some(Path::new(""))
    } else {
        path.starts_with(current_vfs_dir)
            && path.parent() == Some(Path::new(current_vfs_dir))
    }
}

fn get_subdirectory_from_path(current_vfs_dir: &str, path: &Path) -> Option<String> {
    if current_vfs_dir.is_empty() {
        if path.components().count() > 1 {
            return path
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
        }
        return None;
    }

    let prefix = format!("{}/", current_vfs_dir);

    if path.starts_with(&prefix) {
        if let Ok(rel) = path.strip_prefix(current_vfs_dir) {
            let mut comps = rel.components();

            if let Some(first) = comps.next() {
                if comps.next().is_some() {
                    return Some(first.as_os_str().to_string_lossy().into_owned());
                }
            }
        }
    }

    None
}
