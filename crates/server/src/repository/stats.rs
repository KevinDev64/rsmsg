#[derive(Debug)]
pub struct ServerStats {
    pub users_total: i64,
    pub devices_total: i64,
    pub active_tokens_total: i64,
    pub messages_sent_total: i64,
    pub messages_delivered_total: i64,
    pub messages_read_total: i64,
    pub messages_pending_total: i64,
    pub messages_sent_last_10m: i64,
}

pub async fn fetch_server_stats(db: &sqlx::PgPool) -> Result<ServerStats, sqlx::Error> {
    sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64)>(
        "SELECT \
            (SELECT COUNT(*) FROM users), \
            (SELECT COUNT(*) FROM devices), \
            (SELECT COUNT(*) FROM device_auth_tokens WHERE revoked_at IS NULL AND expires_at > NOW()), \
            (SELECT COUNT(*) FROM messages), \
            (SELECT COUNT(*) FROM messages WHERE delivered_at IS NOT NULL), \
            (SELECT COUNT(*) FROM messages WHERE acked_at IS NOT NULL), \
            (SELECT COUNT(*) FROM messages WHERE delivered_at IS NULL), \
            (SELECT COUNT(*) FROM messages WHERE created_at >= NOW() - INTERVAL '10 minutes')",
    )
    .fetch_one(db)
    .await
    .map(|row| ServerStats {
        users_total: row.0,
        devices_total: row.1,
        active_tokens_total: row.2,
        messages_sent_total: row.3,
        messages_delivered_total: row.4,
        messages_read_total: row.5,
        messages_pending_total: row.6,
        messages_sent_last_10m: row.7,
    })
}
