use egui::{RichText, Ui};

use crate::deflate::FileEntry;

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
