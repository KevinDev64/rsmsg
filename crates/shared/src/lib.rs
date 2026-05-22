use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterDeviceRequest {
    pub user_id: String,
    pub device_id: String,
    pub identity_key_b64: String,
    pub signing_identity_key_b64: String,
    pub signed_prekey_b64: String,
    pub signed_prekey_signature_b64: String,
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
    pub signing_identity_key_b64: String,
    pub signed_prekey_b64: String,
    pub signed_prekey_signature_b64: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLoginRequest {
    pub user_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLoginResponse {
    pub device_uuid: String,
    pub auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckMessageRequest {
    pub device_uuid: String,
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckMessageResponse {
    pub acked: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStatusRequest {
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStatusItem {
    pub message_id: String,
    pub delivered: bool,
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStatusResponse {
    pub messages: Vec<MessageStatusItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadBlobRequest {
    pub data_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadBlobResponse {
    pub blob_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlobResponse {
    pub blob_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendBlobChunkRequest {
    pub blob_id: String,
    pub chunk_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendBlobChunkResponse {
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchBlobRequest {
    pub blob_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchBlobResponse {
    pub data_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLogoutRequest {
    pub device_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLogoutResponse {
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisterRequest {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRegisterResponse {
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLoginRequest {
    pub user_id: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLoginResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveUserRequest {
    pub user_id: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveUserResponse {
    pub device_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveDeviceRequest {
    pub device_uuid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveDeviceResponse {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchResponse {
    pub users: Vec<String>,
}
