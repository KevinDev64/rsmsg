# rsmsg API Guide

```rust
pub struct DeviceHeaders {
    pub device_uuid: String,
    pub device_token: String,
}
```

```rust
pub struct RegisterDeviceRequest {
    pub user_id: String,
    pub device_id: String,
    pub identity_key_b64: String,
    pub signed_prekey_b64: String,
}
```

```rust
pub struct DeviceLoginRequest {
    pub user_id: String,
    pub device_id: String,
}

pub struct DeviceLoginResponse {
    pub device_uuid: String,
    pub auth_token: String,
}
```

```rust
pub struct UploadPrekeysRequest {
    pub device_uuid: String,
    pub prekeys: Vec<PrekeyUploadItem>,
}

pub struct PrekeyUploadItem {
    pub key_id: i32,
    pub pubkey_b64: String,
}
```

```rust
pub struct SendMessageRequest {
    pub message_id: String,
    pub from_device_uuid: String,
    pub to_device_uuid: String,
    pub envelope_b64: String,
}

pub struct SendMessageResponse {
    pub accepted: bool,
}
```

```rust
pub struct FetchPendingRequest {
    pub device_uuid: String,
    pub limit: Option<i64>,
}

pub struct PendingMessageItem {
    pub message_id: String,
    pub from_device_uuid: String,
    pub envelope_b64: String,
    pub created_at_unix_ms: i64,
}
```

```rust
pub struct AckMessageRequest {
    pub device_uuid: String,
    pub message_ids: Vec<String>,
}

pub struct AckMessageResponse {
    pub acked: u64,
}
```

```rust
pub struct DeviceLogoutRequest {
    pub device_uuid: String,
}

pub struct DeviceLogoutResponse {
    pub revoked: bool,
}
```

```rust
pub enum Endpoint {
    RegisterDevice,
    DeviceLogin,
    DeviceLogout,
    UploadPrekeys,
    FetchPrekeyBundle,
    SendMessage,
    FetchPending,
    AckMessage,
    WebSocket,
}
```

```rust
pub struct RealtimeSession {
    pub poll_interval_ms: u64,
    pub pending_batch_limit: i64,
}

impl RealtimeSession {
    pub const DEFAULT: Self = Self {
        poll_interval_ms: 1200,
        pending_batch_limit: 200,
    };
}
```
