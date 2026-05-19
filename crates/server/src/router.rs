use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    app_state::AppState,
    handlers::{device, health, messaging, ws},
};

pub fn build_router(app_state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/ws", get(ws::ws_realtime))
        .route("/v1/register_device", post(device::register_device))
        .route("/v1/device_login", post(device::device_login))
        .route("/v1/device_logout", post(device::device_logout))
        .route("/v1/upload_prekeys", post(device::upload_prekeys))
        .route(
            "/v1/fetch_prekey_bundle",
            post(messaging::fetch_prekey_bundle),
        )
        .route("/v1/send_message", post(messaging::send_message))
        .route("/v1/fetch_pending", post(messaging::fetch_pending))
        .route("/v1/ack_message", post(messaging::ack_message))
        .with_state(app_state)
}
