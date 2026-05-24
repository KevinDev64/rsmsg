mod app;
mod history;
mod localization;
mod message_ui;
mod notifications;
mod settings;
mod tray;

use app::MessengerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        run_and_return: false,
        viewport: egui::ViewportBuilder::default().with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "rsmsg",
        options,
        Box::new(|_cc| Ok(Box::new(MessengerApp::new()))),
    )
}
