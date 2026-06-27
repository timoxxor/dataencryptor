use crate::deflate::{FileEntry, VaultReader, create_container};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use zeroize::{Zeroize, Zeroizing};

use crate::gif_player::GifPlayer;
use crate::particles;

#[derive(Default, PartialEq)]
enum AppState {
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
    state: AppState,
    style_initialized: bool,
    current_vfs_dir: String,
    selected_file: Option<FileEntry>,
    show_password_popup: bool,
    password_buffer: String,

    pending_directory: Option<PathBuf>,
    pending_save_path: Option<PathBuf>,
    pending_open_path: Option<PathBuf>,

    vault_reader: Option<VaultReader>,
    progress: f32,
    progress_message: String,
    
    rx: Option<mpsc::Receiver<ProgressMessage>>,
    tx: Option<mpsc::Sender<ProgressMessage>>,
    operation_in_progress: bool,
    operation_result: Option<String>,

    background: particles::ParticleBackground,
    gif_player: Option<GifPlayer>,
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

        self.render_password_popup(ui.ctx());
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

    fn render_custom_title_bar(&mut self, ui: &mut egui::Ui) {
        use eframe::egui::{Align, Layout, RichText, Sense, Vec2, ViewportCommand};

        let height = 32.0;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            Sense::click_and_drag(),
        );

