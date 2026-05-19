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
    app_state::AppState,
    auth::{authorize_device, hash_token},
};

pub async fn register_device(
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

pub async fn upload_prekeys(
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

pub async fn device_login(
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

pub async fn device_logout(
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
