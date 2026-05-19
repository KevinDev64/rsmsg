use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{
    DeviceLoginRequest, DeviceLoginResponse, DeviceLogoutResponse, RegisterDeviceRequest,
    RegisterDeviceResponse, UploadPrekeysRequest, UploadPrekeysResponse,
};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    auth::hash_token,
    repository::{auth_tokens, devices, prekeys},
};

pub async fn register_device(
    db: &sqlx::PgPool,
    payload: RegisterDeviceRequest,
) -> ApiResult<RegisterDeviceResponse> {
    let identity_key = STANDARD
        .decode(payload.identity_key_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid identity key"))?;
    let signed_prekey = STANDARD
        .decode(payload.signed_prekey_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid signed prekey"))?;

    let device_uuid = devices::upsert_device(
        db,
        payload.user_id,
        payload.device_id,
        identity_key,
        signed_prekey,
    )
    .await
    .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(RegisterDeviceResponse {
        device_uuid: device_uuid.to_string(),
    })
}

pub async fn upload_prekeys(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    payload: UploadPrekeysRequest,
) -> ApiResult<UploadPrekeysResponse> {
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let mut tx = db
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

    Ok(UploadPrekeysResponse { inserted })
}

pub async fn device_login(
    db: &sqlx::PgPool,
    payload: DeviceLoginRequest,
) -> ApiResult<DeviceLoginResponse> {
    let device_uuid = devices::find_device_uuid(db, payload.user_id, payload.device_id)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "device not found"))?;

    let auth_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    auth_tokens::create_token(db, device_uuid, hash_token(&auth_token))
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;

    Ok(DeviceLoginResponse {
        device_uuid: device_uuid.to_string(),
        auth_token,
    })
}

pub async fn device_logout(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    token: &str,
    device_uuid: &str,
) -> ApiResult<DeviceLogoutResponse> {
    let parsed = Uuid::parse_str(device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != parsed {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let affected = auth_tokens::revoke_token(db, parsed, hash_token(token))
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(DeviceLogoutResponse {
        revoked: affected == 1,
    })
}
