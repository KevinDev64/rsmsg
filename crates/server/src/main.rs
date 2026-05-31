use anyhow::{Context, Result};
use server::{
    app_state::AppState, login_rate_limit::LoginRateLimiter, router::build_router,
    services::stats::spawn_stats_logger,
};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("server=debug".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let bind_addr = bind_addr()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let app_state = AppState {
        db: db.clone(),
        login_rate_limiter: LoginRateLimiter::new(),
        min_client_version: std::env::var("MIN_CLIENT_VERSION")
            .unwrap_or_else(|_| "0.0.0".to_string()),
    };
    spawn_stats_logger(db);
    let app = build_router(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(%bind_addr, "server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn bind_addr() -> Result<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-ip" {
            return args.next().context("-ip requires an address");
        }
    }
    Ok(std::env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string()))
}
