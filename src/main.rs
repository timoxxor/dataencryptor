use eframe::egui;
mod crypto;
mod deflate;
mod gui;
mod particles;
mod gif_player;
mod ui;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EVFS - Encrypted Virtual File System")
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([700.0, 500.0])
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "EVFS App",
        options,
        Box::new(|cc| Ok(Box::new(gui::FileBrowserApp::new(&cc.egui_ctx)))),
    )
}
