use egui;

pub fn apply_style(ctx: &egui::Context) {
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
