use anyhow::{Context, Result};
use server::{app_state::AppState, login_rate_limit::LoginRateLimiter, router::build_router};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("server=debug".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let bind_addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let app_state = AppState {
        db,
        login_rate_limiter: LoginRateLimiter::new(),
    };
    let app = build_router(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!(%bind_addr, "server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
