use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTheme {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppLanguage {
    System,
    English,
    Russian,
}

impl AppLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::System => system_language_code(),
            Self::English => "en",
            Self::Russian => "ru",
        }
    }
}

impl Default for AppLanguage {
    fn default() -> Self {
        Self::System
    }
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
    pub language: AppLanguage,
    #[serde(default)]
    pub default_username: String,
    #[serde(default = "default_media_device")]
    pub microphone: String,
    #[serde(default = "default_media_device")]
    pub camera: String,
    #[serde(default = "default_ice_servers")]
    pub ice_servers: String,
    #[serde(default)]
    pub turn_username: String,
    #[serde(default)]
    pub turn_password: String,
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

fn system_language_code() -> &'static str {
    let language = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default()
        .to_lowercase();
    if language.starts_with("ru") {
        "ru"
    } else {
        "en"
    }
}

fn default_media_device() -> String {
    crate::media::SYSTEM_DEFAULT_DEVICE.to_string()
}

fn default_ice_servers() -> String {
    "stun:stun.l.google.com:19302".to_string()
}
