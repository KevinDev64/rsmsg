use anyhow::Result;
use crypto::CryptoEngine;
use shared::{DeviceLoginRequest, RegisterDeviceRequest, SendMessageRequest, UploadPrekeysRequest};
use uuid::Uuid;

use crate::{
    transport::ApiTransport,
    types::{ClientConfig, DecryptedMessage, DeviceAuth, PendingEnvelope},
};

pub struct ClientCore {
    crypto: CryptoEngine,
    transport: ApiTransport,
}

impl ClientCore {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            crypto: CryptoEngine::new(),
            transport: ApiTransport::new(config),
        }
    }

    pub fn healthcheck(&self) -> Result<()> {
        self.crypto.healthcheck()
    }

    pub fn generate_shared_key_b64(&self) -> String {
        self.crypto.generate_shared_key_b64()
    }

    pub async fn register_device(&self, req: RegisterDeviceRequest) -> Result<String> {
        let response = self.transport.register_device(req).await?;
        Ok(response.device_uuid)
    }

    pub async fn login_device(&self, user_id: String, device_id: String) -> Result<DeviceAuth> {
        let response = self
            .transport
            .device_login(DeviceLoginRequest { user_id, device_id })
            .await?;
        Ok(DeviceAuth {
            device_uuid: response.device_uuid,
            auth_token: response.auth_token,
        })
    }

    pub async fn upload_prekeys(
        &self,
        auth: &DeviceAuth,
        req: UploadPrekeysRequest,
    ) -> Result<u64> {
        let response = self.transport.upload_prekeys(auth, req).await?;
        Ok(response.inserted)
    }

    pub async fn send_message(&self, auth: &DeviceAuth, req: SendMessageRequest) -> Result<bool> {
        let response = self.transport.send_message(auth, req).await?;
        Ok(response.accepted)
    }

    pub async fn send_text_message(
        &self,
        auth: &DeviceAuth,
        to_device_uuid: String,
        plaintext: String,
        shared_key_b64: &str,
    ) -> Result<bool> {
        let envelope_b64 = self
            .crypto
            .encrypt_text_to_b64(shared_key_b64, &plaintext)?;
        let req = SendMessageRequest {
            message_id: Uuid::new_v4().to_string(),
            from_device_uuid: auth.device_uuid.clone(),
            to_device_uuid,
            envelope_b64,
        };
        self.send_message(auth, req).await
    }

    pub async fn fetch_pending(
        &self,
        auth: &DeviceAuth,
        limit: Option<i64>,
    ) -> Result<Vec<PendingEnvelope>> {
        self.transport.fetch_pending(auth, limit).await
    }

    pub async fn ack_messages(&self, auth: &DeviceAuth, message_ids: Vec<String>) -> Result<()> {
        self.transport.ack_messages(auth, message_ids).await
    }

    pub async fn ws_drain_once(&self, auth: &DeviceAuth) -> Result<Vec<PendingEnvelope>> {
        self.transport.ws_once(auth).await
    }

    pub fn decrypt_pending(
        &self,
        pending: Vec<PendingEnvelope>,
        shared_key_b64: &str,
    ) -> Vec<DecryptedMessage> {
        pending
            .into_iter()
            .filter_map(|item| {
                self.crypto
                    .decrypt_text_from_b64(shared_key_b64, &item.envelope_b64)
                    .ok()
                    .map(|plaintext| DecryptedMessage {
                        message_id: item.message_id,
                        from_device_uuid: item.from_device_uuid,
                        plaintext,
                        created_at_unix_ms: item.created_at_unix_ms,
                    })
            })
            .collect()
    }
}
