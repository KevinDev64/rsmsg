use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub user_id: String,
    pub device_id: String,
    pub identity_key_b64: String,
    pub signed_prekey_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceResponse {
    pub device_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrekeyUploadItem {
    pub key_id: i32,
    pub pubkey_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPrekeysRequest {
    pub device_uuid: String,
    pub prekeys: Vec<PrekeyUploadItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPrekeysResponse {
    pub inserted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPrekeyBundleRequest {
    pub user_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPrekeyBundleResponse {
    pub device_uuid: String,
    pub identity_key_b64: String,
    pub signed_prekey_b64: String,
    pub one_time_prekey: Option<PrekeyUploadItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub message_id: String,
    pub from_device_uuid: String,
    pub to_device_uuid: String,
    pub envelope_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPendingRequest {
    pub device_uuid: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMessageItem {
    pub message_id: String,
    pub from_device_uuid: String,
    pub envelope_b64: String,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchPendingResponse {
    pub messages: Vec<PendingMessageItem>,
}
