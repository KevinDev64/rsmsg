use axum::{Json, extract::State, http::HeaderMap};
use shared::{FetchBlobRequest, FetchBlobResponse, UploadBlobRequest, UploadBlobResponse};

use crate::{api_error::ApiResult, app_state::AppState, auth::authorize_device, domain::blob};

pub async fn upload_blob(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadBlobRequest>,
) -> ApiResult<Json<UploadBlobResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        blob::upload_blob(&state.db, auth_device, payload).await?,
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
