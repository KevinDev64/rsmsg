use std::{
    collections::VecDeque,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::http::StatusCode;
use shared::{
    CallSignalItem, FetchCallSignalsRequest, FetchCallSignalsResponse, SendCallSignalRequest,
    SendCallSignalResponse,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    api_error::{ApiError, ApiResult},
    domain::realtime,
    repository::{devices, user_blocks},
};

const MAX_SIGNAL_QUEUE: usize = 2048;

#[derive(Clone)]
struct QueuedCallSignal {
    to_device_uuid: Uuid,
    item: CallSignalItem,
}

static CALL_SIGNALS: OnceLock<Mutex<VecDeque<QueuedCallSignal>>> = OnceLock::new();

fn call_signals() -> &'static Mutex<VecDeque<QueuedCallSignal>> {
    CALL_SIGNALS.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub async fn send_call_signal(
    db: &sqlx::PgPool,
    auth_device: Uuid,
    payload: SendCallSignalRequest,
) -> ApiResult<SendCallSignalResponse> {
    let from_device = Uuid::parse_str(&payload.from_device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid from device"))?;
    let to_device = Uuid::parse_str(&payload.to_device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid to device"))?;
    if auth_device != from_device {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }
    if payload.call_id.trim().is_empty() || payload.kind.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid call signal",
        ));
    }

    let from_user = devices::find_user_id_by_device_uuid(db, from_device)
        .await
        .map_err(|err| ApiError::database("send_call_signal sender lookup failed", err))?
        .ok_or(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"))?;
    let to_user = devices::find_user_id_by_device_uuid(db, to_device)
        .await
        .map_err(|err| ApiError::database("send_call_signal recipient lookup failed", err))?
        .ok_or(ApiError::new(
            StatusCode::NOT_FOUND,
            "recipient device not found",
        ))?;
    if let Some(blocker) = user_blocks::block_direction(db, from_user.clone(), to_user.clone())
        .await
        .map_err(|err| ApiError::database("send_call_signal block lookup failed", err))?
    {
        if blocker == to_user {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "recipient blocked you",
            ));
        }
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "you blocked recipient",
        ));
    }
    if !realtime::is_online(to_device).await {
        return Err(ApiError::new(StatusCode::CONFLICT, "recipient is offline"));
    }

    let item = CallSignalItem {
        call_id: payload.call_id,
        from_device_uuid: from_device.to_string(),
        kind: payload.kind,
        payload: payload.payload,
        created_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default(),
    };
    let mut queue = call_signals().lock().await;
    if queue.len() >= MAX_SIGNAL_QUEUE {
        queue.pop_front();
    }
    queue.push_back(QueuedCallSignal {
        to_device_uuid: to_device,
        item,
    });
    Ok(SendCallSignalResponse { accepted: true })
}

pub async fn fetch_call_signals(
    auth_device: Uuid,
    payload: FetchCallSignalsRequest,
) -> ApiResult<FetchCallSignalsResponse> {
    let device_uuid = Uuid::parse_str(&payload.device_uuid)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid device uuid"))?;
    if auth_device != device_uuid {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "device mismatch"));
    }

    let limit = payload.limit.unwrap_or(100).clamp(1, 500);
    let mut signals = Vec::new();
    let mut queue = call_signals().lock().await;
    let mut retained = VecDeque::with_capacity(queue.len());
    while let Some(signal) = queue.pop_front() {
        let call_matches = match payload.call_id.as_ref() {
            Some(call_id) => call_id == &signal.item.call_id,
            None => true,
        };
        if signal.to_device_uuid == device_uuid && call_matches && signals.len() < limit {
            signals.push(signal.item);
        } else {
            retained.push_back(signal);
        }
    }
    *queue = retained;
    Ok(FetchCallSignalsResponse { signals })
}
