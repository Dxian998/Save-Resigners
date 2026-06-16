#![windows_subsystem = "windows"]

mod state;
mod ui;
mod crypto;
mod processor;
mod app;

use eframe::egui;
use eframe::epaint::Vec2;
use app::Bl4SaveDataApp;

fn load_icon() -> egui::IconData {
    use ico::IconDir;
    let ico_bytes = include_bytes!("../icon.ico");
    let icon_dir = IconDir::read(std::io::Cursor::new(ico_bytes)).expect("Invalid .ico data");
    let entry = &icon_dir.entries()[0];
    let image = entry.decode().expect("Failed to decode ICO image");
    let (width, height) = (image.width(), image.height());
    let pixels = image.rgba_data().to_vec();
    egui::IconData {
        rgba: pixels,
        width,
        height,
    }
}

fn main() -> Result<(), eframe::Error> {
    let icon = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(600.0, 500.0))
            .with_resizable(false)
            .with_maximized(false)
            .with_maximize_button(false)
            .with_icon(icon),
        ..Default::default()
    };
   
    eframe::run_native(
        "Borderlands 4 Save Data Resigner",
        options,
        Box::new(|_cc| Ok(Box::new(Bl4SaveDataApp::new()))),
    )
}