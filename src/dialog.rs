use std::sync::mpsc;

use crate::ui::DialogMessage;

pub fn pick_folder_and_save_location(tx: mpsc::Sender<DialogMessage>, ctx: egui::Context) {
    std::thread::spawn(move || {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            if let Some(save_path) = rfd::FileDialog::new()
                .add_filter("EVFS", &["enc"])
                .save_file()
            {
                let _ = tx.send(DialogMessage::FolderPicked(dir));
                let _ = tx.send(DialogMessage::SaveLocationPicked(save_path));
                ctx.request_repaint();
            }
        }
    });
}

pub fn pick_vault_file(tx: mpsc::Sender<DialogMessage>, ctx: egui::Context) {
    std::thread::spawn(move || {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("EVFS", &["enc"])
            .pick_file()
        {
            let _ = tx.send(DialogMessage::OpenLocationPicked(file));
            ctx.request_repaint();
        }
    });
}
