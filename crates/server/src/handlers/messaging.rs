use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{
    AckMessageRequest, AckMessageResponse, FetchPendingRequest, FetchPendingResponse,
    FetchPrekeyBundleRequest, FetchPrekeyBundleResponse, PrekeyUploadItem, SendMessageRequest,
    SendMessageResponse,
};
use uuid::Uuid;

use crate::{
    app_state::AppState, auth::authorize_device, services::messages::drain_pending_messages,
};

pub async fn fetch_prekey_bundle(
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

pub async fn send_message(
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

pub async fn fetch_pending(
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

pub async fn ack_message(
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
