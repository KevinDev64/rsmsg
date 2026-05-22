use axum::http::StatusCode;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use shared::{FetchBlobRequest, FetchBlobResponse, UploadBlobRequest, UploadBlobResponse};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    repository::blobs,
};

const MAX_BLOB_BYTES: usize = 100 * 1024 * 1024;

pub async fn upload_blob(
    db: &sqlx::PgPool,
    owner_device: Uuid,
    payload: UploadBlobRequest,
) -> ApiResult<UploadBlobResponse> {
    let data = STANDARD
        .decode(payload.data_b64)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid blob data"))?;
    tracing::info!(decoded_len = data.len(), "upload_blob decoded payload");
    if data.len() > MAX_BLOB_BYTES {
        tracing::warn!(
            decoded_len = data.len(),
            max_len = MAX_BLOB_BYTES,
            "upload_blob rejected: blob too large"
        );
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "blob too large",
        ));
    }
    let blob_id = blobs::insert_blob(db, owner_device, data)
        .await
        .map_err(|err| ApiError::database("upload_blob insert failed", err))?;
    tracing::info!(%blob_id, "upload_blob stored blob");
    Ok(UploadBlobResponse {
        blob_id: blob_id.to_string(),
    })
}

pub async fn upload_blob_bytes(
    db: &sqlx::PgPool,
    owner_device: Uuid,
    data: Vec<u8>,
) -> ApiResult<UploadBlobResponse> {
    tracing::info!(decoded_len = data.len(), "upload_blob raw payload");
    if data.len() > MAX_BLOB_BYTES {
        tracing::warn!(
            decoded_len = data.len(),
            max_len = MAX_BLOB_BYTES,
            "upload_blob rejected: blob too large"
        );
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "blob too large",
        ));
    }
    let blob_id = blobs::insert_blob(db, owner_device, data)
        .await
        .map_err(|err| ApiError::database("upload_blob insert failed", err))?;
    tracing::info!(%blob_id, "upload_blob stored blob");
    Ok(UploadBlobResponse {
        blob_id: blob_id.to_string(),
    })
}

pub async fn fetch_blob(
    db: &sqlx::PgPool,
    payload: FetchBlobRequest,
) -> ApiResult<FetchBlobResponse> {
    let blob_id = Uuid::parse_str(&payload.blob_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid blob id"))?;
    let data = blobs::fetch_blob(db, blob_id)
        .await
        .map_err(|err| ApiError::database("fetch_blob select failed", err))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "blob not found"))?;
    Ok(FetchBlobResponse {
        data_b64: STANDARD.encode(data),
    })
}

pub async fn fetch_blob_bytes(db: &sqlx::PgPool, payload: FetchBlobRequest) -> ApiResult<Vec<u8>> {
    let blob_id = Uuid::parse_str(&payload.blob_id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid blob id"))?;
    blobs::fetch_blob(db, blob_id)
        .await
        .map_err(|err| ApiError::database("fetch_blob select failed", err))?
        .ok_or(ApiError::new(StatusCode::NOT_FOUND, "blob not found"))
}
