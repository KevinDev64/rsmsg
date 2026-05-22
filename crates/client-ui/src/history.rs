use std::{collections::BTreeMap, fs, path::Path};

use client_core::local_vault;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub outgoing: bool,
    pub text: String,
    pub ts: i64,
    #[serde(default)]
    pub status: MessageStatus,
    #[serde(default)]
    pub message_id: Option<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Read,
    Failed,
}

impl Default for MessageStatus {
    fn default() -> Self {
        Self::Sent
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChatHistory {
    #[serde(default)]
    pub chats: BTreeMap<String, Vec<ChatMessage>>,
    #[serde(default)]
    pub peer_by_device_uuid: BTreeMap<String, String>,
    #[serde(default)]
    pub device_uuid_by_peer: BTreeMap<String, String>,
    #[serde(default)]
    pub unread_by_peer: BTreeMap<String, u32>,
    #[serde(default)]
    pub peer_identity_key_by_peer: BTreeMap<String, String>,
    #[serde(default)]
    pub peer_signing_identity_key_by_peer: BTreeMap<String, String>,
}

impl ChatHistory {
    pub fn load(password: Option<&str>) -> Self {
        let file = history_file();
        let path = Path::new(&file);
        if !path.exists() {
            return Self::default();
        }
        if let Some(history) = local_vault::load_json::<Self>(&file, password) {
            return history;
        }
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self, password: Option<&str>) {
        let _ = local_vault::save_json(&history_file(), self, password);
    }
}

fn history_file() -> String {
    let profile = std::env::var("RSMSG_PROFILE").unwrap_or_else(|_| "default".to_string());
    format!(".rsmsg_chat_history.{profile}.json")
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let Ok(dur) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    (dur.as_secs() as i64) * 1000 + (dur.subsec_millis() as i64)
}
