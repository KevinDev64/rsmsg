use uuid::Uuid;

pub async fn insert_one_time_prekey(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    device_uuid: Uuid,
    key_id: i32,
    pubkey: Vec<u8>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO one_time_prekeys (device_ref, key_id, pubkey) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (device_ref, key_id) DO NOTHING",
    )
    .bind(device_uuid)
    .bind(key_id)
    .bind(pubkey)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

pub async fn consume_one_time_prekey(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    device_uuid: Uuid,
) -> Result<Option<(i32, Vec<u8>)>, sqlx::Error> {
    sqlx::query_as::<_, (i32, Vec<u8>)>(
        "WITH picked AS ( \
            SELECT id, key_id, pubkey FROM one_time_prekeys \
            WHERE device_ref = $1 AND consumed_at IS NULL \
            ORDER BY id \
            LIMIT 1 \
            FOR UPDATE SKIP LOCKED \
        ) \
        UPDATE one_time_prekeys p \
        SET consumed_at = NOW() \
        FROM picked \
        WHERE p.id = picked.id \
        RETURNING picked.key_id, picked.pubkey",
    )
    .bind(device_uuid)
    .fetch_optional(&mut **tx)
    .await
}
