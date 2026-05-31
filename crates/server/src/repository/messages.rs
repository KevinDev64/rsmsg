use uuid::Uuid;

pub async fn insert_message(
    db: &sqlx::PgPool,
    message_id: String,
    from_device: Uuid,
    to_device: Uuid,
    envelope: Vec<u8>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO messages (message_id, from_device, to_device, envelope_bytes) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (message_id) DO NOTHING",
    )
    .bind(message_id)
    .bind(from_device)
    .bind(to_device)
    .bind(envelope)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub async fn fetch_pending_locked(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    to_device: Uuid,
    limit: i64,
) -> Result<Vec<(Uuid, String, Uuid, Vec<u8>, i64)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String, Uuid, Vec<u8>, i64)>(
        "SELECT id, message_id, from_device, envelope_bytes, EXTRACT(EPOCH FROM created_at)::BIGINT * 1000 \
         FROM messages \
         WHERE to_device = $1 AND acked_at IS NULL \
         ORDER BY created_at \
         LIMIT $2 \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(to_device)
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
}

pub async fn mark_delivered(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE messages SET delivered_at = COALESCE(delivered_at, NOW()) WHERE id = $1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn ack_messages(
    db: &sqlx::PgPool,
    to_device: Uuid,
    message_ids: Vec<String>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE messages \
         SET acked_at = NOW() \
         WHERE to_device = $1 AND message_id = ANY($2::text[]) AND acked_at IS NULL",
    )
    .bind(to_device)
    .bind(message_ids)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

pub async fn fetch_statuses(
    db: &sqlx::PgPool,
    from_device: Uuid,
    message_ids: Vec<String>,
) -> Result<Vec<(String, bool, bool)>, sqlx::Error> {
    sqlx::query_as::<_, (String, bool, bool)>(
        "SELECT message_id, delivered_at IS NOT NULL, acked_at IS NOT NULL \
         FROM messages \
         WHERE from_device = $1 AND message_id = ANY($2::text[])",
    )
    .bind(from_device)
    .bind(message_ids)
    .fetch_all(db)
    .await
}
