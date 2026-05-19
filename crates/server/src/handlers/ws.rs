use axum::{
    extract::{State, WebSocketUpgrade, ws::Message},
    http::{HeaderMap, StatusCode},
    response::Response,
};

use crate::{
    app_state::AppState, auth::authorize_device, services::messages::drain_pending_messages,
};

pub async fn ws_realtime(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let device_uuid = authorize_device(&state.db, &headers).await?;
    Ok(ws.on_upgrade(move |mut socket| async move {
        if let Ok(messages) = drain_pending_messages(&state.db, device_uuid, 200).await {
            for message in messages {
                if let Ok(payload) = serde_json::to_string(&message) {
                    if socket.send(Message::Text(payload.into())).await.is_err() {
                        return;
                    }
                }
            }
        }
        let _ = socket.send(Message::Text("ready".into())).await;
    }))
}
