use std::sync::Arc;
use std::time::Instant;

use egui::{CentralPanel, Color32, Frame, Ui};
use glow_bloom::{BloomRenderer, BloomText};

use super::types::HomeEvent;
use super::AppState;

pub struct HomeScreen<'a> {
    pub bloom_renderer: &'a Arc<egui::mutex::Mutex<BloomRenderer>>,
    pub typewriter: &'a mut Typewriter,
    pub state: &'a AppState,
}

impl<'a> HomeScreen<'a> {
    pub fn show(self, ui: &mut Ui) -> Option<HomeEvent> {
        let mut event = None;

        CentralPanel::default()
            .frame(Frame::new().fill(Color32::TRANSPARENT))
            .show_inside(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add_space(60.0);

                    let text = self.typewriter.update();
                    ui.allocate_ui(egui::vec2(ui.available_width(), 60.0), |ui| {
                        ui.add(
                            BloomText::new(Arc::clone(self.bloom_renderer), text)
                                .intensity(1.2)
                                .font_scale(36.0),
                        );
                    });

                    ui.add_space(16.0);

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

pub struct Typewriter {
    full_text: String,
    current_length: usize,
    last_char_time: Instant,
    chars_per_second: f32,
    glitch_chars: Vec<char>,
    active_glitch: Option<char>,
}

impl Typewriter {
    pub fn new(full_text: &str) -> Self {
        Self {
            full_text: full_text.to_string(),
            current_length: 0,
            last_char_time: Instant::now(),
            chars_per_second: 12.0,
            glitch_chars: vec!['&', '<', '/'],
            active_glitch: None,
        }
    }

    fn is_finished(&self) -> bool {
        self.current_length >= self.full_text.chars().count()
    }

    pub fn update(&mut self) -> String {
        let total_chars = self.full_text.chars().count();

        if !self.is_finished() {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_char_time).as_secs_f32();
            let time_per_char = 1.0 / self.chars_per_second;

            if elapsed >= time_per_char {
                let chars_to_add = (elapsed / time_per_char) as usize;
                self.current_length = (self.current_length + chars_to_add).min(total_chars);
                self.last_char_time = now;

                let time_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as usize;
                if time_ms % 3 == 0 {
                    let idx = time_ms % self.glitch_chars.len();
                    self.active_glitch = Some(self.glitch_chars[idx]);
                } else {
                    self.active_glitch = None;
                }
            }
        } else {
            self.active_glitch = None;
        }

        let mut s: String = self.full_text.chars().take(self.current_length).collect();
        if let Some(glitch) = self.active_glitch {
            s.push(glitch);
        }

        // let show_cursor = if self.is_finished() {
        //     let now = std::time::SystemTime::now()
        //         .duration_since(std::time::UNIX_EPOCH)
        //         .unwrap()
        //         .as_millis() as f64
        //         / 1000.0;
        //     (now * 2.5).floor() as i64 % 2 == 0
        // } else {
        //     true
        // };
        let show_cursor = true;
        if show_cursor {
            s.push('|');
        }

        s
    }
}
