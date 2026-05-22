use std::collections::HashMap;

use crate::{local_vault, types::StoredPeerSession};

pub fn load(path: &str, password: Option<&str>) -> HashMap<String, String> {
    let Some(items) = local_vault::load_json::<Vec<StoredPeerSession>>(path, password) else {
        return HashMap::new();
    };
    items
        .into_iter()
        .map(|s| (s.peer_device_uuid, s.shared_key_b64))
        .collect()
}

pub fn save(
    path: &str,
    sessions: &HashMap<String, String>,
    password: Option<&str>,
) -> anyhow::Result<()> {
    let mut items: Vec<StoredPeerSession> = sessions
        .iter()
        .map(|(peer_device_uuid, shared_key_b64)| StoredPeerSession {
            peer_device_uuid: peer_device_uuid.clone(),
            shared_key_b64: shared_key_b64.clone(),
        })
        .collect();
    items.sort_by(|a, b| a.peer_device_uuid.cmp(&b.peer_device_uuid));
    local_vault::save_json(path, &items, password)
}
