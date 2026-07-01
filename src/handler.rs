use std::path::PathBuf;
use std::sync::mpsc;
use zeroize::{Zeroize, Zeroizing};

use crate::dialog;
use crate::gui::FileBrowserApp;
use crate::ui::{
    AppEvent, AppState, BrowserEvent, DialogMessage, HomeEvent, PasswordEvent, ProgressMessage,
};
use crate::worker::{WorkerCommand, WorkerResponse};

pub fn handle_event(app: &mut FileBrowserApp, event: AppEvent, ctx: &egui::Context) {
    match event {
        AppEvent::Browser(ev) => handle_browser_event(app, ev, ctx),
        AppEvent::Home(ev) => handle_home_event(app, ev, ctx),
        AppEvent::Password(ev) => handle_password_event(app, ev),
    }
}

fn handle_browser_event(app: &mut FileBrowserApp, ev: BrowserEvent, _ctx: &egui::Context) {
    match ev {
        BrowserEvent::CloseVault => {
            app.container_index = None;
            app.state = AppState::Home;
            if let Some(worker_tx) = &app.worker_tx {
                let _ = worker_tx.send(WorkerCommand::CloseVault);
            }
        }
        BrowserEvent::NavigateTo(dir) => {
            app.current_vfs_dir = dir;
            app.selected_file = None;
        }
        BrowserEvent::SelectFile(file) => {
            app.selected_file = Some(file);
        }
        BrowserEvent::OpenFile(file) => {
            if let Some(worker_tx) = &app.worker_tx {
                let _ = worker_tx.send(WorkerCommand::ReadFile { entry: file });
            }
        }
        BrowserEvent::ContextMenu(file) => {
            app.context_menu = _ctx.pointer_latest_pos().map(|pos| (file, pos));
        }
        BrowserEvent::RenameCancel => {
            app.rename_path = None;
        }
        BrowserEvent::RenameSubmit { old_path, new_path } => {
            app.rename_path = None;
            if let Some(tx) = &app.worker_tx {
                let _ = tx.send(WorkerCommand::RenameFile { old_path, new_path });
            }
        }
        BrowserEvent::DeleteFile(file) => {
            app.context_menu = None;
            if let Some(worker_tx) = &app.worker_tx {
                let _ = worker_tx.send(WorkerCommand::DeleteFile { entry: file });
            }
        }
    }
}

fn handle_home_event(app: &mut FileBrowserApp, ev: HomeEvent, ctx: &egui::Context) {
    match ev {
        HomeEvent::EncryptFolder => {
            if app.dialog_tx.is_none() {
                let (tx, rx) = mpsc::channel();
                app.dialog_tx = Some(tx);
                app.dialog_rx = Some(rx);
            }
            if app.progress_tx.is_none() {
                let (tx, rx) = mpsc::channel();
                app.progress_tx = Some(tx);
                app.progress_rx = Some(rx);
            }
            let tx = app.dialog_tx.as_ref().cloned().unwrap();
            let ctx = ctx.clone();
            dialog::pick_folder_and_save_location(tx, ctx);
        }
        HomeEvent::OpenVault => {
            if app.dialog_tx.is_none() {
                let (tx, rx) = mpsc::channel();
                app.dialog_tx = Some(tx);
                app.dialog_rx = Some(rx);
            }
            let tx = app.dialog_tx.as_ref().cloned().unwrap();
            let ctx = ctx.clone();
            dialog::pick_vault_file(tx, ctx);
        }
    }
}

fn handle_password_event(app: &mut FileBrowserApp, ev: PasswordEvent) {
    match ev {
        PasswordEvent::Submitted(password) => match app.pending_open_path.take() {
            Some(file) => submit_decryption_task(app, file, password),
            None => submit_encryption_task(app, password),
        },
        PasswordEvent::Cancelled => {
            app.pending_directory = None;
            app.pending_save_path = None;
            app.pending_open_path = None;
            app.password_buffer.zeroize();
        }
    }
}

