use egui::{Ui, RichText};
use crate::deflate::FileEntry;

pub enum FileDetailsAction {
    None,
    Open(FileEntry),
}

pub fn render_file_details(ui: &mut Ui, selected_file: Option<&FileEntry>) -> FileDetailsAction {
    let mut action = FileDetailsAction::None;

    ui.vertical(|ui| {
        if let Some(file) = selected_file {
            ui.heading("📄 File Information");
            ui.add_space(4.0);
            
            ui.label(format!("Virtual Path: {}", file.path));
            ui.label(format!("Size: {} bytes", file.stored_size));
            
            ui.add_space(8.0);
            if ui.button(RichText::new("🚀 Open in System").strong()).clicked() {
                action = FileDetailsAction::Open(file.clone());
            }
        } else {
            ui.colored_label(egui::Color32::GRAY, "Select a file to view details");
        }
    });

    action
}