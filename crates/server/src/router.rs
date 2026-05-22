use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use crate::{
    app_state::AppState,
    handlers::{blob, device, health, messaging, user, ws},
};

const MAX_REQUEST_BODY_BYTES: usize = 200 * 1024 * 1024;

pub fn build_router(app_state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/user_register", post(user::user_register))
        .route("/v1/user_login", post(user::user_login))
        .route("/v1/user_search", post(user::user_search))
        .route("/v1/resolve_user", post(user::resolve_user))
        .route("/v1/resolve_device", post(user::resolve_device))
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
        .route("/v1/message_status", post(messaging::message_status))
        .route("/v1/upload_blob", post(blob::upload_blob))
        .route("/v1/fetch_blob", post(blob::fetch_blob))
        .route("/v1/upload_blob_bytes", post(blob::upload_blob_bytes))
        .route("/v1/fetch_blob_bytes", post(blob::fetch_blob_bytes))
        .route("/v1/create_blob", post(blob::create_blob))
        .route("/v1/append_blob_chunk", post(blob::append_blob_chunk))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}
