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
