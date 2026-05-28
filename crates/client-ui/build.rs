#[cfg(windows)]
fn main() {
    let icon_path = "assets/logo.ico";
    if std::path::Path::new(icon_path).exists() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon_path);
        let _ = res.compile();
    }
}

#[cfg(not(windows))]
fn main() {}
