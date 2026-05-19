pub async fn create_user(
    db: &sqlx::PgPool,
    user_id: String,
    password_hash: String,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO users (user_id, password_hash) VALUES ($1, $2) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn get_password_hash(
    db: &sqlx::PgPool,
    user_id: String,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT password_hash FROM users WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await
}
