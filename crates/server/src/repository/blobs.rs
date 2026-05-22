use uuid::Uuid;

pub async fn insert_blob(
    db: &sqlx::PgPool,
    owner_device: Uuid,
    data: Vec<u8>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO blobs (owner_device, data) VALUES ($1, $2) RETURNING id",
    )
    .bind(owner_device)
    .bind(data)
    .fetch_one(db)
    .await
}

pub async fn fetch_blob(db: &sqlx::PgPool, blob_id: Uuid) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar::<_, Vec<u8>>("SELECT data FROM blobs WHERE id = $1")
        .bind(blob_id)
        .fetch_optional(db)
        .await
}
