use egui::{CentralPanel, Color32, Frame, Ui};

use crate::gif_player::GifPlayer;

use super::event::HomeEvent;
use super::AppState;

pub struct HomeScreen<'a> {
    pub gif_player: &'a mut Option<GifPlayer>,
    pub state: &'a AppState,
}

impl<'a> HomeScreen<'a> {
    pub fn show(self, ui: &mut Ui) -> Option<HomeEvent> {
        let mut event = None;

        CentralPanel::default()
            .frame(Frame::new().fill(Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(40.0);

                    if let Some(player) = self.gif_player {
                        player.render(ui);
                    }

                    ui.add_space(15.0);

                    Frame::group(ui.style())
                        .fill(Color32::from_rgba_premultiplied(30, 30, 30, 150))
                        .corner_radius(12.0)
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.allocate_ui(egui::vec2(280.0, 0.0), |ui| {
                                ui.vertical_centered(|ui| {
                                    let btn = ui.add_sized(
                                        [260.0, 45.0],
                                        egui::Button::new("📁 Encrypt folder"),
                                    );
                                    if btn.clicked() && *self.state != AppState::Loading {
                                        event = Some(HomeEvent::EncryptFolder);
                                    }

                                    ui.add_space(10.0);

                                    let btn = ui.add_sized(
                                        [260.0, 45.0],
                                        egui::Button::new("🔐 Open EVFS(.enc)"),
                                    );
                                    if btn.clicked() && *self.state != AppState::Loading {
                                        event = Some(HomeEvent::OpenVault);
                                    }
                                });
                            });
                        });
                });
            });

        event
    }
}
