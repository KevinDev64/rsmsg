use std::{fs, path::Path};

use crate::types::LocalDeviceKeys;

pub fn load(path: &str) -> Option<LocalDeviceKeys> {
    let file = Path::new(path);
    if !file.exists() {
        return None;
    }
    let raw = fs::read_to_string(file).ok()?;
    serde_json::from_str::<LocalDeviceKeys>(&raw).ok()
}

pub fn save(path: &str, keys: &LocalDeviceKeys) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(keys)?;
    fs::write(path, json)?;
    Ok(())
}
