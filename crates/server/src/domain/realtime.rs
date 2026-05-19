use axum::extract::ws::{Message, WebSocket};
use tokio::time::{self, Duration};
use uuid::Uuid;

use crate::services::messages::drain_pending_messages;

pub async fn run_session(mut socket: WebSocket, db: sqlx::PgPool, device_uuid: Uuid) {
    let _ = socket.send(Message::Text("ready".into())).await;

    let mut poll = time::interval(Duration::from_millis(1200));
    loop {
        tokio::select! {
            _ = poll.tick() => {
                if let Ok(messages) = drain_pending_messages(&db, device_uuid, 200).await {
                    for message in messages {
                        if let Ok(payload) = serde_json::to_string(&message) {
                            if socket.send(Message::Text(payload.into())).await.is_err() {
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
                    Some(Ok(Message::Close(_))) | None => return,
                    Some(Err(_)) => return,
                    _ => {}
                }
            }
        }
    }
}