        if response.dragged() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );

        child.add_space(8.0);
        child.label(RichText::new("🔒").size(16.0));
        child.add_space(6.0);
        child.label(RichText::new("XOR-evfs").strong().size(15.0));

        child.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = ui.add_sized(
                [28.0, 24.0],
                egui::Button::new(RichText::new("❌").size(14.0)).frame(false),
            );

            if close.clicked() {
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        });
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

    fn render_loading_popup(&mut self, ctx: &egui::Context) {
        if !self.operation_in_progress || self.state != AppState::Loading {
            return;
        }

        egui::Window::new("Processing...")
            .collapsible(false)
            .resizable(false)
            .fixed_size([400.0, 120.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(&self.progress_message);
                    ui.add_space(10.0);

                    let progress_bar = egui::ProgressBar::new(self.progress)
                        .show_percentage()
                        .animate(true);
                    ui.add_sized([300.0, 20.0], progress_bar);
                });
            });
    }

    fn render_password_popup(&mut self, ctx: &egui::Context) {
        if !self.show_password_popup {
            return;
        }

        // Контекст операции вычисляется на основе того, какой путь сейчас ожидает обработки
        let is_opening = self.pending_open_path.is_some();

        let title = if is_opening { "🔐 Open EVFS Container" } else { "🔒 Create Encrypted Container" };
        let label = if is_opening { "Enter password to decrypt container:" } else { "Enter password to encrypt container:" };
        let btn_label = if is_opening { "🔓 Open" } else { "🔒 Encrypt" };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label(label);
                    ui.add_space(6.0);

                    let text_edit = egui::TextEdit::singleline(&mut self.password_buffer)
                        .password(true)
                        .desired_width(260.0);

                    let response = ui.add(text_edit);

                    if self.password_buffer.is_empty() && !response.has_focus() {
                        response.request_focus();
                    }

                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        let has_text = !self.password_buffer.is_empty();

                        let enter_pressed = response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                        if (ui.add_enabled(has_text, egui::Button::new(btn_label)).clicked() || enter_pressed)
                            && has_text
                        {
                            if is_opening {
                                self.submit_decryption_task();
                            } else {
                                self.submit_encryption_task();
                            }
                        }

                        if ui.button("Cancel").clicked() {
                            self.cancel_password_process();
                        }
                    });
                });
            });
    }

    fn submit_encryption_task(&mut self) {
        if let (Some(dir), Some(save_path)) = (self.pending_directory.take(), self.pending_save_path.take()) {
            self.show_password_popup = false;
            self.progress = 0.0;
            self.progress_message = "Creating container...".to_string();
            self.operation_in_progress = true;
            self.state = AppState::Loading;

            let password_to_send = std::mem::take(&mut self.password_buffer);
            self.create_container_async(dir, save_path, password_to_send);
        }
    }

    fn submit_decryption_task(&mut self) {
        if let Some(file) = self.pending_open_path.take() {
            self.show_password_popup = false;
            self.progress = 0.0;
            self.progress_message = "Opening vault...".to_string();
            self.operation_in_progress = true;
            self.state = AppState::Loading;

            let password_to_send = std::mem::take(&mut self.password_buffer);
            let tx = self.tx.clone().unwrap();

            std::thread::spawn(move || {
                let _ = tx.send(ProgressMessage::StartLoading {
                    message: "Opening vault...".to_string(),
                });

                // Примечание: если VaultReader::open будет принимать пароль, передай сюда _password_to_send
                match VaultReader::open(&file, password_to_send) {
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

        std::thread::spawn(move || {
            match create_container(&dir, &save_path, &tx, password) {
                Ok(_) => {
                    let _ = tx.send(ProgressMessage::DoneEncrypting);
                }
                Err(err) => {
                    let _ = tx.send(ProgressMessage::Error {
                        error: format!("Error while creating EVFS: {}", err),
                    });
                }
            }
        });
    }

    fn render_browser_screen(&mut self, ui: &mut egui::Ui) {
        self.check_progress();

        let vault_reader = match &mut self.vault_reader {
            Some(r) => r,
            None => {
                self.state = AppState::Home;
                return;
            }
        };

        let entries = vault_reader.index.entries.clone();

        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            self.render_navigation_bar(ui);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            let (sub_dirs, current_files) = self.categorize_entries(&entries);
            self.render_file_list(ui, sub_dirs, current_files);

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            self.render_selected_file_info(ui);
        });
    }

    fn render_navigation_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("📦").size(20.0));

            if ui.small_button("Root").clicked() {
                self.current_vfs_dir = String::new();
                self.selected_file = None;
            }

            self.render_breadcrumb(ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Exit").clicked() {
                    self.vault_reader = None;
                    self.state = AppState::Home;
                }
            });
        });
    }

    fn render_breadcrumb(&mut self, ui: &mut egui::Ui) {
        let dir = self.current_vfs_dir.clone();
        let components: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();

        let mut accum = String::new();

        for comp in components {
            ui.label(egui::RichText::new("›").weak().size(16.0));
            accum = if accum.is_empty() {
                comp.to_string()
            } else {
                format!("{}/{}", accum, comp)
            };

            if ui.small_button(comp).clicked() {
                self.current_vfs_dir = accum.clone();
                self.selected_file = None;
            }
        }
    }

    fn categorize_entries(&self, entries: &[FileEntry]) -> (BTreeSet<String>, Vec<FileEntry>) {
        let mut sub_dirs = BTreeSet::new();
        let mut current_files = Vec::new();

        for entry in entries {
            let path = Path::new(&entry.path);

            if self.is_entry_in_current_dir(path) {
                current_files.push(entry.clone());
            } else if let Some(dir) = self.get_subdirectory_from_path(path) {
                sub_dirs.insert(dir);
            }
        }

        (sub_dirs, current_files)
    }

    fn is_entry_in_current_dir(&self, path: &Path) -> bool {
        if self.current_vfs_dir.is_empty() {
            path.parent() == Some(Path::new(""))
        } else {
            path.starts_with(&self.current_vfs_dir)
                && path.parent() == Some(Path::new(&self.current_vfs_dir))
        }
    }

    fn get_subdirectory_from_path(&self, path: &Path) -> Option<String> {
        let prefix = format!("{}/", self.current_vfs_dir);

        if self.current_vfs_dir.is_empty() && path.components().count() > 1 {
            return path
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
        }

        if !self.current_vfs_dir.is_empty() && path.starts_with(&prefix) {
            if let Ok(rel) = path.strip_prefix(&self.current_vfs_dir) {
                if let Some(first_comp) = rel.components().next() {
                    let comp_str = first_comp.as_os_str().to_string_lossy();
                    if rel.components().count() > 1 {
                        return Some(comp_str.into_owned());
                    }
                }
            }
        }

        None
    }

    fn render_file_list(
        &mut self,
        ui: &mut egui::Ui,
        sub_dirs: BTreeSet<String>,
        files: Vec<FileEntry>,
    ) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for dir_name in sub_dirs {
                    self.render_directory_item(ui, &dir_name);
                }

                for file_entry in files {
                    self.render_file_item(ui, &file_entry);
                }
            });
    }

    fn render_directory_item(&mut self, ui: &mut egui::Ui, dir_name: &str) {
        let resp = ui.selectable_label(false, format!("📁   {}", dir_name));
        if resp.clicked() {
            self.current_vfs_dir = if self.current_vfs_dir.is_empty() {
                dir_name.to_string()
            } else {
                format!("{}/{}", self.current_vfs_dir, dir_name)
            };
            self.selected_file = None;
        }
    }

    fn render_file_item(&mut self, ui: &mut egui::Ui, file_entry: &FileEntry) {
        let file_name = Path::new(&file_entry.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let is_selected = self.selected_file.as_ref().map(|f| f.offset) == Some(file_entry.offset);
        let resp = ui.selectable_label(is_selected, format!("📄   {}", file_name));

        if resp.clicked() {
            self.selected_file = Some(file_entry.clone());
        }

        if resp.double_clicked() {
            if let Some(reader) = &mut self.vault_reader {
                let _ = reader.open_file_in_system(file_entry);
            }
        }
    }

    fn render_selected_file_info(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            if let Some(entry) = &self.selected_file {
                self.render_file_details(ui, entry);
                ui.add_space(8.0);

                if ui
                    .add_sized([180.0, 36.0], egui::Button::new("Open"))
                    .clicked()
                {
                    if let Some(reader) = &mut self.vault_reader {
                        let _ = reader.open_file_in_system(entry);
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("The file is not selected")
                        .italics()
                        .weak(),
                );
            }
        });
    }

    fn render_file_details(&self, ui: &mut egui::Ui, entry: &FileEntry) {
        ui.label(
            egui::RichText::new("Virtual file selected")
                .strong()
                .size(16.0),
        );
        ui.label(egui::RichText::new(format!("Virtual file path: {}", entry.path)).small());
        ui.label(
            egui::RichText::new(format!("Offset: {}", entry.offset))
                .monospace()
                .small(),
        );
        ui.label(
            egui::RichText::new(format!(
                "Size: {} byte (Compressed: {} byte)",
                entry.original_size, entry.stored_size
            ))
            .small(),
        );
    }
}

impl eframe::App for FileBrowserApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.initialize_style(ui.ctx());

        self.background.update_and_draw(ui);

        self.render_custom_title_bar(ui);
        ui.add_space(10.0);

        if self.rx.is_some() {
            self.check_progress();

            if self.operation_in_progress {
                ui.ctx().request_repaint();
            }
        }

        match self.state {
            AppState::Home => self.render_home_screen(ui),
            AppState::Loading => self.render_loading_popup(ui.ctx()),
            AppState::Browser => self.render_browser_screen(ui),
        }
    }
}