use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    System,
    Light,
    Dark,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub theme: AppTheme,
    #[serde(default)]
    pub default_username: String,
}

impl AppSettings {
    pub fn load() -> Self {
        let file = settings_file();
        let path = Path::new(&file);
        if !path.exists() {
            return Self::default();
        }
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = fs::write(settings_file(), raw);
        }
    }
}

fn settings_file() -> String {
    let profile = std::env::var("RSMSG_PROFILE").unwrap_or_else(|_| "default".to_string());
    format!(".rsmsg_settings.{profile}.json")
}
