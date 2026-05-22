use uuid::Uuid;

pub async fn upsert_device(
    db: &sqlx::PgPool,
    user_id: String,
    device_id: String,
    identity_key: Vec<u8>,
    signed_prekey: Vec<u8>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO devices (user_id, device_id, identity_key, signed_prekey) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, device_id) \
         DO UPDATE SET identity_key = EXCLUDED.identity_key, signed_prekey = EXCLUDED.signed_prekey \
         RETURNING id",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(identity_key)
    .bind(signed_prekey)
    .fetch_one(db)
    .await
}

pub async fn find_device_uuid(
    db: &sqlx::PgPool,
    user_id: String,
    device_id: String,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM devices WHERE user_id = $1 AND device_id = $2")
        .bind(user_id)
        .bind(device_id)
        .fetch_optional(db)
        .await
}

pub async fn find_device_bundle(
    db: &sqlx::PgPool,
    user_id: String,
    device_id: String,
) -> Result<Option<(Uuid, Vec<u8>, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, Vec<u8>, Vec<u8>)>(
        "SELECT id, identity_key, signed_prekey FROM devices WHERE user_id = $1 AND device_id = $2",
    )
    .bind(user_id)
    .bind(device_id)
    .fetch_optional(db)
    .await
}

pub async fn device_exists(db: &sqlx::PgPool, device_uuid: Uuid) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar::<_, i64>("SELECT 1 FROM devices WHERE id = $1 LIMIT 1")
        .bind(device_uuid)
        .fetch_optional(db)
        .await?;
    Ok(found == Some(1))
}
