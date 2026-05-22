use uuid::Uuid;

pub async fn upsert_device(
    db: &sqlx::PgPool,
    user_id: String,
    device_id: String,
    identity_key: Vec<u8>,
    signing_identity_key: Vec<u8>,
    signed_prekey: Vec<u8>,
    signed_prekey_signature: Vec<u8>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO devices (user_id, device_id, identity_key, signing_identity_key, signed_prekey, signed_prekey_signature) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (user_id, device_id) \
         DO UPDATE SET identity_key = EXCLUDED.identity_key, signing_identity_key = EXCLUDED.signing_identity_key, signed_prekey = EXCLUDED.signed_prekey, signed_prekey_signature = EXCLUDED.signed_prekey_signature \
         RETURNING id",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(identity_key)
    .bind(signing_identity_key)
    .bind(signed_prekey)
    .bind(signed_prekey_signature)
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
) -> Result<Option<(Uuid, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)>(
        "SELECT id, identity_key, signing_identity_key, signed_prekey, signed_prekey_signature FROM devices WHERE user_id = $1 AND device_id = $2",
    )
    .bind(user_id)
    .bind(device_id)
    .fetch_optional(db)
    .await
}

pub async fn device_exists(db: &sqlx::PgPool, device_uuid: Uuid) -> Result<bool, sqlx::Error> {
    let found = sqlx::query_scalar::<_, i32>("SELECT 1 FROM devices WHERE id = $1 LIMIT 1")
        .bind(device_uuid)
        .fetch_optional(db)
        .await?;
    Ok(found == Some(1))
}

pub async fn find_user_id_by_device_uuid(
    db: &sqlx::PgPool,
    device_uuid: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT user_id FROM devices WHERE id = $1")
        .bind(device_uuid)
        .fetch_optional(db)
        .await
}
