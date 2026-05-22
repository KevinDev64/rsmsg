use anyhow::Result;
use crypto::CryptoEngine;
use shared::{
    DeviceLoginRequest, FetchPrekeyBundleResponse, RegisterDeviceRequest, SendMessageRequest,
    UploadPrekeysRequest, UserLoginRequest, UserRegisterRequest,
};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::{
    key_store, session_store,
    transport::ApiTransport,
    types::{
        ClientConfig, DecryptedMessage, DeviceAuth, LocalDeviceKeys, OutgoingMessageStatus,
        PendingEnvelope,
    },
};

pub struct ClientCore {
    crypto: CryptoEngine,
    transport: ApiTransport,
    session_store_path: String,
    key_store_path: String,
    peer_sessions: Mutex<HashMap<String, String>>,
}

impl ClientCore {
    pub fn new(config: ClientConfig) -> Self {
        let sessions = session_store::load(&config.session_store_path);
        Self {
            crypto: CryptoEngine::new(),
            transport: ApiTransport::new(config.clone()),
            session_store_path: config.session_store_path,
            key_store_path: config.key_store_path,
            peer_sessions: Mutex::new(sessions),
        }
    }

    pub fn healthcheck(&self) -> Result<()> {
        self.crypto.healthcheck()
    }

    pub fn generate_shared_key_b64(&self) -> String {
        self.crypto.generate_shared_key_b64()
    }

    pub fn generate_local_device_keys(&self) -> LocalDeviceKeys {
        let identity = self.crypto.generate_x25519_keypair();
        let signed = self.crypto.generate_x25519_keypair();
        LocalDeviceKeys {
            identity_private_b64: identity.private_b64,
            identity_public_b64: identity.public_b64,
            signed_prekey_private_b64: signed.private_b64,
            signed_prekey_public_b64: signed.public_b64,
        }
    }

    pub fn load_or_create_local_device_keys(&self) -> LocalDeviceKeys {
        if let Some(keys) = key_store::load(&self.key_store_path) {
            return keys;
        }
        let keys = self.generate_local_device_keys();
        let _ = key_store::save(&self.key_store_path, &keys);
        keys
    }

    pub fn build_register_request(
        &self,
        user_id: String,
        device_id: String,
        keys: &LocalDeviceKeys,
    ) -> RegisterDeviceRequest {
        RegisterDeviceRequest {
            user_id,
            device_id,
            identity_key_b64: keys.identity_public_b64.clone(),
            signed_prekey_b64: keys.signed_prekey_public_b64.clone(),
        }
    }

    pub async fn register_device(&self, req: RegisterDeviceRequest) -> Result<String> {
        let response = self.transport.register_device(req).await?;
        Ok(response.device_uuid)
    }

    pub async fn register_user(&self, user_id: String, password: String) -> Result<bool> {
        let response = self
            .transport
            .user_register(UserRegisterRequest { user_id, password })
            .await?;
        Ok(response.created)
    }

    pub async fn login_user(&self, user_id: String, password: String) -> Result<bool> {
        let response = self
            .transport
            .user_login(UserLoginRequest { user_id, password })
            .await?;
        Ok(response.ok)
    }

