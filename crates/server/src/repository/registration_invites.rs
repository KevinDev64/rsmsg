use uuid::Uuid;

pub struct RegistrationInvite {
    pub secret_hash: String,
    pub used_at_exists: bool,
    pub expired: bool,
}

pub async fn insert_invite(
    db: &sqlx::PgPool,
    id: Uuid,
    secret_hash: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO registration_invites (id, secret_hash, expires_at) \
         VALUES ($1, $2, NOW() + INTERVAL '2 days')",
    )
    .bind(id)
    .bind(secret_hash)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find_invite(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
) -> Result<Option<RegistrationInvite>, sqlx::Error> {
    sqlx::query_as::<_, (String, bool, bool)>(
        "SELECT secret_hash, used_at IS NOT NULL, expires_at <= NOW() \
         FROM registration_invites \
         WHERE id = $1 \
         FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .map(|row| {
        row.map(
            |(secret_hash, used_at_exists, expired)| RegistrationInvite {
                secret_hash,
                used_at_exists,
                expired,
            },
        )
    })
}

pub async fn mark_used(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    user_db_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE registration_invites \
         SET used_at = NOW(), used_by_user_id = $2 \
         WHERE id = $1 AND used_at IS NULL",
    )
    .bind(id)
    .bind(user_db_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
