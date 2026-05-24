use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use webrtc::{
    api::APIBuilder,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        sdp::session_description::RTCSessionDescription,
    },
};

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

pub struct WebRtcSession {
    peer_connection: Arc<RTCPeerConnection>,
    status: Arc<Mutex<String>>,
}

impl MediaSession {
    pub fn microphone_level(&self) -> f32 {
        self.level.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

impl WebRtcSession {
    pub fn status(&self) -> String {
        self.status.lock().expect("webrtc_status").clone()
    }

    pub async fn close(&self) {
        let _ = self.peer_connection.close().await;
    }

    pub async fn create_offer() -> Result<(Self, String)> {
        let session = Self::new(true).await?;
        let offer = session.peer_connection.create_offer(None).await?;
        let mut gather_complete = session.peer_connection.gathering_complete_promise().await;
        session.peer_connection.set_local_description(offer).await?;
        let _ = gather_complete.recv().await;
        let local = session
            .peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("missing local WebRTC offer"))?;
        Ok((session, serde_json::to_string(&local)?))
    }

    pub async fn create_answer(offer_payload: &str) -> Result<(Self, String)> {
        let session = Self::new(false).await?;
        let offer = serde_json::from_str::<RTCSessionDescription>(offer_payload)?;
        session
            .peer_connection
            .set_remote_description(offer)
            .await?;
        let answer = session.peer_connection.create_answer(None).await?;
        let mut gather_complete = session.peer_connection.gathering_complete_promise().await;
        session
            .peer_connection
            .set_local_description(answer)
            .await?;
        let _ = gather_complete.recv().await;
        let local = session
            .peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("missing local WebRTC answer"))?;
        Ok((session, serde_json::to_string(&local)?))
    }

    pub async fn apply_answer(&self, answer_payload: &str) -> Result<()> {
        let answer = serde_json::from_str::<RTCSessionDescription>(answer_payload)?;
        self.peer_connection.set_remote_description(answer).await?;
        Ok(())
    }

    async fn new(create_data_channel: bool) -> Result<Self> {
        let api = APIBuilder::new().build();
        let peer_connection = Arc::new(api.new_peer_connection(RTCConfiguration::default()).await?);
        let status = Arc::new(Mutex::new("WebRTC starting".to_string()));
        let status_for_state = status.clone();
        peer_connection.on_peer_connection_state_change(Box::new(move |state| {
            *status_for_state.lock().expect("webrtc_status") = format!("WebRTC {state}");
            Box::pin(async {})
        }));
        if create_data_channel {
            let channel = peer_connection
                .create_data_channel("rsmsg-call", None)
                .await?;
            let status_for_channel = status.clone();
            channel.on_open(Box::new(move || {
                *status_for_channel.lock().expect("webrtc_status") =
                    "WebRTC data channel open".to_string();
                Box::pin(async {})
            }));
        } else {
            let status_for_channel = status.clone();
            peer_connection.on_data_channel(Box::new(move |channel| {
                let status_for_open = status_for_channel.clone();
                channel.on_open(Box::new(move || {
                    *status_for_open.lock().expect("webrtc_status") =
                        "WebRTC data channel open".to_string();
                    Box::pin(async {})
                }));
                Box::pin(async {})
            }));
        }
        Ok(Self {
            peer_connection,
            status,
        })
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