pub fn check_progress(app: &mut FileBrowserApp) {
    if let Some(rx) = app.dialog_rx.take() {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                DialogMessage::FolderPicked(dir) => {
                    app.pending_directory = Some(dir);
                }
                DialogMessage::SaveLocationPicked(save_path) => {
                    app.pending_save_path = Some(save_path);
                }
                DialogMessage::OpenLocationPicked(open_path) => {
                    app.pending_open_path = Some(open_path);
                }
            }
        }
        app.dialog_rx = Some(rx);
    }

    if let Some(rx) = app.progress_rx.take() {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ProgressMessage::Progress {
                    current,
                    total,
                    message,
                } => {
                    app.progress = current as f32 / total as f32;
                    app.progress_message = format!("{} ({}/{})", message, current, total);
                }
            }
        }
        app.progress_rx = Some(rx);
    }

    if let Some(rx) = app.worker_rx.take() {
        while let Ok(msg) = rx.try_recv() {
            process_worker_response(app, msg);
        }
        app.worker_rx = Some(rx);
    }
}

fn process_worker_response(app: &mut FileBrowserApp, msg: WorkerResponse) {
    match msg {
        WorkerResponse::VaultOpened { index } => {
            app.container_index = Some(index);
            app.current_vfs_dir = String::new();
            app.selected_file = None;
            app.state = AppState::Browser;
            app.dialog_tx = None;
            app.progress_tx = None;
            app.progress_rx = None;
            app.progress = 0.0;
            app.progress_message = String::new();
            app.toast_manager.info("Vault opened successfully");
        }
        WorkerResponse::EncryptionDone => {
            app.state = AppState::Home;
            app.dialog_tx = None;
            app.progress_tx = None;
            app.progress_rx = None;
            app.progress = 0.0;
            app.progress_message = String::new();
            app.toast_manager.info("Encryption completed successfully");
        }
        WorkerResponse::FileDecryptedToTemp { temp_path } => {
            let _ = opener::open(&temp_path);
        }
        WorkerResponse::FileUpdated { entry } => {
            if let Some(ref mut index) = app.container_index {
                if let Some(existing) =
                    index.entries.iter_mut().find(|e| e.path == entry.path)
                {
                    *existing = entry;
                }
            }
        }
        WorkerResponse::GarbageCollected => {
            app.toast_manager
                .info("Vault garbage collected successfully");
        }
        WorkerResponse::FileRenamed {
            old_path,
            ref new_path,
        } => {
            if let Some(ref mut index) = app.container_index {
                if let Some(entry) =
                    index.entries.iter_mut().find(|e| e.path == old_path)
                {
                    entry.path.clone_from(new_path);
                }
            }
            if app
                .selected_file
                .as_ref()
                .map(|s| s.path == old_path)
                .unwrap_or(false)
            {
                if let Some(index) = &app.container_index {
                    app.selected_file =
                        index.entries.iter().find(|e| e.path == *new_path).cloned();
                }
            }
            app.toast_manager.info("File renamed");
        }
        WorkerResponse::FileDeleted { path } => {
            if let Some(ref mut index) = app.container_index {
                index.entries.retain(|e| e.path != path);
            }
            if app
                .selected_file
                .as_ref()
                .map(|s| s.path == path)
                .unwrap_or(false)
            {
                app.selected_file = None;
            }
            app.toast_manager.info("File deleted from vault");
        }
        WorkerResponse::Error { message } => {
            if message != "Cancelled" {
                app.toast_manager.error(message);
            }
            app.state = AppState::Home;
            app.dialog_tx = None;
            app.progress_tx = None;
            app.progress_rx = None;
            app.progress = 0.0;
            app.progress_message = String::new();
        }
    }
}

pub fn submit_encryption_task(app: &mut FileBrowserApp, password: Zeroizing<String>) {
    if let (Some(dir), Some(save_path)) =
        (app.pending_directory.take(), app.pending_save_path.take())
    {
        switch_to_loading(app, "Creating container...");

        if let (Some(worker_tx), Some(progress_tx)) =
            (app.worker_tx.clone(), app.progress_tx.clone())
        {
            let _ = worker_tx.send(WorkerCommand::EncryptFolder {
                source_dir: dir,
                output_path: save_path,
                password,
                progress_tx,
            });
        }
    }
}

pub fn submit_decryption_task(
    app: &mut FileBrowserApp,
    file: PathBuf,
    password: Zeroizing<String>,
) {
    switch_to_loading(app, "Opening vault...");

    if let Some(worker_tx) = app.worker_tx.clone() {
        let _ = worker_tx.send(WorkerCommand::OpenVault {
            path: file,
            password,
        });
    }
}

fn switch_to_loading(app: &mut FileBrowserApp, message: &str) {
    app.progress = 0.0;
    app.progress_message = message.to_string();
    app.state = AppState::Loading;
}
