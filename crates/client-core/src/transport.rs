use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue};
use shared::{
    AckMessageRequest, AppendBlobChunkRequest, AppendBlobChunkResponse, CreateBlobResponse,
    DeviceLoginRequest, DeviceLoginResponse, DeviceLogoutRequest, DeviceLogoutResponse,
    FetchBlobRequest, FetchPendingRequest, FetchPendingResponse, FetchPrekeyBundleRequest,
    FetchPrekeyBundleResponse, MessageStatusRequest, MessageStatusResponse, RegisterDeviceRequest,
    RegisterDeviceResponse, ResolveDeviceRequest, ResolveDeviceResponse, ResolveUserRequest,
    ResolveUserResponse, SendMessageRequest, SendMessageResponse, UploadPrekeysRequest,
    UploadPrekeysResponse, UserLoginRequest, UserLoginResponse, UserRegisterRequest,
    UserRegisterResponse, UserSearchRequest, UserSearchResponse,
};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::types::{ClientConfig, DeviceAuth, PendingEnvelope};

#[derive(Clone)]
pub struct ApiTransport {
    client: reqwest::Client,
    cfg: ClientConfig,
}

impl ApiTransport {
    pub fn new(cfg: ClientConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            cfg,
        }
    }

    pub async fn register_device(
        &self,
        req: RegisterDeviceRequest,
    ) -> Result<RegisterDeviceResponse> {
        let url = format!("{}/v1/register_device", self.cfg.http_base);
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("register_device failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn user_register(&self, req: UserRegisterRequest) -> Result<UserRegisterResponse> {
        let url = format!("{}/v1/user_register", self.cfg.http_base);
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|value| value.get("error")?.as_str().map(ToString::to_string))
                .unwrap_or_else(|| format!("user_register failed with {status}"));
            return Err(anyhow!(message));
        }
        Ok(response.json().await?)
    }

    pub async fn user_login(&self, req: UserLoginRequest) -> Result<UserLoginResponse> {
        let url = format!("{}/v1/user_login", self.cfg.http_base);
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("user_login failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn user_search(&self, query: String) -> Result<UserSearchResponse> {
        let url = format!("{}/v1/user_search", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .json(&UserSearchRequest { query })
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("user_search failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn device_login(&self, req: DeviceLoginRequest) -> Result<DeviceLoginResponse> {
        let url = format!("{}/v1/device_login", self.cfg.http_base);
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("device_login failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn device_logout(&self, auth: &DeviceAuth) -> Result<DeviceLogoutResponse> {
        let url = format!("{}/v1/device_logout", self.cfg.http_base);
        let req = DeviceLogoutRequest {
            device_uuid: auth.device_uuid.clone(),
        };
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("device_logout failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn upload_prekeys(
        &self,
        auth: &DeviceAuth,
        req: UploadPrekeysRequest,
    ) -> Result<UploadPrekeysResponse> {
        let url = format!("{}/v1/upload_prekeys", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("upload_prekeys failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn send_message(
        &self,
        auth: &DeviceAuth,
        req: SendMessageRequest,
    ) -> Result<SendMessageResponse> {
        let url = format!("{}/v1/send_message", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("send_message failed with {status}: {body}"));
        }
        Ok(response.json().await?)
    }

    pub async fn fetch_prekey_bundle(
        &self,
        user_id: String,
        device_id: String,
    ) -> Result<FetchPrekeyBundleResponse> {
        let url = format!("{}/v1/fetch_prekey_bundle", self.cfg.http_base);
        let req = FetchPrekeyBundleRequest { user_id, device_id };
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "fetch_prekey_bundle failed with {}",
                response.status()
            ));
        }
        Ok(response.json().await?)
    }

    pub async fn resolve_user(
        &self,
        user_id: String,
        device_id: String,
    ) -> Result<ResolveUserResponse> {
        let url = format!("{}/v1/resolve_user", self.cfg.http_base);
        let req = ResolveUserRequest { user_id, device_id };
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("resolve_user failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn resolve_device(&self, device_uuid: String) -> Result<ResolveDeviceResponse> {
        let url = format!("{}/v1/resolve_device", self.cfg.http_base);
        let req = ResolveDeviceRequest { device_uuid };
        let response = self.client.post(url).json(&req).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("resolve_device failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn fetch_pending(
        &self,
        auth: &DeviceAuth,
        limit: Option<i64>,
    ) -> Result<Vec<PendingEnvelope>> {
        let url = format!("{}/v1/fetch_pending", self.cfg.http_base);
        let req = FetchPendingRequest {
            device_uuid: auth.device_uuid.clone(),
            limit,
        };
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("fetch_pending failed with {}", response.status()));
        }
        let payload: FetchPendingResponse = response.json().await?;
        Ok(payload
            .messages
            .into_iter()
            .map(|m| PendingEnvelope {
                message_id: m.message_id,
                from_device_uuid: m.from_device_uuid,
                envelope_b64: m.envelope_b64,
                created_at_unix_ms: m.created_at_unix_ms,
            })
            .collect())
    }

    pub async fn ack_messages(&self, auth: &DeviceAuth, message_ids: Vec<String>) -> Result<()> {
        let url = format!("{}/v1/ack_message", self.cfg.http_base);
        let req = AckMessageRequest {
            device_uuid: auth.device_uuid.clone(),
            message_ids,
        };
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("ack_message failed with {}", response.status()));
        }
        Ok(())
    }

    pub async fn message_status(
        &self,
        auth: &DeviceAuth,
        message_ids: Vec<String>,
    ) -> Result<MessageStatusResponse> {
        let url = format!("{}/v1/message_status", self.cfg.http_base);
        let req = MessageStatusRequest { message_ids };
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&req)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("message_status failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn create_blob(&self, auth: &DeviceAuth) -> Result<CreateBlobResponse> {
        let url = format!("{}/v1/create_blob", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!("create_blob failed with {}", response.status()));
        }
        Ok(response.json().await?)
    }

    pub async fn append_blob_chunk(
        &self,
        auth: &DeviceAuth,
        blob_id: String,
        chunk_b64: String,
    ) -> Result<AppendBlobChunkResponse> {
        let url = format!("{}/v1/append_blob_chunk", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&AppendBlobChunkRequest { blob_id, chunk_b64 })
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("append_blob_chunk failed with {status}: {body}"));
        }
        Ok(response.json().await?)
    }

    pub async fn fetch_blob_bytes(&self, auth: &DeviceAuth, blob_id: String) -> Result<Vec<u8>> {
        let url = format!("{}/v1/fetch_blob_bytes", self.cfg.http_base);
        let response = self
            .client
            .post(url)
            .headers(self.auth_headers(auth)?)
            .json(&FetchBlobRequest { blob_id })
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "fetch_blob_bytes failed with {}",
                response.status()
            ));
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn ws_once(&self, auth: &DeviceAuth) -> Result<Vec<PendingEnvelope>> {
        let url = format!("{}/v1/ws", self.cfg.ws_base);
        let mut req = url.into_client_request()?;
        req.headers_mut().append(
            "x-device-uuid",
            HeaderValue::from_str(&auth.device_uuid).map_err(|_| anyhow!("invalid uuid header"))?,
        );
        req.headers_mut().append(
            "x-device-token",
            HeaderValue::from_str(&auth.auth_token).map_err(|_| anyhow!("invalid token header"))?,
        );

        let (mut socket, _) = connect_async(req).await?;
        let mut out = Vec::new();
        while let Some(msg) = socket.next().await {
            match msg? {
                Message::Text(text) => {
                    if text == "ready" {
                        break;
                    }
                    if let Ok(pending) = serde_json::from_str::<shared::PendingMessageItem>(&text) {
                        out.push(PendingEnvelope {
                            message_id: pending.message_id,
                            from_device_uuid: pending.from_device_uuid,
                            envelope_b64: pending.envelope_b64,
                            created_at_unix_ms: pending.created_at_unix_ms,
                        });
                    }
                }
                Message::Ping(p) => {
                    socket.send(Message::Pong(p)).await?;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = socket.close(None).await;
        Ok(out)
    }

    fn auth_headers(&self, auth: &DeviceAuth) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-device-uuid",
            HeaderValue::from_str(&auth.device_uuid).map_err(|_| anyhow!("invalid uuid header"))?,
        );
        headers.insert(
            "x-device-token",
            HeaderValue::from_str(&auth.auth_token).map_err(|_| anyhow!("invalid token header"))?,
        );
        Ok(headers)
    }
}
