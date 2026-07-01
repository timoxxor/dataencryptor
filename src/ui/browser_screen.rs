use egui::{Align, Frame, Layout, RichText, Ui};

use crate::deflate::FileEntry;

use super::browser::{DirectoryTreeAction, FileTree};
use super::types::BrowserEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum BreadcrumbAction {
    None,
    Exit,
    NavigateTo(String),
}

pub struct BreadcrumbBar<'a> {
    current_vfs_dir: &'a str,
}

impl<'a> BreadcrumbBar<'a> {
    pub fn new(current_vfs_dir: &'a str) -> Self {
        Self { current_vfs_dir }
    }

    pub fn show(self, ui: &mut Ui) -> BreadcrumbAction {
        let mut action = BreadcrumbAction::None;

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("📦").size(20.0));

            if ui.small_button("Root").clicked() {
                action = BreadcrumbAction::NavigateTo(String::new());
            }

            let mut accum = String::new();

            for component in self
                .current_vfs_dir
                .split('/')
                .filter(|s| !s.is_empty())
            {
                ui.label(RichText::new("›").weak().size(16.0));

                if accum.is_empty() {
                    accum.push_str(component);
                } else {
                    accum.push('/');
                    accum.push_str(component);
                }

                if ui.small_button(component).clicked() {
                    action = BreadcrumbAction::NavigateTo(accum.clone());
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Exit").clicked() {
                    action = BreadcrumbAction::Exit;
                }
            });
        });

        action
    }
}

#[derive(Debug, Clone)]
pub enum FileDetailsAction {
    None,
    Open(FileEntry),
}

pub struct FileDetailsView<'a> {
    selected_file: Option<&'a FileEntry>,
}

impl<'a> FileDetailsView<'a> {
    pub fn new(selected_file: Option<&'a FileEntry>) -> Self {
        Self { selected_file }
    }

    pub fn show(self, ui: &mut Ui) -> FileDetailsAction {
        let mut action = FileDetailsAction::None;

        ui.vertical(|ui| {
            if let Some(file) = self.selected_file {
                ui.heading("📄 File Information");
                ui.add_space(4.0);

                ui.label(format!("Virtual Path: {}", file.path));
                ui.label(format!("Size: {} bytes", file.stored_size));

                ui.add_space(8.0);
                if ui
                    .button(RichText::new("🚀 Open in System").strong())
                    .clicked()
                {
                    action = FileDetailsAction::Open(file.clone());
                }
            } else {
                ui.colored_label(egui::Color32::GRAY, "Select a file to view details");
            }
        });

        action
    }
}

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
