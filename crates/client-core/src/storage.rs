use std::path::PathBuf;

pub fn profile() -> String {
    std::env::var("RSMSG_PROFILE").unwrap_or_else(|_| "default".to_string())
}

pub fn profile_file(name: &str) -> PathBuf {
    let profile = profile();
    data_dir().join(format!("{name}.{profile}.json"))
}

pub fn migrated_profile_file(name: &str) -> PathBuf {
    let current = profile_file(name);
    if !current.exists() {
        let legacy = legacy_profile_file(name);
        if legacy.exists() {
            ensure_parent(&current);
            let _ = std::fs::copy(legacy, &current);
        }
    }
    current
}

pub fn legacy_profile_file(name: &str) -> PathBuf {
    let profile = profile();
    PathBuf::from(format!(".{name}.{profile}.json"))
}

pub fn ensure_parent(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
}

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RSMSG_DATA_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(dir) = std::env::var("APPDATA") {
            return PathBuf::from(dir).join("rsmsg");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("rsmsg");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(dir).join("rsmsg");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("rsmsg");
        }
    }
    PathBuf::from(".")
}
