use axum::{Json, extract::State, http::HeaderMap};
use shared::{
    AckMessageRequest, AckMessageResponse, FetchPendingRequest, FetchPendingResponse,
    FetchPrekeyBundleRequest, FetchPrekeyBundleResponse, SendMessageRequest, SendMessageResponse,
};

use crate::{api_error::ApiResult, app_state::AppState, auth::authorize_device, domain::messaging};

pub async fn fetch_prekey_bundle(
    State(state): State<AppState>,
    Json(payload): Json<FetchPrekeyBundleRequest>,
) -> ApiResult<Json<FetchPrekeyBundleResponse>> {
    Ok(Json(
        messaging::fetch_prekey_bundle(&state.db, payload).await?,
    ))
}

pub async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendMessageRequest>,
) -> ApiResult<Json<SendMessageResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        messaging::send_message(&state.db, auth_device, payload).await?,
    ))
}

pub async fn fetch_pending(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchPendingRequest>,
) -> ApiResult<Json<FetchPendingResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        messaging::fetch_pending(&state.db, auth_device, payload).await?,
    ))
}

pub async fn ack_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AckMessageRequest>,
) -> ApiResult<Json<AckMessageResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        messaging::ack_message(&state.db, auth_device, payload).await?,
    ))
}
