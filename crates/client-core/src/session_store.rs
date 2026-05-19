use std::{collections::HashMap, fs, path::Path};

use crate::types::StoredPeerSession;

pub fn load(path: &str) -> HashMap<String, String> {
    let file = Path::new(path);
    if !file.exists() {
        return HashMap::new();
    }
    let Ok(raw) = fs::read_to_string(file) else {
        return HashMap::new();
    };
    let Ok(items) = serde_json::from_str::<Vec<StoredPeerSession>>(&raw) else {
        return HashMap::new();
    };
    items
        .into_iter()
        .map(|s| (s.peer_device_uuid, s.shared_key_b64))
        .collect()
}

pub fn save(path: &str, sessions: &HashMap<String, String>) -> anyhow::Result<()> {
    let mut items: Vec<StoredPeerSession> = sessions
        .iter()
        .map(|(peer_device_uuid, shared_key_b64)| StoredPeerSession {
            peer_device_uuid: peer_device_uuid.clone(),
            shared_key_b64: shared_key_b64.clone(),
        })
        .collect();
    items.sort_by(|a, b| a.peer_device_uuid.cmp(&b.peer_device_uuid));
    let json = serde_json::to_string_pretty(&items)?;
    fs::write(path, json)?;
    Ok(())
}
