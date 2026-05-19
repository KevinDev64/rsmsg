use axum::http::StatusCode;
use sha2::{Digest, Sha256};
use shared::{UserLoginRequest, UserLoginResponse, UserRegisterRequest, UserRegisterResponse};

use crate::{
    api_error::{ApiError, ApiResult},
    repository::users,
};

fn hash_password(password: &str) -> String {
    let digest = Sha256::digest(password.as_bytes());
    format!("{digest:x}")
}

pub async fn register(
    db: &sqlx::PgPool,
    payload: UserRegisterRequest,
) -> ApiResult<UserRegisterResponse> {
    if payload.user_id.trim().is_empty() || payload.password.len() < 6 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid credentials",
        ));
    }
    let created = users::create_user(db, payload.user_id, hash_password(&payload.password))
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(UserRegisterResponse { created })
}

pub async fn login(db: &sqlx::PgPool, payload: UserLoginRequest) -> ApiResult<UserLoginResponse> {
    let stored = users::get_password_hash(db, payload.user_id)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let Some(stored_hash) = stored else {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    };
    if stored_hash != hash_password(&payload.password) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }
    Ok(UserLoginResponse { ok: true })
}
