use axum::http::StatusCode;
use axum::{Json, extract::State};
use shared::{
    ResolveUserRequest, ResolveUserResponse, UserLoginRequest, UserLoginResponse,
    UserRegisterRequest, UserRegisterResponse, UserSearchRequest, UserSearchResponse,
};

use crate::{
    api_error::{ApiError, ApiResult},
    app_state::AppState,
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

pub async fn user_search(
    State(state): State<AppState>,
    Json(payload): Json<UserSearchRequest>,
) -> ApiResult<Json<UserSearchResponse>> {
    Ok(Json(user::search_users(&state.db, payload).await?))
}
