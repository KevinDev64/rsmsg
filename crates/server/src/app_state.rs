use crate::login_rate_limit::LoginRateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub login_rate_limiter: LoginRateLimiter,
    pub min_client_version: String,
}
