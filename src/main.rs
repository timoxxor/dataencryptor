use eframe::egui;
mod crypto;
mod deflate;
mod dialog;
mod gui;
mod handler;
mod particles;
mod theme;
mod ui;
mod worker;

fn main() -> eframe::Result<()> {
    let file_to_open = std::env::args().nth(1).map(std::path::PathBuf::from);

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
        Box::new(|cc| {
            let gl = cc.gl.as_ref().expect("Need glow backend");
            Ok(Box::new(gui::FileBrowserApp::new(&cc.egui_ctx, gl, file_to_open)))
        }),
    )
}
