pub fn notify(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        let _ = notify_rust::Notification::new()
            .appname("rsmsg")
            .summary(&title)
            .body(&body)
            .show();
    });
}
