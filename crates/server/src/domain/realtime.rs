use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
    time::{Duration as StdDuration, Instant},
};

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use uuid::Uuid;

use crate::services::messages::drain_pending_messages;

static ONLINE_DEVICES: OnceLock<Mutex<HashSet<Uuid>>> = OnceLock::new();
static DEVICE_LAST_SEEN: OnceLock<Mutex<HashMap<Uuid, Instant>>> = OnceLock::new();

const ONLINE_TTL: StdDuration = StdDuration::from_secs(20);

fn online_devices() -> &'static Mutex<HashSet<Uuid>> {
    ONLINE_DEVICES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn device_last_seen() -> &'static Mutex<HashMap<Uuid, Instant>> {
    DEVICE_LAST_SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn mark_online(device_uuid: Uuid) {
    device_last_seen()
        .lock()
        .await
        .insert(device_uuid, Instant::now());
}

pub async fn is_online(device_uuid: Uuid) -> bool {
    if online_devices().lock().await.contains(&device_uuid) {
        return true;
    }
    device_last_seen()
        .lock()
        .await
        .get(&device_uuid)
        .is_some_and(|last_seen| last_seen.elapsed() <= ONLINE_TTL)
}

pub async fn run_session(mut socket: WebSocket, db: sqlx::PgPool, device_uuid: Uuid) {
    online_devices().lock().await.insert(device_uuid);
    mark_online(device_uuid).await;
    let _ = socket.send(Message::Text("ready".into())).await;

    let mut poll = time::interval(Duration::from_millis(1200));
    loop {
        tokio::select! {
            _ = poll.tick() => {
                mark_online(device_uuid).await;
                if let Ok(messages) = drain_pending_messages(&db, device_uuid, 200).await {
                    for message in messages {
                        if let Ok(payload) = serde_json::to_string(&message) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
                                online_devices().lock().await.remove(&device_uuid);
                                return;
                            }
                        }
                    }
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    online_devices().lock().await.remove(&device_uuid);
}
