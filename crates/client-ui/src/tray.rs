use std::sync::{
    Arc,
    mpsc::{self, Receiver},
};

use image::imageops::FilterType;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

const LOGO_PNG: &[u8] = include_bytes!("../assets/logo.png");

pub enum TrayCommand {
    Show,
    Quit,
}

pub struct AppTray {
    _tray: TrayIcon,
    rx: Receiver<TrayCommand>,
}

impl AppTray {
    pub fn new() -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        let menu = Menu::new();
        let show = MenuItem::new("Show rsmsg", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append(&show).ok()?;
        menu.append(&quit).ok()?;

        let show_id = show.id().clone();
        let quit_id = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                let _ = tx.send(TrayCommand::Show);
            } else if event.id == quit_id {
                let _ = tx.send(TrayCommand::Quit);
            }
        }));

        let icon = icon()?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("rsmsg")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .build()
            .ok()?;
        Some(Self { _tray: tray, rx })
    }

    pub fn try_recv(&self) -> Option<TrayCommand> {
        self.rx.try_recv().ok()
    }
}

fn icon() -> Option<Icon> {
    let (rgba, width, height) = icon_rgba(Some(32))?;
    Icon::from_rgba(rgba, width, height).ok()
}

pub fn app_icon() -> Arc<egui::IconData> {
    let (rgba, width, height) = icon_rgba(None).unwrap_or_else(fallback_icon_rgba);
    Arc::new(egui::IconData {
        rgba,
        width,
        height,
    })
}

fn icon_rgba(size: Option<u32>) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory(LOGO_PNG).ok()?;
    let image = match size {
        Some(size) => image.resize_exact(size, size, FilterType::Lanczos3),
        None => image,
    };
    let width = image.width();
    let height = image.height();
    Some((image.into_rgba8().into_raw(), width, height))
}

fn fallback_icon_rgba() -> (Vec<u8>, u32, u32) {
    let size = 32_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let inside = dx * dx + dy * dy <= 15 * 15;
            rgba.extend_from_slice(if inside {
                &[233, 87, 43, 255]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    (rgba, size, size)
}
