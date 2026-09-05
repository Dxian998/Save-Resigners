#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

pub mod crypto;
pub mod app;

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    app::setup(&app);
    app.run()
}
