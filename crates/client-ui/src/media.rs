use cpal::traits::{DeviceTrait, HostTrait};

pub const SYSTEM_DEFAULT_DEVICE: &str = "System default";

pub fn microphone_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut devices = host
        .input_devices()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|device| device.name().ok())
        .filter(|name| !name.trim().is_empty())
        .collect::<Vec<_>>();
    devices.sort();
    devices.dedup();
    devices
}

pub fn camera_devices() -> Vec<String> {
    Vec::new()
}
