use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{
    RegisterDeviceRequest, RegisterDeviceResponse, UploadPrekeysRequest, UploadPrekeysResponse,
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,
}

async fn health(State(state): State<AppState>) -> StatusCode {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(1) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> Result<Json<RegisterDeviceResponse>, StatusCode> {
    let identity_key = STANDARD
        .decode(payload.identity_key_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let signed_prekey = STANDARD
        .decode(payload.signed_prekey_b64)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let row = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO devices (user_id, device_id, identity_key, signed_prekey) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, device_id) \
         DO UPDATE SET identity_key = EXCLUDED.identity_key, signed_prekey = EXCLUDED.signed_prekey \
         RETURNING id",
    )
    .bind(payload.user_id)
    .bind(payload.device_id)
    .bind(identity_key)
    .bind(signed_prekey)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(RegisterDeviceResponse {
        device_uuid: row.to_string(),
    }))
}

async fn upload_prekeys(
    State(state): State<AppState>,
    Json(payload): Json<UploadPrekeysRequest>,
) -> Result<Json<UploadPrekeysResponse>, StatusCode> {
    let device_uuid = Uuid::parse_str(&payload.device_uuid).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut inserted = 0_u64;
    for item in payload.prekeys {
        let pubkey = STANDARD
            .decode(item.pubkey_b64)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let result = sqlx::query(
            "INSERT INTO one_time_prekeys (device_ref, key_id, pubkey) \
             VALUES ($1, $2, $3) \
             ON CONFLICT (device_ref, key_id) DO NOTHING",
        )
        .bind(device_uuid)
        .bind(item.key_id)
        .bind(pubkey)
        .execute(&mut *tx)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        inserted += result.rows_affected();
    }

    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(UploadPrekeysResponse { inserted }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
    let bind_addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let app_state = AppState { db };
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/register_device", post(register_device))
        .route("/v1/upload_prekeys", post(upload_prekeys))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
