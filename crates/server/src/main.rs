use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    http::HeaderMap,
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use shared::{
    AckMessageRequest, AckMessageResponse, DeviceLoginRequest, DeviceLoginResponse,
    DeviceLogoutRequest, DeviceLogoutResponse, FetchPendingRequest, FetchPendingResponse,
    FetchPrekeyBundleRequest, FetchPrekeyBundleResponse, PendingMessageItem, PrekeyUploadItem,
    RegisterDeviceRequest, RegisterDeviceResponse, SendMessageRequest, SendMessageResponse,
    UploadPrekeysRequest, UploadPrekeysResponse,
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{digest:x}")
}

async fn authorize_device(db: &sqlx::PgPool, headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    let device_uuid = headers
        .get("x-device-uuid")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)
        .and_then(|v| Uuid::parse_str(v).map_err(|_| StatusCode::UNAUTHORIZED))?;
    let token = headers
        .get("x-device-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let token_hash = hash_token(token);

    let found = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM device_auth_tokens \
         WHERE device_ref = $1 AND token_hash = $2 \
           AND revoked_at IS NULL AND expires_at > NOW() \
         LIMIT 1",
    )
    .bind(device_uuid)
    .bind(token_hash)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if found == Some(1) {
        Ok(device_uuid)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(1) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> Result<Json<RegisterDeviceResponse>, StatusCode> {
    let identity_key = STANDARD
        .decode(payload.identity_key_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let signed_prekey = STANDARD
        .decode(payload.signed_prekey_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let row = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO devices (user_id, device_id, identity_key, signed_prekey) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, device_id) \
         DO UPDATE SET identity_key = EXCLUDED.identity_key, signed_prekey = EXCLUDED.signed_prekey \
         RETURNING id",
    )
    .bind(payload.user_id)
    .bind(payload.device_id)
    .bind(identity_key)
    .bind(signed_prekey)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RegisterDeviceResponse {
        device_uuid: row.to_string(),
    }))
}

async fn upload_prekeys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadPrekeysRequest>,
) -> Result<Json<UploadPrekeysResponse>, StatusCode> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    if auth_device != device_uuid {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut inserted = 0_u64;
    for item in payload.prekeys {
        let pubkey = STANDARD
            .decode(item.pubkey_b64)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let result = sqlx::query(
            "INSERT INTO one_time_prekeys (device_ref, key_id, pubkey) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (device_ref, key_id) DO NOTHING",
        )
        .bind(device_uuid)
        .bind(item.key_id)
        .bind(pubkey)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        inserted += result.rows_affected();
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(UploadPrekeysResponse { inserted }))
}

async fn device_login(
    State(state): State<AppState>,
    Json(payload): Json<DeviceLoginRequest>,
) -> Result<Json<DeviceLoginResponse>, StatusCode> {
    let device_uuid = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM devices WHERE user_id = $1 AND device_id = $2",
    )
    .bind(payload.user_id)
    .bind(payload.device_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let auth_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&auth_token);

    sqlx::query(
        "INSERT INTO device_auth_tokens (device_ref, token_hash, expires_at) \
         VALUES ($1, $2, NOW() + INTERVAL '30 days')",
    )
    .bind(device_uuid)
    .bind(token_hash)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(DeviceLoginResponse {
        device_uuid: device_uuid.to_string(),
        auth_token,
    }))
}

async fn device_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeviceLogoutRequest>,
) -> Result<Json<DeviceLogoutResponse>, StatusCode> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    if auth_device != device_uuid {
        return Err(StatusCode::FORBIDDEN);
    }

    let token = headers
        .get("x-device-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let result = sqlx::query(
        "UPDATE device_auth_tokens \
         SET revoked_at = NOW() \
         WHERE device_ref = $1 AND token_hash = $2 AND revoked_at IS NULL",
    )
    .bind(device_uuid)
    .bind(hash_token(token))
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(DeviceLogoutResponse {
        revoked: result.rows_affected() == 1,
    }))
}

