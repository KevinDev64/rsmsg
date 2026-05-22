mod app;
mod history;
mod message_ui;

use app::MessengerApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        run_and_return: false,
        ..Default::default()
    };
    eframe::run_native(
        "rsmsg",
        options,
        Box::new(|_cc| Ok(Box::new(MessengerApp::new()))),
    )
}
