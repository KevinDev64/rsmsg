pub async fn block_user(
    db: &sqlx::PgPool,
    blocker_user_id: String,
    blocked_user_id: String,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO user_blocks (blocker_user_id, blocked_user_id) \
         VALUES ($1, $2) \
         ON CONFLICT (blocker_user_id, blocked_user_id) DO NOTHING",
    )
    .bind(blocker_user_id)
    .bind(blocked_user_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn unblock_user(
    db: &sqlx::PgPool,
    blocker_user_id: String,
    blocked_user_id: String,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM user_blocks WHERE blocker_user_id = $1 AND blocked_user_id = $2")
            .bind(blocker_user_id)
            .bind(blocked_user_id)
            .execute(db)
            .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn list_blocked_users(
    db: &sqlx::PgPool,
    blocker_user_id: String,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT blocked_user_id FROM user_blocks WHERE blocker_user_id = $1 ORDER BY blocked_user_id",
    )
    .bind(blocker_user_id)
    .fetch_all(db)
    .await
}

pub async fn block_direction(
    db: &sqlx::PgPool,
    sender_user_id: String,
    recipient_user_id: String,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT blocker_user_id FROM user_blocks \
         WHERE (blocker_user_id = $1 AND blocked_user_id = $2) \
            OR (blocker_user_id = $2 AND blocked_user_id = $1) \
         ORDER BY CASE WHEN blocker_user_id = $2 THEN 0 ELSE 1 END \
         LIMIT 1",
    )
    .bind(sender_user_id)
    .bind(recipient_user_id)
    .fetch_optional(db)
    .await
}
