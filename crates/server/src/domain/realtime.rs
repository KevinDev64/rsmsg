use std::{collections::HashSet, sync::OnceLock};

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::Mutex;
use tokio::time::{self, Duration};
use uuid::Uuid;

use crate::services::messages::drain_pending_messages;

static ONLINE_DEVICES: OnceLock<Mutex<HashSet<Uuid>>> = OnceLock::new();

fn online_devices() -> &'static Mutex<HashSet<Uuid>> {
    ONLINE_DEVICES.get_or_init(|| Mutex::new(HashSet::new()))
}

pub async fn is_online(device_uuid: Uuid) -> bool {
    online_devices().lock().await.contains(&device_uuid)
}

pub async fn run_session(mut socket: WebSocket, db: sqlx::PgPool, device_uuid: Uuid) {
    online_devices().lock().await.insert(device_uuid);
    let _ = socket.send(Message::Text("ready".into())).await;

    let mut poll = time::interval(Duration::from_millis(1200));
    loop {
        tokio::select! {
            _ = poll.tick() => {
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
