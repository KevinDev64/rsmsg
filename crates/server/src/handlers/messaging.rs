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
    api_error::{ApiError, ApiResult},
    app_state::AppState,
    auth::authorize_device,
    repository::{devices, messages, prekeys},
    services::messages::drain_pending_messages,
};

pub async fn fetch_prekey_bundle(
    State(state): State<AppState>,
    Json(payload): Json<FetchPrekeyBundleRequest>,
) -> ApiResult<Json<FetchPrekeyBundleResponse>> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    let device = devices::find_device_bundle(&state.db, payload.user_id, payload.device_id)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "device not found"))?;

    let one_time = prekeys::consume_one_time_prekey(&mut tx, device.0)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    tx.commit()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

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
) -> ApiResult<Json<SendMessageResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
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

    let rows = messages::insert_message(
        &state.db,
        payload.message_id,
        from_device,
        to_device,
        envelope,
    )
    .await
    .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(SendMessageResponse {
        accepted: rows == 1,
    }))
}

pub async fn fetch_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchPendingRequest>,
) -> ApiResult<Json<FetchPendingResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let to_device = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    let limit = payload.limit.unwrap_or(100).clamp(1, 500);

    if auth_device != to_device {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let messages = drain_pending_messages(&state.db, to_device, limit).await?;
    Ok(Json(FetchPendingResponse { messages }))
}

pub async fn ack_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AckMessageRequest>,
) -> ApiResult<Json<AckMessageResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }
    if payload.message_ids.is_empty() {
        return Ok(Json(AckMessageResponse { acked: 0 }));
    }

    let acked = messages::ack_messages(&state.db, device_uuid, payload.message_ids)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(AckMessageResponse { acked }))
}
