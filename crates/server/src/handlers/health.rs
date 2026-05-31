use axum::{extract::State, http::StatusCode};

use crate::app_state::AppState;

pub async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query_scalar::<_, i64>("SELECT 1::BIGINT")
        .fetch_one(&state.db)
        .await
    {
        Ok(1) => StatusCode::OK,
        Ok(value) => {
            tracing::error!(value, "health check returned unexpected value");
            StatusCode::SERVICE_UNAVAILABLE
        }
        Err(error) => {
            tracing::error!(%error, "health check database query failed");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}
