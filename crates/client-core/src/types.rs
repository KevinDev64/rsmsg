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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalDeviceKeys {
    pub identity_private_b64: String,
    pub identity_public_b64: String,
    pub signed_prekey_private_b64: String,
    pub signed_prekey_public_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredPeerSession {
    pub peer_device_uuid: String,
    pub shared_key_b64: String,
}
