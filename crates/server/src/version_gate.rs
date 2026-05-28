use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::app_state::AppState;

pub async fn require_supported_client(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.uri().path() == "/health" || !version_gt(&state.min_client_version, "0.0.0") {
        return next.run(request).await;
    }

    let Some(version) = request
        .headers()
        .get("x-rsmsg-client-version")
        .and_then(|value| value.to_str().ok())
    else {
        return upgrade_required(&state.min_client_version);
    };

    if version_gt(&state.min_client_version, version) {
        return upgrade_required(&state.min_client_version);
    }

    next.run(request).await
}

fn upgrade_required(minimum_version: &str) -> Response {
    (
        StatusCode::UPGRADE_REQUIRED,
        Json(json!({
            "error": "client version is no longer supported",
            "minimum_version": minimum_version,
            "update_url": "https://kevindev64.ru/rsmsg-downloads/stable/manifest.json"
        })),
    )
        .into_response()
}

fn version_gt(left: &str, right: &str) -> bool {
    parse_version(left) > parse_version(right)
}

fn parse_version(version: &str) -> (u64, u64, u64) {
    let mut parts = version
        .split(['.', '-', '+'])
        .map(|part| part.parse::<u64>().unwrap_or_default());
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}