    pub async fn search_users(&self, query: String) -> Result<Vec<String>> {
        let response = self.transport.user_search(query).await?;
        Ok(response.users)
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

    pub async fn logout_device(&self, auth: &DeviceAuth) -> Result<bool> {
        let response = self.transport.device_logout(auth).await?;
        Ok(response.revoked)
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
        self.send_text_message_with_id(
            auth,
            to_device_uuid,
            plaintext,
            shared_key_b64,
            Uuid::new_v4().to_string(),
        )
        .await
    }

    pub async fn send_text_message_with_id(
        &self,
        auth: &DeviceAuth,
        to_device_uuid: String,
        plaintext: String,
        shared_key_b64: &str,
        message_id: String,
    ) -> Result<bool> {
        let envelope_b64 = self
            .crypto
            .encrypt_text_to_b64(shared_key_b64, &plaintext)?;
        let req = SendMessageRequest {
            message_id,
            from_device_uuid: auth.device_uuid.clone(),
            to_device_uuid,
            envelope_b64,
        };
        self.send_message(auth, req).await
    }

    pub async fn send_text_to_peer(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        plaintext: String,
    ) -> Result<bool> {
        let key = self
            .peer_sessions
            .lock()
            .expect("peer_sessions")
            .get(&peer_device_uuid)
            .cloned();
        let Some(key) = key else {
            return Ok(false);
        };
        self.send_text_message(auth, peer_device_uuid, plaintext, &key)
            .await
    }

    pub async fn send_text_to_peer_with_id(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        plaintext: String,
        message_id: String,
    ) -> Result<bool> {
        let key = self
            .peer_sessions
            .lock()
            .expect("peer_sessions")
            .get(&peer_device_uuid)
            .cloned();
        let Some(key) = key else {
            return Ok(false);
        };
        self.send_text_message_with_id(auth, peer_device_uuid, plaintext, &key, message_id)
            .await
    }

    pub fn has_peer_session(&self, peer_device_uuid: &str) -> bool {
        self.peer_sessions
            .lock()
            .expect("peer_sessions")
            .contains_key(peer_device_uuid)
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

    pub async fn message_statuses(
        &self,
        auth: &DeviceAuth,
        message_ids: Vec<String>,
    ) -> Result<Vec<OutgoingMessageStatus>> {
        let response = self.transport.message_status(auth, message_ids).await?;
        Ok(response
            .messages
            .into_iter()
            .map(|item| OutgoingMessageStatus {
                message_id: item.message_id,
                delivered: item.delivered,
                read: item.read,
            })
            .collect())
    }

    pub async fn ws_drain_once(&self, auth: &DeviceAuth) -> Result<Vec<PendingEnvelope>> {
        self.transport.ws_once(auth).await
    }

    pub async fn derive_peer_shared_key(
        &self,
        local_keys: &LocalDeviceKeys,
        peer_user_id: String,
        peer_device_id: String,
    ) -> Result<(String, FetchPrekeyBundleResponse)> {
        let bundle = self
            .transport
            .fetch_prekey_bundle(peer_user_id, peer_device_id)
            .await?;
        let key_b64 = self
            .crypto
            .derive_shared_key_b64(&local_keys.identity_private_b64, &bundle.identity_key_b64)?;
        self.peer_sessions
            .lock()
            .expect("peer_sessions")
            .insert(bundle.device_uuid.clone(), key_b64.clone());
        let _ = self.persist_sessions();
        Ok((key_b64, bundle))
    }

    pub async fn resolve_user_device(&self, user_id: String, device_id: String) -> Result<String> {
        let response = self.transport.resolve_user(user_id, device_id).await?;
        Ok(response.device_uuid)
    }

    pub async fn resolve_device_user(&self, device_uuid: String) -> Result<String> {
        let response = self.transport.resolve_device(device_uuid).await?;
        Ok(response.user_id)
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

    pub fn decrypt_pending_with_sessions(
        &self,
        pending: Vec<PendingEnvelope>,
    ) -> (Vec<DecryptedMessage>, Vec<String>) {
        let sessions = self.peer_sessions.lock().expect("peer_sessions");
        let mut out = Vec::new();
        let mut ack_ids = Vec::new();
        for item in pending {
            if let Some(key) = sessions.get(&item.from_device_uuid) {
                if let Ok(plaintext) = self.crypto.decrypt_text_from_b64(key, &item.envelope_b64) {
                    ack_ids.push(item.message_id.clone());
                    out.push(DecryptedMessage {
                        message_id: item.message_id,
                        from_device_uuid: item.from_device_uuid,
                        plaintext,
                        created_at_unix_ms: item.created_at_unix_ms,
                    });
                }
            }
        }
        (out, ack_ids)
    }

    fn persist_sessions(&self) -> Result<()> {
        let sessions = self.peer_sessions.lock().expect("peer_sessions");
        session_store::save(&self.session_store_path, &sessions)?;
        Ok(())
    }
}
