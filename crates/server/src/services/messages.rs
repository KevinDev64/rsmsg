use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::PendingMessageItem;
use uuid::Uuid;

pub async fn drain_pending_messages(
    db: &sqlx::PgPool,
    to_device: Uuid,
    limit: i64,
) -> Result<Vec<PendingMessageItem>, StatusCode> {
    let mut tx = db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = sqlx::query_as::<_, (Uuid, String, Uuid, Vec<u8>, i64)>(
        "SELECT id, message_id, from_device, envelope_bytes, EXTRACT(EPOCH FROM created_at)::BIGINT * 1000 \
         FROM messages \
         WHERE to_device = $1 AND delivered_at IS NULL \
         ORDER BY created_at \
         LIMIT $2 \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(to_device)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for row in &rows {
        sqlx::query("UPDATE messages SET delivered_at = NOW() WHERE id = $1")
            .bind(row.0)
            .execute(&mut *tx)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
