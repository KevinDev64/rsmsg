use uuid::Uuid;

pub async fn is_token_active(
    db: &sqlx::PgPool,
    device_uuid: Uuid,
    token_hash: String,
) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM device_auth_tokens \
         WHERE device_ref = $1 AND token_hash = $2 \
           AND revoked_at IS NULL AND expires_at > NOW() \
         LIMIT 1",
    )
    .bind(device_uuid)
    .bind(token_hash)
    .fetch_optional(db)
    .await?;

    Ok(found == Some(1))
}

pub async fn create_token(
    db: &sqlx::PgPool,
    device_uuid: Uuid,
    token_hash: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO device_auth_tokens (device_ref, token_hash, expires_at) \
         VALUES ($1, $2, NOW() + INTERVAL '30 days')",
    )
    .bind(device_uuid)
    .bind(token_hash)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke_token(
    db: &sqlx::PgPool,
    device_uuid: Uuid,
    token_hash: String,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE device_auth_tokens \
         SET revoked_at = NOW() \
         WHERE device_ref = $1 AND token_hash = $2 AND revoked_at IS NULL",
    )
    .bind(device_uuid)
    .bind(token_hash)
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}
