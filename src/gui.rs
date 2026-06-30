use std::path::PathBuf;
use std::sync::mpsc;
use zeroize::Zeroize;

use crate::deflate::{ContainerIndex, FileEntry};
use crate::gif_player::GifPlayer;
use crate::particles;
use crate::worker::{WorkerCommand, WorkerResponse};

use crate::ui::{
    AppEvent, AppState, BrowserEvent, BrowserScreen, CustomTitleBar, HomeEvent, HomeScreen,
    LoadingModal, PasswordEvent, PasswordModal, ProgressMessage,
};

pub struct FileBrowserApp {
    pub state: AppState,
    pub current_vfs_dir: String,
    pub selected_file: Option<FileEntry>,
    pub password_buffer: String,

    pub pending_directory: Option<PathBuf>,
    pub pending_save_path: Option<PathBuf>,
    pub pending_open_path: Option<PathBuf>,

    pub worker_tx: Option<mpsc::Sender<WorkerCommand>>,
    pub worker_rx: Option<mpsc::Receiver<WorkerResponse>>,
    pub container_index: Option<ContainerIndex>,

    pub progress: f32,
    pub progress_message: String,

    pub rx: Option<mpsc::Receiver<ProgressMessage>>,
    pub tx: Option<mpsc::Sender<ProgressMessage>>,
    pub operation_result: Option<String>,

    pub background: particles::ParticleBackground,
    pub gif_player: Option<GifPlayer>,
}

impl FileBrowserApp {
    pub fn new(ctx: &egui::Context) -> Self {
        let (worker_tx, worker_rx) = crate::worker::spawn();

        Self {
            state: AppState::Home,
            current_vfs_dir: String::new(),
            selected_file: None,
            password_buffer: String::new(),
            pending_directory: None,
            pending_save_path: None,
            pending_open_path: None,
            worker_tx: Some(worker_tx),
            worker_rx: Some(worker_rx),
            container_index: None,
            progress: 0.0,
            progress_message: String::new(),
            rx: None,
            tx: None,
            operation_result: None,
            background: particles::ParticleBackground::default(),
            gif_player: Some(GifPlayer::new(ctx, "assets/title.gif")),
        }
    }

