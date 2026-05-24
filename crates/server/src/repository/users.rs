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

pub async fn create_user_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: String,
    password_hash: String,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (user_id, password_hash) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO NOTHING \
         RETURNING id",
    )
    .bind(user_id)
    .bind(password_hash)
    .fetch_optional(&mut **tx)
    .await
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

pub async fn user_exists(db: &sqlx::PgPool, user_id: String) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar::<_, i32>("SELECT 1 FROM users WHERE user_id = $1 LIMIT 1")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    Ok(found == Some(1))
}

pub async fn search_users(db: &sqlx::PgPool, query: String) -> Result<Vec<String>, sqlx::Error> {
    let pattern = format!("{}%", query);
    sqlx::query_scalar::<_, String>(
        "SELECT user_id FROM users WHERE user_id ILIKE $1 ORDER BY user_id LIMIT 20",
    )
    .bind(pattern)
    .fetch_all(db)
    .await
}
