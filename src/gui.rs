use egui::{Id, Ui};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use zeroize::Zeroize;

use crate::deflate::{ContainerIndex, FileEntry};
use crate::gif_player::GifPlayer;
use crate::particles;
use crate::worker::{WorkerCommand, WorkerResponse};

use crate::ui::{
    AppEvent, AppState, BrowserEvent, BrowserScreen, CustomTitleBar, HomeEvent, HomeScreen,
    LoadingModal, PasswordEvent, PasswordModal, ProgressMessage, ToastManager,
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

    pub background: particles::ParticleBackground,
    pub gif_player: Option<GifPlayer>,
    pub toast_manager: ToastManager,
    pub context_menu: Option<(FileEntry, egui::Pos2)>,
    pub rename_path: Option<String>,
    pub rename_buffer: String,
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
            background: particles::ParticleBackground::default(),
            gif_player: Some(GifPlayer::new(ctx, "assets/title.gif")),
            toast_manager: ToastManager::new(),
            context_menu: None,
            rename_path: None,
            rename_buffer: String::new(),
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
                        self.toast_manager.info("Vault opened successfully");
                    }
                    WorkerResponse::EncryptionDone => {
                        self.state = AppState::Home;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                        self.toast_manager.info("Encryption completed successfully");
                    }
                    WorkerResponse::FileDecryptedToTemp { temp_path } => {
                        let _ = opener::open(&temp_path);
                    }
                    WorkerResponse::FileUpdated { entry } => {
                        if let Some(ref mut index) = self.container_index {
                            if let Some(existing) =
                                index.entries.iter_mut().find(|e| e.path == entry.path)
                            {
                                *existing = entry;
                            }
                        }
                    }
                    WorkerResponse::GarbageCollected => {
                        self.toast_manager
                            .info("Vault garbage collected successfully");
                    }
                    WorkerResponse::FileRenamed {
                        old_path,
                        ref new_path,
                    } => {
                        if let Some(ref mut index) = self.container_index {
                            if let Some(entry) =
                                index.entries.iter_mut().find(|e| e.path == old_path)
                            {
                                entry.path.clone_from(new_path);
                            }
                        }
                        if self
                            .selected_file
                            .as_ref()
                            .map(|s| s.path == old_path)
                            .unwrap_or(false)
                        {
                            if let Some(index) = &self.container_index {
                                self.selected_file =
                                    index.entries.iter().find(|e| e.path == *new_path).cloned();
                            }
                        }
                        self.toast_manager.info("File renamed");
                    }
                    WorkerResponse::FileDeleted { path } => {
                        if let Some(ref mut index) = self.container_index {
                            index.entries.retain(|e| e.path != path);
                        }
                        if self
                            .selected_file
                            .as_ref()
                            .map(|s| s.path == path)
                            .unwrap_or(false)
                        {
                            self.selected_file = None;
                        }
                        self.toast_manager.info("File deleted from vault");
                    }
                    WorkerResponse::Error { message } => {
                        if message != "Cancelled" {
                            self.toast_manager.error(message);
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
                BrowserEvent::ContextMenu(file) => {
                    self.context_menu = ctx.pointer_latest_pos().map(|pos| (file, pos));
                }
                BrowserEvent::RenameCancel => {
                    self.rename_path = None;
                }
                BrowserEvent::RenameSubmit { old_path, new_path } => {
                    self.rename_path = None;
                    if let Some(tx) = &self.worker_tx {
                        let _ = tx.send(WorkerCommand::RenameFile { old_path, new_path });
                    }
                }
                BrowserEvent::DeleteFile(file) => {
                    self.context_menu = None;
                    if let Some(worker_tx) = &self.worker_tx {
                        let _ = worker_tx.send(WorkerCommand::DeleteFile { entry: file });
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

    fn render_context_menu(&mut self, ctx: &egui::Context) {
        let Some((file, pos)) = self.context_menu.clone() else {
            return;
        };

        let area_id = egui::Id::new("file_context_menu");

        let area_resp = egui::Area::new(area_id)
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let frame = egui::Frame {
                    fill: egui::Color32::from_rgba_unmultiplied(
                        ui.style().visuals.window_fill().r(),
                        ui.style().visuals.window_fill().g(),
                        ui.style().visuals.window_fill().b(),
                        220,
                    ),
                    stroke: ui.style().visuals.window_stroke(),
                    corner_radius: egui::CornerRadius::ZERO,
                    shadow: egui::epaint::Shadow::default(),
                    ..Default::default()
                };

                frame.show(ui, |ui| {
                    ui.set_width(100.0);
                    ui.spacing_mut().item_spacing.y = 0.0;

                    let item = |ui: &mut egui::Ui, label: &str| -> bool {
                        let galley = egui::WidgetText::from(label).into_galley(
                            ui,
                            Some(egui::TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Button,
                        );

                        let padding = ui.spacing().button_padding;
                        let size =
                            egui::vec2(ui.available_width(), galley.size().y + padding.y * 2.0);

                        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, 0.0, egui::Color32::from_gray(55));
                        }

                        ui.painter().galley(
                            rect.min + egui::vec2(padding.x, padding.y),
                            galley,
                            ui.style().visuals.text_color(),
                        );

                        response.clicked()
                    };

                    if item(ui, "Open") {
                        self.context_menu = None;
                        self.handle_event(
                            AppEvent::Browser(BrowserEvent::OpenFile(file.clone())),
                            ctx,
                        );
                        ui.close();
                    }

                    if item(ui, "Properties") {
                        self.context_menu = None;
                        self.handle_event(
                            AppEvent::Browser(BrowserEvent::SelectFile(file.clone())),
                            ctx,
                        );
                        ui.close();
                    }

                    if item(ui, "Rename") {
                        self.context_menu = None;

                        let file_name = Path::new(&file.path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();

                        self.rename_buffer = file_name.clone();

                        ctx.data_mut(|d| {
                            d.insert_persisted(Id::new("rename_sel_done").with(&file_name), false);
                            d.insert_persisted(Id::new("rename_focus").with(&file_name), false);
                        });

                        self.rename_path = Some(file.path.clone());

                        ui.close();
                    }

                    if item(ui, "Delete") {
                        self.context_menu = None;
                        self.handle_event(AppEvent::Browser(BrowserEvent::DeleteFile(file)), ctx);
                        ui.close();
                    }
                });
            });

        if area_resp.response.clicked_elsewhere() {
            self.context_menu = None;
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
        self.toast_manager.show(ui.ctx());

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
                    state: &self.state,
                };
                if let Some(event) = home.show(ui) {
                    self.handle_event(AppEvent::Home(event), ui.ctx());
                }

                self.render_password_popup(ui.ctx());
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
                    self.handle_event(AppEvent::Browser(event), ui.ctx());
                }

                self.render_context_menu(ui.ctx());
            }
            AppState::Loading => {
                let home = HomeScreen {
                    gif_player: &mut self.gif_player,
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
