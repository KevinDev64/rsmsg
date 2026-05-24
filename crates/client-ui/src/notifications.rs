use std::process::Command;

pub fn notify(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "display notification {} with title {}",
                applescript_string(&body),
                applescript_string(&title)
            );
            let _ = Command::new("osascript").arg("-e").arg(script).status();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = Command::new("notify-send").arg(title).arg(body).status();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = (title, body);
        }
    });
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
