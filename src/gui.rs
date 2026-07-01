use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::deflate::{ContainerIndex, FileEntry};
use crate::gif_player::GifPlayer;
use crate::handler;
use crate::particles;
use crate::theme;
use crate::ui::{
    AppEvent, AppState, BrowserEvent, BrowserScreen, ContextMenu, ContextMenuAction,
    CustomTitleBar, DialogMessage, HomeScreen, LoadingPopup, PasswordPopup, ProgressMessage,
    PropertiesDialog, ToastManager,
};
use crate::worker::{WorkerCommand, WorkerResponse};

pub struct FileBrowserApp {
    pub state: AppState,
    pub current_vfs_dir: String,
    pub selected_file: Option<FileEntry>,
    pub password_buffer: String,

    pub pending_directory: Option<PathBuf>,
    pub pending_save_path: Option<PathBuf>,
    pub pending_open_path: Option<PathBuf>,

    pub dialog_tx: Option<mpsc::Sender<DialogMessage>>,
    pub dialog_rx: Option<mpsc::Receiver<DialogMessage>>,
    pub progress_tx: Option<mpsc::Sender<ProgressMessage>>,
    pub progress_rx: Option<mpsc::Receiver<ProgressMessage>>,

    pub worker_tx: Option<mpsc::Sender<WorkerCommand>>,
    pub worker_rx: Option<mpsc::Receiver<WorkerResponse>>,
    pub container_index: Option<ContainerIndex>,

    pub progress: f32,
    pub progress_message: String,

    pub background: particles::ParticleBackground,
    pub gif_player: Option<GifPlayer>,
    pub toast_manager: ToastManager,
    pub context_menu: Option<(FileEntry, egui::Pos2)>,
    pub rename_path: Option<String>,
    pub rename_buffer: String,
    pub properties_dialog: PropertiesDialog,
}

impl FileBrowserApp {
    pub fn new(ctx: &egui::Context, file_to_open: Option<PathBuf>) -> Self {
        let (worker_tx, worker_rx) = crate::worker::spawn();

        Self {
            state: AppState::Home,
            current_vfs_dir: String::new(),
            selected_file: None,
            password_buffer: String::new(),
            pending_directory: None,
            pending_save_path: None,
            pending_open_path: file_to_open,
            dialog_tx: None,
            dialog_rx: None,
            progress_tx: None,
            progress_rx: None,
            worker_tx: Some(worker_tx),
            worker_rx: Some(worker_rx),
            container_index: None,
            progress: 0.0,
            progress_message: String::new(),
            background: particles::ParticleBackground::default(),
            gif_player: Some(GifPlayer::new(ctx, include_bytes!("../assets/title.gif"))),
            toast_manager: ToastManager::new(),
            context_menu: None,
            rename_path: None,
            rename_buffer: String::new(),
            properties_dialog: PropertiesDialog::new(),
        }
    }
}

impl eframe::App for FileBrowserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::apply_style(ui.ctx());
        self.background.update_and_draw(ui);
        self.toast_manager.show(ui.ctx());

        CustomTitleBar::show(ui);
        ui.add_space(10.0);

        handler::check_progress(self);
        if self.state == AppState::Loading {
            ui.ctx().request_repaint();
        }

        match self.state {
            AppState::Home => {
                let home = HomeScreen {
                    gif_player: &mut self.gif_player,
                    state: &self.state,
                };
                if let Some(event) = home.show(ui) {
                    handler::handle_event(self, AppEvent::Home(event), ui.ctx());
                }

                let popup = PasswordPopup::new(
                    self.pending_save_path.as_ref(),
                    self.pending_open_path.as_ref(),
                    &mut self.password_buffer,
                );
                if let Some(event) = popup.show(ui.ctx()) {
                    handler::handle_event(self, AppEvent::Password(event), ui.ctx());
                }
            }
            AppState::Browser => {
                let (entries, directories) = match &self.container_index {
                    Some(index) => (index.entries.clone(), index.directories.clone()),
                    None => {
                        self.state = AppState::Home;
                        return;
                    }
                };

                let browser = BrowserScreen {
                    current_vfs_dir: &self.current_vfs_dir,
                    selected_file: self.selected_file.as_ref(),
                    entries: &entries,
                    directories: &directories,
                    rename_path: self.rename_path.as_deref(),
                    rename_buffer: &mut self.rename_buffer,
                };

                if let Some(event) = browser.show(ui) {
                    handler::handle_event(self, AppEvent::Browser(event), ui.ctx());
                }

                if let Some((file, pos)) = self.context_menu.clone() {
                    let resp = ui.add(ContextMenu::new(file.clone(), pos));
                    let action = ui
                        .ctx()
                        .data_mut(|d| d.remove_temp::<ContextMenuAction>(egui::Id::new("ctx_menu_action")))
                        .unwrap_or(ContextMenuAction::None);
                    match action {
                        ContextMenuAction::Open(file) => {
                            self.context_menu = None;
                            handler::handle_event(
                                self,
                                AppEvent::Browser(BrowserEvent::OpenFile(file)),
                                ui.ctx(),
                            );
                        }
                        ContextMenuAction::Properties(file) => {
                            self.context_menu = None;
                            self.properties_dialog.open(file);
                        }
                        ContextMenuAction::Rename(file) => {
                            self.context_menu = None;

                            let file_name = Path::new(&file.path)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();

                            self.rename_buffer.clone_from(&file_name);

                            ui.ctx().data_mut(|d| {
                                d.insert_persisted(
                                    egui::Id::new("rename_sel_done").with(&file_name),
                                    false,
                                );
                                d.insert_persisted(
                                    egui::Id::new("rename_focus").with(&file_name),
                                    false,
                                );
                            });

                            self.rename_path = Some(file.path.clone());
                        }
                        ContextMenuAction::Delete(file) => {
                            self.context_menu = None;
                            handler::handle_event(
                                self,
                                AppEvent::Browser(BrowserEvent::DeleteFile(file)),
                                ui.ctx(),
                            );
                        }
                        ContextMenuAction::None => {}
                    }
                    if resp.clicked_elsewhere() {
                        self.context_menu = None;
                    }
                }

                self.properties_dialog.show(ui.ctx());
            }
            AppState::Loading => {
                let home = HomeScreen {
                    gif_player: &mut self.gif_player,
                    state: &self.state,
                };
                if let Some(event) = home.show(ui) {
                    handler::handle_event(self, AppEvent::Home(event), ui.ctx());
                }

                let popup = PasswordPopup::new(
                    self.pending_save_path.as_ref(),
                    self.pending_open_path.as_ref(),
                    &mut self.password_buffer,
                );
                if let Some(event) = popup.show(ui.ctx()) {
                    handler::handle_event(self, AppEvent::Password(event), ui.ctx());
                }

                LoadingPopup::new(&self.state, self.progress, &self.progress_message)
                    .show(ui.ctx());
            }
        }
    }
}
