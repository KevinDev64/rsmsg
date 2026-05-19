use axum::{Json, extract::State};
use shared::{UserLoginRequest, UserLoginResponse, UserRegisterRequest, UserRegisterResponse};

use crate::{api_error::ApiResult, app_state::AppState, domain::user};

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
    Ok(Json(user::login(&state.db, payload).await?))
}
