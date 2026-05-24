use axum::http::HeaderMap;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    domain::realtime,
    repository::auth_tokens,
};

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{digest:x}")
}

pub async fn authorize_device(db: &sqlx::PgPool, headers: &HeaderMap) -> ApiResult<Uuid> {
    let device_uuid = headers
        .get("x-device-uuid")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "missing device credentials",
        ))
        .and_then(|v| {
            Uuid::parse_str(v).map_err(|_| {
                ApiError::new(axum::http::StatusCode::UNAUTHORIZED, "invalid device uuid")
            })
        })?;
    let token = headers
        .get("x-device-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "missing device token",
        ))?;
    let token_hash = hash_token(token);

    let active = auth_tokens::is_token_active(db, device_uuid, token_hash)
        .await
        .map_err(|err| ApiError::database("authorize_device token lookup failed", err))?;

    if active {
        realtime::mark_online(device_uuid).await;
        Ok(device_uuid)
    } else {
        Err(ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "invalid token",
        ))
    }
}
