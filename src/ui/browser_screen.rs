use egui::{Frame, Ui};

use crate::deflate::FileEntry;

use super::browser::{
    BreadcrumbAction, BreadcrumbBar, DirectoryTreeAction, FileDetailsAction, FileDetailsView,
    FileTree,
};
use super::event::BrowserEvent;

pub struct BrowserScreen<'a> {
    pub current_vfs_dir: &'a str,
    pub selected_file: Option<&'a FileEntry>,
    pub entries: &'a [FileEntry],
    pub directories: &'a [String],
    pub rename_path: Option<&'a str>,
    pub rename_buffer: &'a mut String,
}

impl<'a> BrowserScreen<'a> {
    pub fn show(self, ui: &mut Ui) -> Option<BrowserEvent> {
        let mut result = None;

        Frame::new().inner_margin(16).show(ui, |ui| {
            let action = BreadcrumbBar::new(self.current_vfs_dir).show(ui);
            match action {
                BreadcrumbAction::None => {}
                BreadcrumbAction::Exit => {
                    result = Some(BrowserEvent::CloseVault);
                    return;
                }
                BreadcrumbAction::NavigateTo(path) => {
                    result = Some(BrowserEvent::NavigateTo(path));
                    return;
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            let tree_action = FileTree::new(
                self.entries,
                self.directories,
                self.current_vfs_dir,
                self.rename_buffer,
            )
            .selected(self.selected_file)
            .directories_first(true)
            .show_icons(true)
            .allow_double_click(true)
            .renaming(self.rename_path)
            .show(ui);

            match tree_action {
                DirectoryTreeAction::None => {}
                DirectoryTreeAction::SelectFolder(dir) => {
                    result = Some(BrowserEvent::NavigateTo(dir));
                    return;
                }
                DirectoryTreeAction::SelectFile(file) => {
                    result = Some(BrowserEvent::SelectFile(file));
                    return;
                }
                DirectoryTreeAction::ExecuteFile(file) => {
                    result = Some(BrowserEvent::OpenFile(file));
                    return;
                }
                DirectoryTreeAction::ContextMenu(file) => {
                    result = Some(BrowserEvent::ContextMenu(file));
                }
                DirectoryTreeAction::RenameCancel => {
                    result = Some(BrowserEvent::RenameCancel);
                }
                DirectoryTreeAction::RenameSubmit {
                    old_path,
                    new_path,
                } => {
                    result = Some(BrowserEvent::RenameSubmit {
                        old_path,
                        new_path,
                    });
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            let details_action = FileDetailsView::new(self.selected_file).show(ui);
            match details_action {
                FileDetailsAction::None => {}
                FileDetailsAction::Open(entry) => {
                    result = Some(BrowserEvent::OpenFile(entry));
                }
            }
        });

        result
    }
}
