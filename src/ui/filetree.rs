use std::collections::BTreeSet;
use std::path::Path;

use egui::{ScrollArea, Ui};

use crate::deflate::FileEntry;

#[derive(Debug, Clone)]
pub enum DirectoryTreeAction {
    None,
    SelectFolder(String),
    SelectFile(FileEntry),
    ExecuteFile(FileEntry),
}

pub fn render_directory_tree(
    ui: &mut Ui,
    current_vfs_dir: &str,
    selected_file: Option<&FileEntry>,
    entries: &[FileEntry],
) -> DirectoryTreeAction {
    let (sub_dirs, files) = categorize_entries(current_vfs_dir, entries);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut action = DirectoryTreeAction::None;

            // ---------- Папки ----------
            for dir_name in sub_dirs {
                let resp = ui.selectable_label(false, format!("📁   {}", dir_name));

                if resp.clicked() {
                    let next_dir = if current_vfs_dir.is_empty() {
                        dir_name
                    } else {
                        format!("{}/{}", current_vfs_dir, dir_name)
                    };

                    action = DirectoryTreeAction::SelectFolder(next_dir);
                }
            }

            if !files.is_empty() {
                ui.separator();
            }

            // ---------- Файлы ----------
            for file in files {
                let file_name = Path::new(&file.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                let is_selected = selected_file
                    .map(|f| f.offset == file.offset)
                    .unwrap_or(false);

                let resp = ui.selectable_label(
                    is_selected,
                    format!("📄   {}", file_name),
                );

                if resp.clicked() {
                    action = DirectoryTreeAction::SelectFile(file.clone());
                }

                if resp.double_clicked() {
                    action = DirectoryTreeAction::ExecuteFile(file.clone());
                }
            }

            action
        })
        .inner
}

fn categorize_entries(
    current_vfs_dir: &str,
    entries: &[FileEntry],
) -> (BTreeSet<String>, Vec<FileEntry>) {
    let mut sub_dirs = BTreeSet::new();
    let mut current_files = Vec::new();

    for entry in entries {
        let path = Path::new(&entry.path);

        if is_entry_in_current_dir(current_vfs_dir, path) {
            current_files.push(entry.clone());
        } else if let Some(dir) = get_subdirectory_from_path(current_vfs_dir, path) {
            sub_dirs.insert(dir);
        }
    }

    (sub_dirs, current_files)
}

fn is_entry_in_current_dir(
    current_vfs_dir: &str,
    path: &Path,
) -> bool {
    if current_vfs_dir.is_empty() {
        path.parent() == Some(Path::new(""))
    } else {
        path.starts_with(current_vfs_dir)
            && path.parent() == Some(Path::new(current_vfs_dir))
    }
}

fn get_subdirectory_from_path(
    current_vfs_dir: &str,
    path: &Path,
) -> Option<String> {
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