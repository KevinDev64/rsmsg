#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub http_base: String,
    pub ws_base: String,
}

impl ClientConfig {
    pub fn local_default() -> Self {
        Self {
            http_base: "http://127.0.0.1:3000".to_string(),
            ws_base: "ws://127.0.0.1:3000".to_string(),
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
