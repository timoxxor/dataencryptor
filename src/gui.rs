use crate::deflate::{FileEntry, VaultReader, create_container};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use zeroize::Zeroize;

use crate::gif_player::GifPlayer;
use crate::particles;

// Импортируем наши чистые функции рендеринга и оставшиеся вспомогательные структуры
use crate::ui::{
    BreadcrumbAction, CustomTitleBar, DirectoryTreeAction, FileDetailsAction, LoadingModal,
    PasswordModal, PasswordResult, render_breadcrumb_bar, render_directory_tree,
    render_file_details,
};

#[derive(Default, PartialEq)]
pub enum AppState {
    #[default]
    Home,
    Browser,
    Loading,
}

pub enum ProgressMessage {
    StartLoading {
        message: String,
    },
    Progress {
        current: usize,
        total: usize,
        message: String,
    },
    DoneDecrypting {
        reader: VaultReader,
    },
    DoneEncrypting,
    Error {
        error: String,
    },
    FolderPicked(PathBuf),
    SaveLocationPicked(PathBuf),
    OpenLocationPicked(PathBuf),
}

pub struct FileBrowserApp {
    pub state: AppState,
    style_initialized: bool,
    pub current_vfs_dir: String,
    pub selected_file: Option<FileEntry>,
    pub show_password_popup: bool,
    pub password_buffer: String,

    pub pending_directory: Option<PathBuf>,
    pub pending_save_path: Option<PathBuf>,
    pub pending_open_path: Option<PathBuf>,

    pub vault_reader: Option<VaultReader>,
    pub progress: f32,
    pub progress_message: String,

    pub rx: Option<mpsc::Receiver<ProgressMessage>>,
    pub tx: Option<mpsc::Sender<ProgressMessage>>,
    pub operation_in_progress: bool,
    pub operation_result: Option<String>,

    pub background: particles::ParticleBackground,
    pub gif_player: Option<GifPlayer>,
}

impl Default for FileBrowserApp {
    fn default() -> Self {
        Self {
            state: AppState::Home,
            style_initialized: false,
            current_vfs_dir: String::new(),
            selected_file: None,
            show_password_popup: false,
            password_buffer: String::new(),
            pending_directory: None,
            pending_save_path: None,
            pending_open_path: None,
            vault_reader: None,
            progress: 0.0,
            progress_message: String::new(),
            rx: None,
            tx: None,
            operation_in_progress: false,
            operation_result: None,
            background: particles::ParticleBackground::default(),
            gif_player: None,
        }
    }
}

impl FileBrowserApp {
    fn initialize_style(&mut self, ctx: &egui::Context) {
        if self.style_initialized {
            return;
        }

        if self.gif_player.is_none() {
            self.gif_player = Some(GifPlayer::new(ctx, "assets/title.gif"));
        }

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
        self.style_initialized = true;
    }

    fn check_progress(&mut self) {
        if let Some(rx) = self.rx.take() {
            let mut closed = false;

            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ProgressMessage::StartLoading { message } => {
                        self.operation_in_progress = true;
                        self.state = AppState::Loading;
                        self.progress = 0.0;
                        self.progress_message = message;
                        self.operation_result = None;
                    }
                    ProgressMessage::FolderPicked(dir) => {
                        self.pending_directory = Some(dir);
                    }
                    ProgressMessage::SaveLocationPicked(save_path) => {
                        self.pending_save_path = Some(save_path);
                        self.show_password_popup = true;
                    }
                    ProgressMessage::OpenLocationPicked(open_path) => {
                        self.pending_open_path = Some(open_path);
                        self.show_password_popup = true;
                    }
                    ProgressMessage::Progress {
                        current,
                        total,
                        message,
                    } => {
                        self.progress = current as f32 / total as f32;
                        self.progress_message = format!("{} ({}/{})", message, current, total);
                    }
                    ProgressMessage::DoneEncrypting => {
                        self.state = AppState::Home;
                        self.operation_in_progress = false;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                        closed = true;
                    }
                    ProgressMessage::DoneDecrypting { reader } => {
                        self.vault_reader = Some(reader);
                        self.current_vfs_dir = String::new();
                        self.selected_file = None;
                        self.state = AppState::Browser;
                        self.operation_in_progress = false;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                        closed = true;
                    }
                    ProgressMessage::Error { error } => {
                        if error != "Cancelled" {
                            self.operation_result = Some(format!("Error: {}", error));
                        }
                        self.operation_in_progress = false;
                        self.state = AppState::Home;
                        self.tx = None;
                        self.progress = 0.0;
                        self.progress_message = String::new();
                        closed = true;
                    }
                }
            }