async fn drain_pending_messages(
    db: &sqlx::PgPool,
    to_device: Uuid,
    limit: i64,
) -> Result<Vec<PendingMessageItem>, StatusCode> {
    let mut tx = db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = sqlx::query_as::<_, (Uuid, String, Uuid, Vec<u8>, i64)>(
        "SELECT id, message_id, from_device, envelope_bytes, EXTRACT(EPOCH FROM created_at)::BIGINT * 1000 \
         FROM messages \
         WHERE to_device = $1 AND delivered_at IS NULL \
         ORDER BY created_at \
         LIMIT $2 \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(to_device)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for row in &rows {
        sqlx::query("UPDATE messages SET delivered_at = NOW() WHERE id = $1")
            .bind(row.0)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(rows
        .into_iter()
        .map(
            |(_, message_id, from_device_uuid, envelope_bytes, created_at_unix_ms)| {
                PendingMessageItem {
                    message_id,
                    from_device_uuid: from_device_uuid.to_string(),
                    envelope_b64: STANDARD.encode(envelope_bytes),
                    created_at_unix_ms,
                }
            },
        )
        .collect())
}

async fn fetch_prekey_bundle(
    State(state): State<AppState>,
    Json(payload): Json<FetchPrekeyBundleRequest>,
) -> Result<Json<FetchPrekeyBundleResponse>, StatusCode> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let device = sqlx::query_as::<_, (Uuid, Vec<u8>, Vec<u8>)>(
        "SELECT id, identity_key, signed_prekey FROM devices WHERE user_id = $1 AND device_id = $2",
    )
    .bind(payload.user_id)
    .bind(payload.device_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let one_time = sqlx::query_as::<_, (i32, Vec<u8>)>(
        "WITH picked AS ( \
            SELECT id, key_id, pubkey FROM one_time_prekeys \
            WHERE device_ref = $1 AND consumed_at IS NULL \
            ORDER BY id \
            LIMIT 1 \
            FOR UPDATE SKIP LOCKED \
        ) \
        UPDATE one_time_prekeys p \
        SET consumed_at = NOW() \
        FROM picked \
        WHERE p.id = picked.id \
        RETURNING picked.key_id, picked.pubkey",
    )
    .bind(device.0)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(FetchPrekeyBundleResponse {
        device_uuid: device.0.to_string(),
        identity_key_b64: STANDARD.encode(device.1),
        signed_prekey_b64: STANDARD.encode(device.2),
        one_time_prekey: one_time.map(|(key_id, pubkey)| PrekeyUploadItem {
            key_id,
            pubkey_b64: STANDARD.encode(pubkey),
        }),
    }))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let from_device =
        Uuid::parse_str(&payload.from_device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    let to_device =
        Uuid::parse_str(&payload.to_device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    let envelope = STANDARD
        .decode(payload.envelope_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if auth_device != from_device {
        return Err(StatusCode::FORBIDDEN);
    }

    let result = sqlx::query(
        "INSERT INTO messages (message_id, from_device, to_device, envelope_bytes) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (message_id) DO NOTHING",
    )
    .bind(payload.message_id)
    .bind(from_device)
    .bind(to_device)
    .bind(envelope)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SendMessageResponse {
        accepted: result.rows_affected() == 1,
    }))
}

async fn fetch_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchPendingRequest>,
) -> Result<Json<FetchPendingResponse>, StatusCode> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let to_device = Uuid::parse_str(&payload.device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    let limit = payload.limit.unwrap_or(100).clamp(1, 500);

    if auth_device != to_device {
        return Err(StatusCode::FORBIDDEN);
    }

    let messages = drain_pending_messages(&state.db, to_device, limit).await?;

    Ok(Json(FetchPendingResponse { messages }))
}

async fn ws_realtime(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<axum::response::Response, StatusCode> {
    let device_uuid = authorize_device(&state.db, &headers).await?;
    Ok(ws.on_upgrade(move |mut socket| async move {
        if let Ok(messages) = drain_pending_messages(&state.db, device_uuid, 200).await {
            for message in messages {
                if let Ok(payload) = serde_json::to_string(&message) {
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
        let _ = socket.send(Message::Text("ready".into())).await;
    }))
}

async fn ack_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AckMessageRequest>,
) -> Result<Json<AckMessageResponse>, StatusCode> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    if auth_device != device_uuid {
        return Err(StatusCode::FORBIDDEN);
    }
    if payload.message_ids.is_empty() {
        return Ok(Json(AckMessageResponse { acked: 0 }));
    }

    let result = sqlx::query(
        "UPDATE messages \
         SET acked_at = NOW() \
         WHERE to_device = $1 AND message_id = ANY($2::text[]) AND acked_at IS NULL",
    )
    .bind(device_uuid)
    .bind(payload.message_ids)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AckMessageResponse {
        acked: result.rows_affected(),
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let bind_addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let app_state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/ws", get(ws_realtime))
        .route("/v1/register_device", post(register_device))
        .route("/v1/device_login", post(device_login))
        .route("/v1/device_logout", post(device_logout))
        .route("/v1/upload_prekeys", post(upload_prekeys))
        .route("/v1/fetch_prekey_bundle", post(fetch_prekey_bundle))
        .route("/v1/send_message", post(send_message))
        .route("/v1/fetch_pending", post(fetch_pending))
        .route("/v1/ack_message", post(ack_message))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
