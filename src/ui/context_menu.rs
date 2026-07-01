use egui::{self, Id, Widget};

use crate::deflate::FileEntry;

#[derive(Clone, Default)]
pub enum ContextMenuAction {
    #[default]
    None,
    Open(FileEntry),
    Properties(FileEntry),
    Rename(FileEntry),
    Delete(FileEntry),
}

pub struct ContextMenu {
    file: FileEntry,
    pos: egui::Pos2,
}

impl ContextMenu {
    pub fn new(file: FileEntry, pos: egui::Pos2) -> Self {
        Self { file, pos }
    }
}

impl Widget for ContextMenu {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        const ACTION_ID: &str = "ctx_menu_action";
        let ctx = ui.ctx();
        let file = self.file;
        let pos = self.pos;

        ctx.data_mut(|d| {
            d.insert_temp(Id::new(ACTION_ID), ContextMenuAction::None);
        });

        let area_resp = egui::Area::new(Id::new("file_context_menu"))
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

                        let (rect, response) =
                            ui.allocate_exact_size(size, egui::Sense::click());

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
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                Id::new(ACTION_ID),
                                ContextMenuAction::Open(file.clone()),
                            );
                        });
                        ui.close();
                    }

                    if item(ui, "Properties") {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                Id::new(ACTION_ID),
                                ContextMenuAction::Properties(file.clone()),
                            );
                        });
                        ui.close();
                    }

                    if item(ui, "Rename") {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                Id::new(ACTION_ID),
                                ContextMenuAction::Rename(file.clone()),
                            );
                        });
                        ui.close();
                    }

                    if item(ui, "Delete") {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                Id::new(ACTION_ID),
                                ContextMenuAction::Delete(file.clone()),
                            );
                        });
                        ui.close();
                    }
                });
            });

        if area_resp.response.clicked_elsewhere() {
            ctx.data_mut(|d| {
                d.insert_temp(Id::new(ACTION_ID), ContextMenuAction::None);
            });
        }

        area_resp.response
    }
}
