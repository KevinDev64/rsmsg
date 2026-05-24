use std::sync::mpsc::{self, Receiver};

use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

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
    let size = 32_u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as i32 - 16;
            let dy = y as i32 - 16;
            let inside = dx * dx + dy * dy <= 15 * 15;
            if inside {
                rgba.extend_from_slice(&[56, 120, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, size, size).ok()
}
