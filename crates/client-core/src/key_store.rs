use crate::{local_vault, types::LocalDeviceKeys};

pub fn load(path: &str, password: Option<&str>) -> Option<LocalDeviceKeys> {
    local_vault::load_json(path, password)
}

pub fn save(path: &str, keys: &LocalDeviceKeys, password: Option<&str>) -> anyhow::Result<()> {
    local_vault::save_json(path, keys, password)
}
