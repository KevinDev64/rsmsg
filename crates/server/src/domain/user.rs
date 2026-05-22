use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::http::StatusCode;
use shared::{
    ResolveUserRequest, ResolveUserResponse, UserLoginRequest, UserLoginResponse,
    UserRegisterRequest, UserRegisterResponse, UserSearchRequest, UserSearchResponse,
};

use crate::{
    api_error::{ApiError, ApiResult},
    repository::{devices, users},
};

fn hash_password(password: &str) -> ApiResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "hashing failed"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
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
    let password_hash = hash_password(&payload.password)?;
    let created = users::create_user(db, payload.user_id, password_hash)
        .await
        .map_err(|err| ApiError::database("user_register create failed", err))?;
    Ok(UserRegisterResponse { created })
}

pub async fn login(db: &sqlx::PgPool, payload: UserLoginRequest) -> ApiResult<UserLoginResponse> {
    let stored = users::get_password_hash(db, payload.user_id)
        .await
        .map_err(|err| ApiError::database("user_login password lookup failed", err))?;
    let Some(stored_hash) = stored else {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    };
    if !verify_password(&payload.password, &stored_hash) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid credentials",
        ));
    }
    Ok(UserLoginResponse { ok: true })
}

pub async fn resolve_user(
    db: &sqlx::PgPool,
    payload: ResolveUserRequest,
) -> ApiResult<ResolveUserResponse> {
    let device_uuid = devices::find_device_uuid(db, payload.user_id, payload.device_id)
        .await
        .map_err(|err| ApiError::database("resolve_user device lookup failed", err))?
        .ok_or(ApiError::new(
            StatusCode::NOT_FOUND,
            "user device not found",
        ))?;
    Ok(ResolveUserResponse {
        device_uuid: device_uuid.to_string(),
    })
}

pub async fn search_users(
    db: &sqlx::PgPool,
    payload: UserSearchRequest,
) -> ApiResult<UserSearchResponse> {
    if payload.query.trim().is_empty() {
        return Ok(UserSearchResponse { users: Vec::new() });
    }
    let users = users::search_users(db, payload.query)
        .await
        .map_err(|err| ApiError::database("user_search query failed", err))?;
    Ok(UserSearchResponse { users })
}
