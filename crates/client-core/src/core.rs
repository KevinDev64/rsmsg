use anyhow::Result;
use crypto::CryptoEngine;
use serde::{Deserialize, Serialize};
use shared::{
    CallSignalItem, DeviceLoginRequest, FetchPrekeyBundleResponse, RegisterDeviceRequest,
    SendCallSignalRequest, SendMessageRequest, UploadPrekeysRequest, UserLoginRequest,
    UserRegisterRequest,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::{
    key_store, session_store,
    transport::ApiTransport,
    types::{
        ClientConfig, DecryptedMessage, DeviceAuth, EncryptedMessagePayload, LocalDeviceKeys,
        OutgoingMessageStatus, PeerSession, PendingEnvelope,
    },
};

#[derive(Serialize, Deserialize)]
struct RatchetEnvelope {
    v: u8,
    counter: u64,
    envelope_b64: String,
}

#[derive(Clone)]
pub struct ClientCore {
    crypto: CryptoEngine,
    transport: ApiTransport,
    session_store_path: String,
    key_store_path: String,
    local_password: Arc<Mutex<Option<String>>>,
    peer_sessions: Arc<Mutex<HashMap<String, PeerSession>>>,
}

impl ClientCore {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            crypto: CryptoEngine::new(),
            transport: ApiTransport::new(config.clone()),
            session_store_path: config.session_store_path,
            key_store_path: config.key_store_path,
            local_password: Arc::new(Mutex::new(None)),
            peer_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn unlock_local_storage(&self, password: String) {
        let sessions = session_store::load(&self.session_store_path, Some(&password));
        *self.local_password.lock().expect("local_password") = Some(password);
        *self.peer_sessions.lock().expect("peer_sessions") = sessions;
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
        let signing = self.crypto.generate_ed25519_keypair();
        LocalDeviceKeys {
            identity_private_b64: identity.private_b64,
            identity_public_b64: identity.public_b64,
            signing_identity_private_b64: Some(signing.private_b64),
            signing_identity_public_b64: Some(signing.public_b64),
            signed_prekey_private_b64: signed.private_b64,
            signed_prekey_public_b64: signed.public_b64,
        }
    }

    pub fn load_or_create_local_device_keys(&self) -> LocalDeviceKeys {
        let password = self.local_password.lock().expect("local_password").clone();
        if let Some(mut keys) = key_store::load(&self.key_store_path, password.as_deref()) {
            if keys.signing_identity_private_b64.is_none()
                || keys.signing_identity_public_b64.is_none()
            {
                let signing = self.crypto.generate_ed25519_keypair();
                keys.signing_identity_private_b64 = Some(signing.private_b64);
                keys.signing_identity_public_b64 = Some(signing.public_b64);
                let _ = key_store::save(&self.key_store_path, &keys, password.as_deref());
            }
            let _ = key_store::save(&self.key_store_path, &keys, password.as_deref());
            return keys;
        }
        let keys = self.generate_local_device_keys();
        let _ = key_store::save(&self.key_store_path, &keys, password.as_deref());
        keys
    }

    pub fn build_register_request(
        &self,
        user_id: String,
        device_id: String,
        keys: &LocalDeviceKeys,
    ) -> Result<RegisterDeviceRequest> {
        let signing_private = keys
            .signing_identity_private_b64
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing signing identity private key"))?;
        let signing_public = keys
            .signing_identity_public_b64
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("missing signing identity public key"))?;
        let signature = self
            .crypto
            .sign_prekey_b64(signing_private, &keys.signed_prekey_public_b64)?;
        Ok(RegisterDeviceRequest {
            user_id,
            device_id,
            identity_key_b64: keys.identity_public_b64.clone(),
            signing_identity_key_b64: signing_public.to_string(),
            signed_prekey_b64: keys.signed_prekey_public_b64.clone(),
            signed_prekey_signature_b64: signature,
        })
    }

    pub async fn register_device(&self, req: RegisterDeviceRequest) -> Result<String> {
        let response = self.transport.register_device(req).await?;
        Ok(response.device_uuid)
    }

    pub async fn register_user(
        &self,
        user_id: String,
        password: String,
        invite_code: String,
    ) -> Result<bool> {
        let response = self
            .transport
            .user_register(UserRegisterRequest {
                user_id,
                password,
                invite_code,
            })
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

    pub async fn is_user_online(&self, user_id: String, device_id: String) -> Result<bool> {
        let response = self.transport.user_online(user_id, device_id).await?;
        Ok(response.online)
    }

    pub async fn block_user(&self, auth: &DeviceAuth, user_id: String) -> Result<bool> {
        let response = self.transport.block_user(auth, user_id).await?;
        Ok(response.blocked)
    }

    pub async fn unblock_user(&self, auth: &DeviceAuth, user_id: String) -> Result<bool> {
        let response = self.transport.unblock_user(auth, user_id).await?;
        Ok(response.unblocked)
    }

    pub async fn blocked_users(&self, auth: &DeviceAuth) -> Result<Vec<String>> {
        let response = self.transport.blocked_users(auth).await?;
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
        self.send_text_message(auth, peer_device_uuid, plaintext, &key.shared_key_b64)
            .await
    }

    pub async fn send_text_to_peer_with_id(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        plaintext: String,
        message_id: String,
    ) -> Result<bool> {
        let envelope_b64 = {
            let mut sessions = self.peer_sessions.lock().expect("peer_sessions");
            let Some(session) = sessions.get_mut(&peer_device_uuid) else {
                return Ok(false);
            };
            let (message_key, next_chain_key) =
                self.crypto.ratchet_step_b64(&session.send_chain_key_b64)?;
            let inner = self.crypto.encrypt_text_to_b64(&message_key, &plaintext)?;
            let envelope = RatchetEnvelope {
                v: 2,
                counter: session.send_counter,
                envelope_b64: inner,
            };
            session.send_chain_key_b64 = next_chain_key;
            session.send_counter += 1;
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                serde_json::to_vec(&envelope)?,
            )
        };
        let req = SendMessageRequest {
            message_id,
            from_device_uuid: auth.device_uuid.clone(),
            to_device_uuid: peer_device_uuid,
            envelope_b64,
        };
        let sent = self.send_message(auth, req).await;
        let _ = self.persist_sessions();
        sent
    }

    pub async fn send_file_to_peer_with_id(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        file_name: String,
        data: Vec<u8>,
        message_id: String,
    ) -> Result<bool> {
        let payload = EncryptedMessagePayload::File {
            v: 1,
            file_name,
            file_size: data.len() as u64,
            data_b64: Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                data,
            )),
            blob_id: None,
            file_key_b64: None,
        };
        let plaintext = serde_json::to_string(&payload)?;
        self.send_text_to_peer_with_id(auth, peer_device_uuid, plaintext, message_id)
            .await
    }

    pub async fn send_file_blob_to_peer_with_id(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        file_name: String,
        data: Vec<u8>,
        message_id: String,
    ) -> Result<bool> {
        let file_key_b64 = self.crypto.generate_shared_key_b64();
        let encrypted_blob = self.crypto.encrypt_bytes(&file_key_b64, &data)?;
        let blob_id = self.upload_blob_chunked(auth, encrypted_blob).await?;
        let payload = EncryptedMessagePayload::File {
            v: 2,
            file_name,
            file_size: data.len() as u64,
            data_b64: None,
            blob_id: Some(blob_id),
            file_key_b64: Some(file_key_b64),
        };
        let plaintext = serde_json::to_string(&payload)?;
        self.send_text_to_peer_with_id(auth, peer_device_uuid, plaintext, message_id)
            .await
    }

    pub async fn send_call_invite_to_peer_with_id(
        &self,
        auth: &DeviceAuth,
        peer_device_uuid: String,
        call_id: String,
        video: bool,
        message_id: String,
    ) -> Result<bool> {
        let payload = EncryptedMessagePayload::Call {
            v: 1,
            call_id,
            video,
        };
        let plaintext = serde_json::to_string(&payload)?;
        self.send_text_to_peer_with_id(auth, peer_device_uuid, plaintext, message_id)
            .await
    }

    pub async fn fetch_file_blob(
        &self,
        auth: &DeviceAuth,
        blob_id: String,
        file_key_b64: String,
    ) -> Result<Vec<u8>> {
        let blob = self.transport.fetch_blob_bytes(auth, blob_id).await?;
        self.crypto.decrypt_bytes(&file_key_b64, &blob)
    }

    async fn upload_blob_chunked(
        &self,
        auth: &DeviceAuth,
        encrypted_blob: Vec<u8>,
    ) -> Result<String> {
        const CHUNK_SIZE: usize = 256 * 1024;
        let created = self.transport.create_blob(auth).await?;
        for chunk in encrypted_blob.chunks(CHUNK_SIZE) {
            self.transport
                .append_blob_chunk(
                    auth,
                    created.blob_id.clone(),
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, chunk),
                )
                .await?;
        }
        Ok(created.blob_id)
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

    pub async fn send_call_signal(
        &self,
        auth: &DeviceAuth,
        call_id: String,
        to_device_uuid: String,
        kind: String,
        payload: String,
    ) -> Result<bool> {
        let response = self
            .transport
            .send_call_signal(
                auth,
                SendCallSignalRequest {
                    call_id,
                    from_device_uuid: auth.device_uuid.clone(),
                    to_device_uuid,
                    kind,
                    payload,
                },
            )
            .await?;
        Ok(response.accepted)
    }

    pub async fn fetch_call_signals(
        &self,
        auth: &DeviceAuth,
        call_id: Option<String>,
    ) -> Result<Vec<CallSignalItem>> {
        let response = self
            .transport
            .fetch_call_signals(auth, call_id, Some(100))
            .await?;
        Ok(response.signals)
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
        self.crypto.verify_prekey_signature_b64(
            &bundle.signing_identity_key_b64,
            &bundle.signed_prekey_b64,
            &bundle.signed_prekey_signature_b64,
        )?;
        let key_b64 = self
            .crypto
            .derive_shared_key_b64(&local_keys.identity_private_b64, &bundle.identity_key_b64)?;
        self.peer_sessions
            .lock()
            .expect("peer_sessions")
            .entry(bundle.device_uuid.clone())
            .or_insert_with(|| PeerSession {
                shared_key_b64: key_b64.clone(),
                send_chain_key_b64: key_b64.clone(),
                recv_chain_key_b64: key_b64.clone(),
                send_counter: 0,
                recv_counter: 0,
            });
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
        let mut sessions = self.peer_sessions.lock().expect("peer_sessions");
        let mut out = Vec::new();
        let mut ack_ids = Vec::new();
        for item in pending {
            if let Some(session) = sessions.get_mut(&item.from_device_uuid) {
                if let Ok(plaintext) =
                    decrypt_with_session(&self.crypto, session, &item.envelope_b64)
                {
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
        drop(sessions);
        let _ = self.persist_sessions();
        (out, ack_ids)
    }

    fn persist_sessions(&self) -> Result<()> {
        let sessions = self.peer_sessions.lock().expect("peer_sessions");
        let password = self.local_password.lock().expect("local_password").clone();
        session_store::save(&self.session_store_path, &sessions, password.as_deref())?;
        Ok(())
    }
}

fn decrypt_with_session(
    crypto: &CryptoEngine,
    session: &mut PeerSession,
    envelope_b64: &str,
) -> Result<String> {
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, envelope_b64)?;
    if let Ok(envelope) = serde_json::from_slice::<RatchetEnvelope>(&decoded) {
        if envelope.v == 2 {
            while session.recv_counter < envelope.counter {
                let (_skipped, next_chain_key) =
                    crypto.ratchet_step_b64(&session.recv_chain_key_b64)?;
                session.recv_chain_key_b64 = next_chain_key;
                session.recv_counter += 1;
            }
            if session.recv_counter != envelope.counter {
                return Err(anyhow::anyhow!("stale ratchet message"));
            }
            let (message_key, next_chain_key) =
                crypto.ratchet_step_b64(&session.recv_chain_key_b64)?;
            let plaintext = crypto.decrypt_text_from_b64(&message_key, &envelope.envelope_b64)?;
            session.recv_chain_key_b64 = next_chain_key;
            session.recv_counter += 1;
            return Ok(plaintext);
        }
    }
    crypto.decrypt_text_from_b64(&session.shared_key_b64, envelope_b64)
}
