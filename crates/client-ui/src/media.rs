use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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

pub struct MediaSession {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
}

impl MediaSession {
    pub fn microphone_level(&self) -> f32 {
        self.level.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

pub fn start_microphone_capture(device_name: &str) -> Result<MediaSession> {
    let host = cpal::default_host();
    let device = if device_name == SYSTEM_DEFAULT_DEVICE {
        host.default_input_device()
    } else {
        host.input_devices()
            .ok()
            .into_iter()
            .flatten()
            .find(|device| device.name().ok().as_deref() == Some(device_name))
    }
    .ok_or_else(|| anyhow!("microphone not found"))?;
    let config = device.default_input_config()?;
    let level = Arc::new(AtomicU32::new(0));
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_input_stream::<f32>(&device, config.into(), level.clone())?
        }
        cpal::SampleFormat::I16 => {
            build_input_stream::<i16>(&device, config.into(), level.clone())?
        }
        cpal::SampleFormat::U16 => {
            build_input_stream::<u16>(&device, config.into(), level.clone())?
        }
        other => return Err(anyhow!("unsupported microphone sample format: {other:?}")),
    };
    stream.play()?;
    Ok(MediaSession {
        _stream: stream,
        level,
    })
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    level: Arc<AtomicU32>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + SampleLevel,
{
    Ok(device.build_input_stream(
        &config,
        move |data: &[T], _| {
            let peak = data
                .iter()
                .map(SampleLevel::sample_level)
                .fold(0.0_f32, f32::max)
                .clamp(0.0, 1.0);
            level.store((peak * 1000.0) as u32, Ordering::Relaxed);
        },
        move |_| {},
        None,
    )?)
}

trait SampleLevel {
    fn sample_level(&self) -> f32;
}

impl SampleLevel for f32 {
    fn sample_level(&self) -> f32 {
        self.abs()
    }
}

impl SampleLevel for i16 {
    fn sample_level(&self) -> f32 {
        (*self as f32 / i16::MAX as f32).abs()
    }
}

impl SampleLevel for u16 {
    fn sample_level(&self) -> f32 {
        ((*self as f32 - 32768.0) / 32768.0).abs()
    }
}
