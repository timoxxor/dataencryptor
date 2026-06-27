use egui::{Context, Window, ProgressBar};

pub struct LoadingModal<'a> {
    progress: f32,
    message: &'a str,
}

impl<'a> LoadingModal<'a> {
    pub fn new(progress: f32, message: &'a str) -> Self {
        Self { progress, message }
    }

    pub fn show(self, ctx: &Context) {
        Window::new("⏳ Processing")
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(self.message);
                    ui.add_space(8.0);
                    ui.add(ProgressBar::new(self.progress).show_percentage());
                    ui.add_space(4.0);
                });
            });
    }
}