use axum::http::{HeaderMap, StatusCode};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    format!("{digest:x}")
}

pub async fn authorize_device(db: &sqlx::PgPool, headers: &HeaderMap) -> Result<Uuid, StatusCode> {
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
