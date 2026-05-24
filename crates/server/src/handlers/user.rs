use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use shared::{
    BlockUserRequest, BlockUserResponse, BlockedUsersResponse, ResolveDeviceRequest,
    ResolveDeviceResponse, ResolveUserRequest, ResolveUserResponse, UnblockUserRequest,
    UnblockUserResponse, UserLoginRequest, UserLoginResponse, UserRegisterRequest,
    UserRegisterResponse, UserSearchRequest, UserSearchResponse,
};

use crate::{
    api_error::{ApiError, ApiResult},
    app_state::AppState,
    auth::authorize_device,
    domain::user,
};

pub async fn user_register(
    State(state): State<AppState>,
    Json(payload): Json<UserRegisterRequest>,
) -> ApiResult<Json<UserRegisterResponse>> {
    Ok(Json(user::register(&state.db, payload).await?))
}

pub async fn user_login(
    State(state): State<AppState>,
    Json(payload): Json<UserLoginRequest>,
) -> ApiResult<Json<UserLoginResponse>> {
    if !state.login_rate_limiter.allow(&payload.user_id) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "too many attempts",
        ));
    }
    match user::login(&state.db, payload.clone()).await {
        Ok(response) => {
            state.login_rate_limiter.record_success(&payload.user_id);
            Ok(Json(response))
        }
        Err(err) => {
            state.login_rate_limiter.record_failure(&payload.user_id);
            Err(err)
        }
    }
}

pub async fn resolve_user(
    State(state): State<AppState>,
    Json(payload): Json<ResolveUserRequest>,
) -> ApiResult<Json<ResolveUserResponse>> {
    Ok(Json(user::resolve_user(&state.db, payload).await?))
}

pub async fn resolve_device(
    State(state): State<AppState>,
    Json(payload): Json<ResolveDeviceRequest>,
) -> ApiResult<Json<ResolveDeviceResponse>> {
    Ok(Json(user::resolve_device(&state.db, payload).await?))
}

pub async fn user_search(
    State(state): State<AppState>,
    Json(payload): Json<UserSearchRequest>,
) -> ApiResult<Json<UserSearchResponse>> {
    Ok(Json(user::search_users(&state.db, payload).await?))
}

pub async fn block_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BlockUserRequest>,
) -> ApiResult<Json<BlockUserResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        user::block_user(&state.db, auth_device, payload).await?,
    ))
}

pub async fn unblock_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UnblockUserRequest>,
) -> ApiResult<Json<UnblockUserResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        user::unblock_user(&state.db, auth_device, payload).await?,
    ))
}

pub async fn blocked_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<BlockedUsersResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(user::blocked_users(&state.db, auth_device).await?))
}
