use std::{fs, path::PathBuf};

use client_core::storage;

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
    #[serde(default)]
    pub server_url: String,
    #[serde(default = "default_media_device")]
    pub microphone: String,
    #[serde(default = "default_media_device")]
    pub speaker: String,
    #[serde(default = "default_media_device")]
    pub camera: String,
    #[serde(default = "default_ice_servers")]
    pub ice_servers: String,
    #[serde(default)]
    pub turn_username: String,
    #[serde(default)]
    pub turn_password: String,
    #[serde(default = "default_true")]
    pub noise_suppression: bool,
    #[serde(default = "default_true")]
    pub automatic_gain_control: bool,
    #[serde(default)]
    pub show_call_debug_info: bool,
}

impl AppSettings {
    pub fn load() -> Self {
        let file = settings_file();
        let raw = fs::read_to_string(&file)
            .or_else(|_| fs::read_to_string(legacy_settings_file()))
            .unwrap_or_default();
        if raw.is_empty() {
            return Self::default();
        }
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let file = settings_file();
            storage::ensure_parent(&file);
            let _ = fs::write(file, raw);
        }
    }
}

fn settings_file() -> PathBuf {
    storage::profile_file("rsmsg_settings")
}

fn legacy_settings_file() -> PathBuf {
    storage::legacy_profile_file("rsmsg_settings")
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

fn default_true() -> bool {
    true
}
