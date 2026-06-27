use egui::{Align, Layout, RichText, Sense, Vec2, ViewportCommand, Ui};

pub struct CustomTitleBar;

impl CustomTitleBar {
    pub fn show(ui: &mut Ui) {
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
}