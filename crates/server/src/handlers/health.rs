use axum::{extract::State, http::StatusCode};

use crate::app_state::AppState;

pub async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(1) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}
