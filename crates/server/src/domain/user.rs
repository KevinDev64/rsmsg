use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::http::StatusCode;
use shared::{
    ResolveDeviceRequest, ResolveDeviceResponse, ResolveUserRequest, ResolveUserResponse,
    UserLoginRequest, UserLoginResponse, UserRegisterRequest, UserRegisterResponse,
    UserSearchRequest, UserSearchResponse,
};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    repository::{devices, registration_invites, users},
};

const INVITE_CODE_PREFIX: &str = "RSMSG:";

struct ParsedInviteCode {
    id: Uuid,
    secret: String,
}

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

fn parse_invite_code(code: &str) -> ApiResult<ParsedInviteCode> {
    let Some(rest) = code.trim().strip_prefix(INVITE_CODE_PREFIX) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid invite code",
        ));
    };
    let Some((id, secret)) = rest.split_once(':') else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid invite code",
        ));
    };
    if secret.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid invite code",
        ));
    }
    let id = Uuid::parse_str(id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid invite code"))?;
    Ok(ParsedInviteCode {
        id,
        secret: secret.to_string(),
    })
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
    let invite = parse_invite_code(&payload.invite_code)?;
    let password_hash = hash_password(&payload.password)?;

    let mut tx = db
        .begin()
        .await
        .map_err(|err| ApiError::database("user_register transaction begin failed", err))?;
    let stored_invite = registration_invites::find_invite(&mut tx, invite.id)
        .await
        .map_err(|err| ApiError::database("user_register invite lookup failed", err))?
        .ok_or(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid invite code",
        ))?;
    if stored_invite.used_at_exists || stored_invite.expired {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid invite code",
        ));
    }
    if !verify_password(&invite.secret, &stored_invite.secret_hash) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid invite code",
        ));
    }
    let user_db_id = users::create_user_tx(&mut tx, payload.user_id, password_hash)
        .await
        .map_err(|err| ApiError::database("user_register create failed", err))?;
    let Some(user_db_id) = user_db_id else {
        return Ok(UserRegisterResponse { created: false });
    };
    registration_invites::mark_used(&mut tx, invite.id, user_db_id)
        .await
        .map_err(|err| ApiError::database("user_register invite consume failed", err))?;
    tx.commit()
        .await
        .map_err(|err| ApiError::database("user_register transaction commit failed", err))?;
    Ok(UserRegisterResponse { created: true })
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

pub async fn resolve_device(
    db: &sqlx::PgPool,
    payload: ResolveDeviceRequest,
) -> ApiResult<ResolveDeviceResponse> {
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    let user_id = devices::find_user_id_by_device_uuid(db, device_uuid)
        .await
        .map_err(|err| ApiError::database("resolve_device user lookup failed", err))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "device not found"))?;
    Ok(ResolveDeviceResponse { user_id })
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
