use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::PendingMessageItem;
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    repository::messages,
};

pub async fn drain_pending_messages(
    db: &sqlx::PgPool,
    to_device: Uuid,
    limit: i64,
) -> ApiResult<Vec<PendingMessageItem>> {
    let mut tx = db.begin().await.map_err(|_| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "database error",
        )
    })?;

    let rows = messages::fetch_pending_locked(&mut tx, to_device, limit)
        .await
        .map_err(|_| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "database error",
            )
        })?;

    for row in &rows {
        messages::mark_delivered(&mut tx, row.0)
            .await
            .map_err(|_| {
                ApiError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "database error",
                )
            })?;
    }

    tx.commit().await.map_err(|_| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "database error",
        )
    })?;

    Ok(rows
        .into_iter()
        .map(
            |(_, message_id, from_device_uuid, envelope_bytes, created_at_unix_ms)| {
                PendingMessageItem {
                    message_id,
                    from_device_uuid: from_device_uuid.to_string(),
                    envelope_b64: STANDARD.encode(envelope_bytes),
                    created_at_unix_ms,
                }
            },
        )
        .collect())
}
