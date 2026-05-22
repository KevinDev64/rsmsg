use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub http_base: String,
    pub ws_base: String,
    pub session_store_path: String,
    pub key_store_path: String,
}

impl ClientConfig {
    pub fn local_default() -> Self {
        let profile = std::env::var("RSMSG_PROFILE").unwrap_or_else(|_| "default".to_string());
        Self {
            http_base: "http://127.0.0.1:3000".to_string(),
            ws_base: "ws://127.0.0.1:3000".to_string(),
            session_store_path: format!(".rsmsg_peer_sessions.{profile}.json"),
            key_store_path: format!(".rsmsg_local_keys.{profile}.json"),
        }
    }

    pub fn for_server(server: &str) -> Self {
        let mut config = Self::local_default();
        let mut http_base = server.trim().trim_end_matches('/').to_string();
        if !http_base.starts_with("http://") && !http_base.starts_with("https://") {
            http_base = format!("http://{http_base}");
        }
        let ws_base = if let Some(rest) = http_base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = http_base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            http_base.clone()
        };
        config.http_base = http_base;
        config.ws_base = ws_base;
        config
    }
}

#[derive(Clone, Debug)]
pub struct DeviceAuth {
    pub device_uuid: String,
    pub auth_token: String,
}

#[derive(Clone, Debug)]
pub struct PendingEnvelope {
    pub message_id: String,
    pub from_device_uuid: String,
    pub envelope_b64: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct DecryptedMessage {
    pub message_id: String,
    pub from_device_uuid: String,
    pub plaintext: String,
    pub created_at_unix_ms: i64,
}

#[derive(Clone, Debug)]
pub struct OutgoingMessageStatus {
    pub message_id: String,
    pub delivered: bool,
    pub read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum EncryptedMessagePayload {
    #[serde(rename = "file")]
    File {
        v: u8,
        file_name: String,
        file_size: u64,
        #[serde(default)]
        data_b64: Option<String>,
        #[serde(default)]
        blob_id: Option<String>,
        #[serde(default)]
        file_key_b64: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalDeviceKeys {
    pub identity_private_b64: String,
    pub identity_public_b64: String,
    #[serde(default)]
    pub signing_identity_private_b64: Option<String>,
    #[serde(default)]
    pub signing_identity_public_b64: Option<String>,
    pub signed_prekey_private_b64: String,
    pub signed_prekey_public_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredPeerSession {
    pub peer_device_uuid: String,
    pub shared_key_b64: String,
    #[serde(default)]
    pub send_chain_key_b64: Option<String>,
    #[serde(default)]
    pub recv_chain_key_b64: Option<String>,
    #[serde(default)]
    pub send_counter: u64,
    #[serde(default)]
    pub recv_counter: u64,
}

#[derive(Clone, Debug)]
pub struct PeerSession {
    pub shared_key_b64: String,
    pub send_chain_key_b64: String,
    pub recv_chain_key_b64: String,
    pub send_counter: u64,
    pub recv_counter: u64,
}
