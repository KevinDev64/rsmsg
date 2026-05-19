use axum::{
    extract::{State, WebSocketUpgrade},
    http::HeaderMap,
    response::Response,
};

use crate::{api_error::ApiResult, app_state::AppState, auth::authorize_device, domain::realtime};

pub async fn ws_realtime(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let device_uuid = authorize_device(&state.db, &headers).await?;
    let db = state.db.clone();
    Ok(ws.on_upgrade(move |socket| async move {
        realtime::run_session(socket, db, device_uuid).await;
    }))
}