    fn apply_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.global_style()).clone();
        style.visuals = egui::Visuals::dark();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.window_margin = egui::Margin::same(16);
        style.visuals.window_corner_radius = 14.0.into();
        style.visuals.menu_corner_radius = 12.0.into();
        style.visuals.widgets.inactive.corner_radius = 10.0.into();
        style.visuals.widgets.hovered.corner_radius = 10.0.into();
        style.visuals.widgets.active.corner_radius = 10.0.into();
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(80, 120, 220);
        ctx.set_global_style(style);
    }

    fn check_progress(&mut self) {
        if let Some(rx) = self.rx.take() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ProgressMessage::FolderPicked(dir) => {
                        self.pending_directory = Some(dir);
                    }
                    ProgressMessage::SaveLocationPicked(save_path) => {
                        self.pending_save_path = Some(save_path);
                    }
                    ProgressMessage::OpenLocationPicked(open_path) => {
                        self.pending_open_path = Some(open_path);
                    }
                    ProgressMessage::Progress {
                        current,
                        total,
                        message,
                    } => {
                        self.progress = current as f32 / total as f32;
                        self.progress_message = format!("{} ({}/{})", message, current, total);
                    }
                }
            }

            self.rx = Some(rx);
        }

        if let Some(rx) = self.worker_rx.take() {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerResponse::VaultOpened { index } => {
                        self.container_index = Some(index);
                        self.current_vfs_dir = String::new();
                        self.selected_file = None;
                        self.state = AppState::Browser;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                    }
                    WorkerResponse::EncryptionDone => {
                        self.state = AppState::Home;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                    }
                    WorkerResponse::FileDecryptedToTemp { temp_path } => {
                        let _ = opener::open(&temp_path);
                    }
                    WorkerResponse::Error { message } => {
                        if message != "Cancelled" {
                            self.operation_result = Some(format!("Error: {}", message));
                        }
                        self.state = AppState::Home;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                    }
                }
            }

            self.worker_rx = Some(rx);
        }
    }

    fn submit_encryption_task(&mut self, password: zeroize::Zeroizing<String>) {
        if let (Some(dir), Some(save_path)) =
            (self.pending_directory.take(), self.pending_save_path.take())
        {
            self.switch_to_loading("Creating container...");

            if let (Some(worker_tx), Some(tx)) = (self.worker_tx.clone(), self.tx.clone()) {
                let _ = worker_tx.send(WorkerCommand::EncryptFolder {
                    source_dir: dir,
                    output_path: save_path,
                    password,
                    progress_tx: tx,
                });
            }
        }
    }

    fn submit_decryption_task(&mut self, file: PathBuf, password: zeroize::Zeroizing<String>) {
        self.switch_to_loading("Opening vault...");

        if let Some(worker_tx) = self.worker_tx.clone() {
            let _ = worker_tx.send(WorkerCommand::OpenVault {
                path: file,
                password,
            });
        }
    }

    fn handle_event(&mut self, event: AppEvent, ctx: &egui::Context) {
        match event {
            AppEvent::Browser(ev) => match ev {
                BrowserEvent::CloseVault => {
                    self.container_index = None;
                    self.state = AppState::Home;
                    if let Some(worker_tx) = &self.worker_tx {
                        let _ = worker_tx.send(WorkerCommand::CloseVault);
                    }
                }
                BrowserEvent::NavigateTo(dir) => {
                    self.current_vfs_dir = dir;
                    self.selected_file = None;
                }
                BrowserEvent::SelectFile(file) => {
                    self.selected_file = Some(file);
                }
                BrowserEvent::OpenFile(file) => {
                    if let Some(worker_tx) = &self.worker_tx {
                        let _ = worker_tx.send(WorkerCommand::ReadFile { entry: file });
                    }
                }
            },
            AppEvent::Home(ev) => match ev {
                HomeEvent::EncryptFolder => {
                    if self.tx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        self.tx = Some(tx);
                        self.rx = Some(rx);
                    }
                    let tx = self.tx.as_ref().cloned().unwrap();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            if let Some(save_path) = rfd::FileDialog::new()
                                .add_filter("EVFS", &["enc"])
                                .save_file()
                            {
                                let _ = tx.send(ProgressMessage::FolderPicked(dir));
                                let _ = tx.send(ProgressMessage::SaveLocationPicked(save_path));
                                ctx.request_repaint();
                            }
                        }
                    });
                }
                HomeEvent::OpenVault => {
                    if self.tx.is_none() {
                        let (tx, rx) = mpsc::channel();
                        self.tx = Some(tx);
                        self.rx = Some(rx);
                    }
                    let tx = self.tx.as_ref().cloned().unwrap();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        if let Some(file) = rfd::FileDialog::new()
                            .add_filter("EVFS", &["enc"])
                            .pick_file()
                        {
                            let _ = tx.send(ProgressMessage::OpenLocationPicked(file));
                            ctx.request_repaint();
                        }
                    });
                }
            },
            AppEvent::Password(ev) => match ev {
                PasswordEvent::Submitted(password) => match self.pending_open_path.take() {
                    Some(file) => self.submit_decryption_task(file, password),
                    None => self.submit_encryption_task(password),
                },
                PasswordEvent::Cancelled => {
                    self.pending_directory = None;
                    self.pending_save_path = None;
                    self.pending_open_path = None;
                    self.password_buffer.zeroize();
                }
            },
        }
    }

    fn switch_to_loading(&mut self, message: &str) {
        self.progress = 0.0;
        self.progress_message = message.to_string();
        self.state = AppState::Loading;
    }

    fn render_password_popup(&mut self, ctx: &egui::Context) {
        if !(self.pending_save_path.is_some() || self.pending_open_path.is_some()) {
            return;
        }

        let is_opening = self.pending_open_path.is_some();
        if let Some(event) = PasswordModal::new(is_opening, &mut self.password_buffer).show(ctx) {
            self.handle_event(AppEvent::Password(event), ctx);
        }
    }

    fn render_loading_popup(&mut self, ctx: &egui::Context) {
        if self.state == AppState::Loading {
            LoadingModal::new(self.progress, &self.progress_message).show(ctx);
        }
    }
}

impl eframe::App for FileBrowserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.apply_style(ui.ctx());
        self.background.update_and_draw(ui);

        CustomTitleBar::show(ui);
        ui.add_space(10.0);

        self.check_progress();
        if self.state == AppState::Loading {
            ui.ctx().request_repaint();
        }

        match self.state {
            AppState::Home => {
                let home = HomeScreen {
                    gif_player: &mut self.gif_player,
                    operation_result: &self.operation_result,
                    state: &self.state,
                };
                if let Some(event) = home.show(ui) {
                    self.handle_event(AppEvent::Home(event), ui.ctx());
                }

                self.render_password_popup(ui.ctx());
            }
            AppState::Browser => {
                let entries = match &self.container_index {
                    Some(index) => index.entries.clone(),
                    None => {
                        self.state = AppState::Home;
                        return;
                    }
                };

                let browser = BrowserScreen {
                    current_vfs_dir: &self.current_vfs_dir,
                    selected_file: self.selected_file.as_ref(),
                    entries: &entries,
                };

                if let Some(event) = browser.show(ui) {
                    self.handle_event(AppEvent::Browser(event), ui.ctx());
                }
            }
            AppState::Loading => {
                let home = HomeScreen {
                    gif_player: &mut self.gif_player,
                    operation_result: &self.operation_result,
                    state: &self.state,
                };
                if let Some(event) = home.show(ui) {
                    self.handle_event(AppEvent::Home(event), ui.ctx());
                }

                self.render_password_popup(ui.ctx());
                self.render_loading_popup(ui.ctx());
            }
        }
    }
}
