use std::time::Duration;

use crate::repository::stats;

pub fn spawn_stats_logger(db: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(600));
        loop {
            interval.tick().await;
            log_stats(&db).await;
        }
    });
}

async fn log_stats(db: &sqlx::PgPool) {
    match stats::fetch_server_stats(db).await {
        Ok(stats) => tracing::info!(
            users_total = stats.users_total,
            devices_total = stats.devices_total,
            active_tokens_total = stats.active_tokens_total,
            messages_sent_total = stats.messages_sent_total,
            messages_delivered_total = stats.messages_delivered_total,
            messages_read_total = stats.messages_read_total,
            messages_pending_total = stats.messages_pending_total,
            messages_sent_last_10m = stats.messages_sent_last_10m,
            "server aggregate stats"
        ),
        Err(error) => tracing::warn!(%error, "failed to collect server aggregate stats"),
    }
}
