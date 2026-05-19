use anyhow::Result;
use crypto::CryptoEngine;
use shared::{DeviceLoginRequest, RegisterDeviceRequest, SendMessageRequest, UploadPrekeysRequest};

use crate::{
    transport::ApiTransport,
    types::{ClientConfig, DeviceAuth, PendingEnvelope},
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
}
