use axum::{Json, extract::State, http::HeaderMap};
use shared::{
    FetchCallSignalsRequest, FetchCallSignalsResponse, SendCallSignalRequest,
    SendCallSignalResponse,
};

use crate::{
    api_error::ApiResult, app_state::AppState, auth::authorize_device, domain::call_signaling,
};

pub async fn send_call_signal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SendCallSignalRequest>,
) -> ApiResult<Json<SendCallSignalResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        call_signaling::send_call_signal(&state.db, auth_device, payload).await?,
    ))
}

pub async fn fetch_call_signals(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchCallSignalsRequest>,
) -> ApiResult<Json<FetchCallSignalsResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        call_signaling::fetch_call_signals(auth_device, payload).await?,
    ))
}
