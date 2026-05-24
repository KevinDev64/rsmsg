#[cfg(target_os = "macos")]
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
        #[cfg(not(target_os = "macos"))]
        {
            let _ = notify_rust::Notification::new()
                .appname("rsmsg")
                .summary(&title)
                .body(&body)
                .show();
        }
    });
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}
