use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use shared::{FetchBlobRequest, FetchBlobResponse, UploadBlobRequest, UploadBlobResponse};

use crate::{api_error::ApiResult, app_state::AppState, auth::authorize_device, domain::blob};

pub async fn upload_blob(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadBlobRequest>,
) -> ApiResult<Json<UploadBlobResponse>> {
    tracing::info!(
        encoded_len = payload.data_b64.len(),
        "upload_blob request received"
    );
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        blob::upload_blob(&state.db, auth_device, payload).await?,
    ))
}

pub async fn upload_blob_bytes(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<UploadBlobResponse>> {
    tracing::info!(bytes_len = body.len(), "upload_blob raw request received");
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        blob::upload_blob_bytes(&state.db, auth_device, body.to_vec()).await?,
    ))
}

pub async fn fetch_blob(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchBlobRequest>,
) -> ApiResult<Json<FetchBlobResponse>> {
    let _auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(blob::fetch_blob(&state.db, payload).await?))
}

pub async fn fetch_blob_bytes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FetchBlobRequest>,
) -> ApiResult<Response> {
    let _auth_device = authorize_device(&state.db, &headers).await?;
    let data = blob::fetch_blob_bytes(&state.db, payload).await?;
    Ok((StatusCode::OK, data).into_response())
}
