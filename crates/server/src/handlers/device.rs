use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use shared::{
    DeviceLoginRequest, DeviceLoginResponse, DeviceLogoutRequest, DeviceLogoutResponse,
    RegisterDeviceRequest, RegisterDeviceResponse, UploadPrekeysRequest, UploadPrekeysResponse,
};

use crate::{
    api_error::{ApiError, ApiResult},
    app_state::AppState,
    auth::authorize_device,
    domain::device,
};

pub async fn register_device(
    State(state): State<AppState>,
    Json(payload): Json<RegisterDeviceRequest>,
) -> ApiResult<Json<RegisterDeviceResponse>> {
    Ok(Json(device::register_device(&state.db, payload).await?))
}

pub async fn upload_prekeys(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UploadPrekeysRequest>,
) -> ApiResult<Json<UploadPrekeysResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    Ok(Json(
        device::upload_prekeys(&state.db, auth_device, payload).await?,
    ))
}

pub async fn device_login(
    State(state): State<AppState>,
    Json(payload): Json<DeviceLoginRequest>,
) -> ApiResult<Json<DeviceLoginResponse>> {
    Ok(Json(device::device_login(&state.db, payload).await?))
}

pub async fn device_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DeviceLogoutRequest>,
) -> ApiResult<Json<DeviceLogoutResponse>> {
    let auth_device = authorize_device(&state.db, &headers).await?;
    let token = headers
        .get("x-device-token")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "missing device token",
        ))?;
    Ok(Json(
        device::device_logout(&state.db, auth_device, token, &payload.device_uuid).await?,
    ))
}
