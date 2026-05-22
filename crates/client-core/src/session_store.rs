use std::collections::HashMap;

use crate::{
    local_vault,
    types::{PeerSession, StoredPeerSession},
};

pub fn load(path: &str, password: Option<&str>) -> HashMap<String, PeerSession> {
    let Some(items) = local_vault::load_json::<Vec<StoredPeerSession>>(path, password) else {
        return HashMap::new();
    };
    items
        .into_iter()
        .map(|s| {
            let send_chain_key_b64 = s
                .send_chain_key_b64
                .clone()
                .unwrap_or_else(|| s.shared_key_b64.clone());
            let recv_chain_key_b64 = s
                .recv_chain_key_b64
                .clone()
                .unwrap_or_else(|| s.shared_key_b64.clone());
            (
                s.peer_device_uuid,
                PeerSession {
                    shared_key_b64: s.shared_key_b64,
                    send_chain_key_b64,
                    recv_chain_key_b64,
                    send_counter: s.send_counter,
                    recv_counter: s.recv_counter,
                },
            )
        })
        .collect()
}

pub fn save(
    path: &str,
    sessions: &HashMap<String, PeerSession>,
    password: Option<&str>,
) -> anyhow::Result<()> {
    let mut items: Vec<StoredPeerSession> = sessions
        .iter()
        .map(|(peer_device_uuid, session)| StoredPeerSession {
            peer_device_uuid: peer_device_uuid.clone(),
            shared_key_b64: session.shared_key_b64.clone(),
            send_chain_key_b64: Some(session.send_chain_key_b64.clone()),
            recv_chain_key_b64: Some(session.recv_chain_key_b64.clone()),
            send_counter: session.send_counter,
            recv_counter: session.recv_counter,
        })
        .collect();
    items.sort_by(|a, b| a.peer_device_uuid.cmp(&b.peer_device_uuid));
    local_vault::save_json(path, &items, password)
}
