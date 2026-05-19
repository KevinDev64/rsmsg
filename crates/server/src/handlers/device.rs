use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{
    DeviceLoginRequest, DeviceLoginResponse, DeviceLogoutRequest, DeviceLogoutResponse,
    RegisterDeviceRequest, RegisterDeviceResponse, UploadPrekeysRequest, UploadPrekeysResponse,
};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    app_state::AppState,
    auth::{authorize_device, hash_token},
    repository::{auth_tokens, devices, prekeys},
};

pub async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> ApiResult<Json<RegisterDeviceResponse>> {
    let identity_key = STANDARD
        .decode(payload.identity_key_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid identity key"))?;
    let signed_prekey = STANDARD
        .decode(payload.signed_prekey_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid signed prekey"))?;

    let row = devices::upsert_device(
        &state.db,
        payload.user_id,
        payload.device_id,
        identity_key,
        signed_prekey,
    )
    .await
    .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(RegisterDeviceResponse {
        device_uuid: row.to_string(),
    }))
}

pub async fn upload_prekeys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadPrekeysRequest>,
) -> ApiResult<Json<UploadPrekeysResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    let mut inserted = 0_u64;
    for item in payload.prekeys {
        let pubkey = STANDARD
            .decode(item.pubkey_b64)
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid prekey"))?;
        inserted += prekeys::insert_one_time_prekey(&mut tx, device_uuid, item.key_id, pubkey)
            .await
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    }

    tx.commit()
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(UploadPrekeysResponse { inserted }))
}

pub async fn device_login(
    State(state): State<AppState>,
    Json(payload): Json<DeviceLoginRequest>,
) -> ApiResult<Json<DeviceLoginResponse>> {
    let device_uuid = devices::find_device_uuid(&state.db, payload.user_id, payload.device_id)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "device not found"))?;

    let auth_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&auth_token);

    auth_tokens::create_token(&state.db, device_uuid, token_hash)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(DeviceLoginResponse {
        device_uuid: device_uuid.to_string(),
        auth_token,
    }))
}

pub async fn device_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeviceLogoutRequest>,
) -> ApiResult<Json<DeviceLogoutResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let token = headers
        .get("x-device-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "missing device token",
        ))?;

    let affected = auth_tokens::revoke_token(&state.db, device_uuid, hash_token(token))
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(Json(DeviceLogoutResponse {
        revoked: affected == 1,
    }))
}
