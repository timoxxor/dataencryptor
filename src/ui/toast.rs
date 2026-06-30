use std::time::Instant;

use egui::{Area, Color32, Context, CornerRadius, FontId, Frame, Id, Order, Stroke};

#[derive(Clone, PartialEq)]
pub enum ToastType {
    Error,
    Info,
}

#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub duration: f64,
}

impl Toast {
    pub fn new(message: impl Into<String>, toast_type: ToastType) -> Self {
        Self {
            message: message.into(),
            toast_type,
            created_at: Instant::now(),
            duration: 4.0,
        }
    }

    pub fn expired(&self, now: Instant) -> bool {
        (now - self.created_at).as_secs_f64() > self.duration
    }
}

pub struct ToastManager {
    pub toasts: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self { toasts: Vec::new() }
    }

    pub fn error(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast::new(message, ToastType::Error));
    }

    pub fn info(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast::new(message, ToastType::Info));
    }

    pub fn show(&mut self, ctx: &Context) {
        const FADE_IN: f32 = 0.25;
        const FADE_OUT: f32 = 1.0;
        const WIDTH: f32 = 320.0;
        const HEIGHT: f32 = 42.0;
        const SPACING: f32 = 8.0;

        let now = Instant::now();
        self.toasts.retain(|t| !t.expired(now));

        if self.toasts.is_empty() {
            return;
        }

        let screen = ctx.content_rect();

        for (i, toast) in self.toasts.iter().enumerate() {
            let age = (now - toast.created_at).as_secs_f32();

            let alpha = if age < FADE_IN {
                age / FADE_IN
            } else if age > toast.duration as f32 - FADE_OUT {
                let t = (age - (toast.duration as f32 - FADE_OUT)) / FADE_OUT;
                1.0 - t
            } else {
                1.0
            }
            .clamp(0.0, 1.0);

            let y_anim = if age < FADE_IN {
                egui::lerp(-20.0..=0.0, age / FADE_IN)
            } else if age > toast.duration as f32 - FADE_OUT {
                let t = (age - (toast.duration as f32 - FADE_OUT)) / FADE_OUT;
                egui::lerp(0.0..=20.0, t)
            } else {
                0.0
            };

            let x = screen.right() - WIDTH - 12.0;
            let y = screen.top() + 42.0 + i as f32 * (HEIGHT + SPACING) + y_anim;

            let (bg, border, icon, text_color) = match toast.toast_type {
                ToastType::Error => (
                    Color32::from_rgba_unmultiplied(50, 15, 15, (220.0 * alpha) as u8),
                    Color32::from_rgba_unmultiplied(220, 60, 60, (200.0 * alpha) as u8),
                    "⚠",
                    Color32::from_rgba_unmultiplied(255, 220, 220, (255.0 * alpha) as u8),
                ),
                ToastType::Info => (
                    Color32::from_rgba_unmultiplied(15, 45, 20, (220.0 * alpha) as u8),
                    Color32::from_rgba_unmultiplied(50, 190, 70, (200.0 * alpha) as u8),
                    "✓",
                    Color32::from_rgba_unmultiplied(200, 255, 210, (255.0 * alpha) as u8),
                ),
            };

            Area::new(Id::new(("toast", i)))
                .order(Order::Foreground)
                .fixed_pos(egui::pos2(x, y))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.set_width(WIDTH);

                    Frame::new()
                        .fill(bg)
                        .corner_radius(CornerRadius::same(6))
                        .stroke(Stroke::new(1.0_f32, border))
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(icon)
                                        .font(FontId::proportional(16.0))
                                        .color(border),
                                );

                                ui.label(
                                    egui::RichText::new(&toast.message)
                                        .font(FontId::proportional(14.0))
                                        .color(text_color),
                                );
                            });
                        });
                });
        }

        ctx.request_repaint();
    }
}