            if !closed {
                self.rx = Some(rx);
            }
        }
    }

    // --- Скрин Домашнего Экрана ---
    fn render_home_screen(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(40.0);

                    if let Some(player) = &mut self.gif_player {
                        player.render(ui);
                    }

                    ui.add_space(15.0);

                    if let Some(result) = &self.operation_result {
                        ui.colored_label(egui::Color32::RED, result);
                        ui.add_space(10.0);
                    }

                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgba_premultiplied(30, 30, 30, 150))
                        .corner_radius(12.0)
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.allocate_ui(egui::vec2(280.0, 0.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    self.render_encrypt_folder_button(ui);
                                    ui.add_space(10.0);
                                    self.render_open_evfs_button(ui);
                                });
                            });
                        });
                });
            });

        // Работа со структурами-попапами (без трейта Widget)
        self.handle_password_popup(ui.ctx());
        self.render_loading_popup(ui.ctx());
    }

    fn render_encrypt_folder_button(&mut self, ui: &mut egui::Ui) {
        let button = ui.add_sized([260.0, 45.0], egui::Button::new("📁 Encrypt folder"));
        if button.clicked() && !self.operation_in_progress {
            if self.tx.is_none() {
                let (tx, rx) = mpsc::channel();
                self.tx = Some(tx);
                self.rx = Some(rx);
            }
            let tx = self.tx.clone().unwrap();
            let ctx = ui.ctx().clone();
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
    }

    fn render_open_evfs_button(&mut self, ui: &mut egui::Ui) {
        let button = ui.add_sized([260.0, 45.0], egui::Button::new("🔐 Open EVFS(.enc)"));
        if button.clicked() && !self.operation_in_progress {
            if self.tx.is_none() {
                let (tx, rx) = mpsc::channel();
                self.tx = Some(tx);
                self.rx = Some(rx);
            }
            let tx = self.tx.clone().unwrap();
            let ctx = ui.ctx().clone();
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
    }

    // --- Скрин Браузера Файлов ---
    fn render_browser_screen(&mut self, ui: &mut egui::Ui) {
        self.check_progress();

        let entries = match &self.vault_reader {
            Some(r) => r.index.entries.clone(),
            None => {
                self.state = AppState::Home;
                return;
            }
        };

        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            match render_breadcrumb_bar(ui, &self.current_vfs_dir) {
                BreadcrumbAction::None => {}

                BreadcrumbAction::Exit => {
                    self.vault_reader = None;
                    self.state = AppState::Home;
                    return;
                }

                BreadcrumbAction::NavigateTo(path) => {
                    self.current_vfs_dir = path;
                    self.selected_file = None;
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // 2. Дерево файлов через чистую функцию
            match render_directory_tree(
                ui,
                &self.current_vfs_dir,
                self.selected_file.as_ref(),
                &entries,
            ) {
                DirectoryTreeAction::None => {}
                DirectoryTreeAction::SelectFolder(dir) => {
                    self.current_vfs_dir = dir;
                    self.selected_file = None;
                }
                DirectoryTreeAction::SelectFile(file) => {
                    self.selected_file = Some(file);
                }
                DirectoryTreeAction::ExecuteFile(file) => {
                    if let Some(reader) = &mut self.vault_reader {
                        let _ = reader.open_file_in_system(&file);
                    }
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // 3. Панель детальной информации через чистую функцию
            match render_file_details(ui, self.selected_file.as_ref()) {
                FileDetailsAction::None => {}
                FileDetailsAction::Open(entry) => {
                    if let Some(reader) = &mut self.vault_reader {
                        let _ = reader.open_file_in_system(&entry);
                    }
                }
            }
        });
    }

    fn handle_password_popup(&mut self, ctx: &egui::Context) {
        if !self.show_password_popup {
            return;
        }

        let is_opening = self.pending_open_path.is_some();
        let modal_result = PasswordModal::new(is_opening, &mut self.password_buffer).show(ctx);

        match modal_result {
            PasswordResult::None => {}
            PasswordResult::Submit(password) => {
                if is_opening {
                    self.submit_decryption_task(password);
                } else {
                    self.submit_encryption_task(password);
                }
            }
            PasswordResult::Cancel => {
                self.cancel_password_process();
            }
        }
    }

    fn render_loading_popup(&mut self, ctx: &egui::Context) {
        if self.operation_in_progress && self.state == AppState::Loading {
            LoadingModal::new(self.progress, &self.progress_message).show(ctx);
        }
    }

    // --- Фоновые бизнес-задачи ---
    fn submit_encryption_task(&mut self, password: zeroize::Zeroizing<String>) {
        if let (Some(dir), Some(save_path)) =
            (self.pending_directory.take(), self.pending_save_path.take())
        {
            self.switch_to_loading("Creating container...");
            self.create_container_async(dir, save_path, password.to_string());
        }
    }

    fn submit_decryption_task(&mut self, password: zeroize::Zeroizing<String>) {
        if let Some(file) = self.pending_open_path.take() {
            self.switch_to_loading("Opening vault...");
            let tx = self.tx.clone().unwrap();

            std::thread::spawn(move || {
                let _ = tx.send(ProgressMessage::StartLoading {
                    message: "Opening vault...".to_string(),
                });
                match VaultReader::open(&file, password.to_string()) {
                    Ok(reader) => {
                        let _ = tx.send(ProgressMessage::DoneDecrypting { reader });
                    }
                    Err(err) => {
                        let _ = tx.send(ProgressMessage::Error {
                            error: format!("Error while opening an EVFS: {}", err),
                        });
                    }
                }
            });
        }
    }

    fn cancel_password_process(&mut self) {
        self.show_password_popup = false;
        self.pending_directory = None;
        self.pending_save_path = None;
        self.pending_open_path = None;
        self.password_buffer.zeroize();
    }

    fn create_container_async(&mut self, dir: PathBuf, save_path: PathBuf, password: String) {
        let tx = self.tx.clone().unwrap();
        std::thread::spawn(
            move || match create_container(&dir, &save_path, &tx, password) {
                Ok(_) => {
                    let _ = tx.send(ProgressMessage::DoneEncrypting);
                }
                Err(err) => {
                    let _ = tx.send(ProgressMessage::Error {
                        error: format!("Error while creating EVFS: {}", err),
                    });
                }
            },
        );
    }

    fn switch_to_loading(&mut self, message: &str) {
        self.show_password_popup = false;
        self.progress = 0.0;
        self.progress_message = message.to_string();
        self.operation_in_progress = true;
        self.state = AppState::Loading;
    }
}

impl eframe::App for FileBrowserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.initialize_style(ui.ctx());
        self.background.update_and_draw(ui);

        CustomTitleBar::show(ui);
        ui.add_space(10.0);

        if self.rx.is_some() {
            self.check_progress();
            if self.operation_in_progress {
                ui.ctx().request_repaint();
            }
        }

        match self.state {
            AppState::Home => self.render_home_screen(ui),
            AppState::Browser => self.render_browser_screen(ui),
            AppState::Loading => {
                // Если мы в стейте Loading, рисуем базовый домашний экран
                // как подложку, а поверх него вызовется модалка загрузки
                self.render_home_screen(ui);
            }
        }
    }
}
