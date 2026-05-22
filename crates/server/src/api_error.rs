use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub type ApiResult<T> = Result<T, ApiError>;

pub struct ApiError {
    pub status: StatusCode,
    pub message: &'static str,
}

impl ApiError {
    pub fn new(status: StatusCode, message: &'static str) -> Self {
        Self { status, message }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!(status = %self.status, message = self.message, "api error");
        } else {
            tracing::warn!(status = %self.status, message = self.message, "api error");
        }
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
