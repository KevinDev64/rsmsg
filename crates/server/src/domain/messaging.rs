use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{
    AckMessageRequest, AckMessageResponse, FetchPendingRequest, FetchPendingResponse,
    FetchPrekeyBundleRequest, FetchPrekeyBundleResponse, PrekeyUploadItem, SendMessageRequest,
    SendMessageResponse,
};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    repository::{devices, messages, prekeys},
    services::messages::drain_pending_messages,
};

pub async fn fetch_prekey_bundle(
    db: &sqlx::PgPool,
    payload: FetchPrekeyBundleRequest,
) -> ApiResult<FetchPrekeyBundleResponse> {
    let mut tx = db
        .begin()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let device = devices::find_device_bundle(db, payload.user_id, payload.device_id)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "device not found"))?;
    let one_time = prekeys::consume_one_time_prekey(&mut tx, device.0)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    tx.commit()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(FetchPrekeyBundleResponse {
        device_uuid: device.0.to_string(),
        identity_key_b64: STANDARD.encode(device.1),
        signed_prekey_b64: STANDARD.encode(device.2),
        one_time_prekey: one_time.map(|(key_id, pubkey)| PrekeyUploadItem {
            key_id,
            pubkey_b64: STANDARD.encode(pubkey),
        }),
    })
}

pub async fn send_message(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    payload: SendMessageRequest,
) -> ApiResult<SendMessageResponse> {
    let from_device = Uuid::parse_str(&payload.from_device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid from device"))?;
    let to_device = Uuid::parse_str(&payload.to_device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid to device"))?;
    let envelope = STANDARD
        .decode(payload.envelope_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid envelope"))?;
    if auth_device != from_device {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let rows = messages::insert_message(db, payload.message_id, from_device, to_device, envelope)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(SendMessageResponse {
        accepted: rows == 1,
    })
}

pub async fn fetch_pending(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    payload: FetchPendingRequest,
) -> ApiResult<FetchPendingResponse> {
    let to_device = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != to_device {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }
    let messages =
        drain_pending_messages(db, to_device, payload.limit.unwrap_or(100).clamp(1, 500)).await?;
    Ok(FetchPendingResponse { messages })
}

pub async fn ack_message(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    payload: AckMessageRequest,
) -> ApiResult<AckMessageResponse> {
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }
    if payload.message_ids.is_empty() {
        return Ok(AckMessageResponse { acked: 0 });
    }
    let acked = messages::ack_messages(db, device_uuid, payload.message_ids)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(AckMessageResponse { acked })
}
