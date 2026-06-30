use std::path::Path;

use egui::{Color32, CornerRadius, Frame, Margin, Ui};

use crate::deflate::FileEntry;

const CONTENT_BG: Color32 = Color32::from_gray(42);

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.2} {}", UNITS[i])
    }
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .map(|s| s.to_str().unwrap_or(path))
        .unwrap_or(path)
}

fn file_ext(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("(none)")
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    General,
    Security,
    Storage,
    Advanced,
}

pub struct PropertiesDialog {
    pub file: Option<FileEntry>,
    current_tab: Tab,
}

fn tab(ui: &mut Ui, selected: bool, text: &str, width: f32) -> bool {
    let height = 28.0;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let bg = if selected {
        CONTENT_BG
    } else if response.hovered() {
        Color32::from_gray(35)
    } else {
        Color32::from_gray(28)
    };

    ui.painter().rect_filled(rect, 0.0, bg);

    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );

    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        if selected {
            Color32::WHITE
        } else {
            Color32::from_gray(160)
        },
    );

    response.clicked()
}

impl PropertiesDialog {
    pub fn new() -> Self {
        Self {
            file: None,
            current_tab: Tab::General,
        }
    }

    pub fn open(&mut self, file: FileEntry) {
        self.file = Some(file);
        self.current_tab = Tab::General;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.file = None;
        }

        let file = match self.file.as_ref() {
            Some(f) => f.clone(),
            None => return,
        };

        let mut open = true;

        egui::Window::new("Properties")
            .id(egui::Id::new("properties_dialog"))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.viewport_rect().center())
            .fixed_size([360.0, 260.0])
            .resizable(false)
            .collapsible(false)
            .frame(Frame {
                fill: ctx.global_style().visuals.window_fill(),
                stroke: ctx.global_style().visuals.window_stroke(),
                corner_radius: CornerRadius::ZERO,
                shadow: egui::epaint::Shadow::default(),
                ..Default::default()
            })
            .open(&mut open)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                let tab_width = ui.available_width() / 4.0;

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    if tab(ui, self.current_tab == Tab::General, "General", tab_width) {
                        self.current_tab = Tab::General;
                    }
                    if tab(ui, self.current_tab == Tab::Security, "Security", tab_width) {
                        self.current_tab = Tab::Security;
                    }
                    if tab(ui, self.current_tab == Tab::Storage, "Storage", tab_width) {
                        self.current_tab = Tab::Storage;
                    }
                    if tab(ui, self.current_tab == Tab::Advanced, "Advanced", tab_width) {
                        self.current_tab = Tab::Advanced;
                    }
                });

                let remaining = ui.available_rect_before_wrap();

                ui.allocate_ui_with_layout(
                    remaining.size(),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        ui.painter()
                            .rect_filled(ui.max_rect(), 0.0, CONTENT_BG);

                        Frame {
                            fill: CONTENT_BG,
                            inner_margin: Margin::ZERO,
                            corner_radius: CornerRadius::ZERO,
                            ..Default::default()
                        }
                        .show(ui, |ui| {
                            ui.set_min_height(remaining.height());

                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);

                                match self.current_tab {
                                    Tab::General => show_general(ui, &file),
                                    Tab::Security => show_security(ui, &file),
                                    Tab::Storage => show_storage(ui, &file),
                                    Tab::Advanced => show_advanced(ui, &file),
                                }
                            });
                        });
                    },
                );
            });

        if !open {
            self.file = None;
        }
    }
}

fn show_general(ui: &mut Ui, file: &FileEntry) {
    prop(ui, "Name:", file_name(&file.path));
    prop(ui, "Path:", &file.path);
    prop(ui, "Type:", file_ext(&file.path));
    prop(ui, "Size:", &format_size(file.original_size));
    prop(ui, "Size (bytes):", &file.original_size.to_string());
}

fn show_security(ui: &mut Ui, _file: &FileEntry) {
    prop(ui, "Encryption:", "AES-256-GCM");
    prop(ui, "Key size:", "256-bit");
    prop(ui, "Master KDF:", "Argon2id");
    prop(ui, "File KDF:", "HKDF-SHA256");
    prop(ui, "Per-file key:", "Yes (HKDF derived)");
    prop(ui, "Authentication:", "GCM MAC (128-bit)");
    prop(ui, "Integrity:", "Verified on read");
}

fn show_storage(ui: &mut Ui, file: &FileEntry) {
    let compressed = if file.compressed { "Yes" } else { "No" };
    prop(ui, "Compressed:", compressed);
    prop(ui, "Stored size:", &format_size(file.stored_size));
    prop(ui, "Original size:", &format_size(file.original_size));
    prop(ui, "Stored (bytes):", &file.stored_size.to_string());
    prop(ui, "Original (bytes):", &file.original_size.to_string());

    if file.compressed && file.original_size > 0 {
        let ratio =
            100.0 - (file.stored_size as f64 / file.original_size as f64) * 100.0;
        prop(ui, "Compression ratio:", &format!("{ratio:.1}% saved"));
    }

    prop(ui, "Storage offset:", &file.offset.to_string());
}

fn show_advanced(ui: &mut Ui, file: &FileEntry) {
    let uuid = uuid::Uuid::from_bytes_le(file.id);
    prop(ui, "UUID:", &uuid.to_string());
    prop(ui, "Container:", "EVFS v2");
}

fn prop(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).strong());
        ui.label(value);
    });
}
