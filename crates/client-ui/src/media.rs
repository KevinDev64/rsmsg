use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
        mpsc,
    },
    thread,
};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use webrtc::{
    api::APIBuilder,
    data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage},
    ice_transport::ice_server::RTCIceServer,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        sdp::session_description::RTCSessionDescription,
    },
};

pub const SYSTEM_DEFAULT_DEVICE: &str = "System default";
const AUDIO_SAMPLE_RATE: usize = 48_000;
const PLAYBACK_BUFFER_START: usize = AUDIO_SAMPLE_RATE / 10;
const PLAYBACK_BUFFER_MIN: usize = AUDIO_SAMPLE_RATE / 25;
const PLAYBACK_BUFFER_TARGET: usize = AUDIO_SAMPLE_RATE / 5;
const PLAYBACK_BUFFER_MAX: usize = AUDIO_SAMPLE_RATE * 2;

#[derive(Clone)]
pub struct IceConfig {
    pub servers: Vec<String>,
    pub turn_username: String,
    pub turn_password: String,
}

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

pub fn speaker_devices() -> Vec<String> {
    let host = cpal::default_host();
    let mut devices = host
        .output_devices()
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

pub struct AudioPlayback {
    _stream: cpal::Stream,
    _queue: Arc<Mutex<VecDeque<f32>>>,
}

pub struct WebRtcSession {
    peer_connection: Arc<RTCPeerConnection>,
    status: Arc<Mutex<String>>,
    _data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    outbound_audio_tx: mpsc::Sender<Vec<u8>>,
    playback_queue: Arc<Mutex<VecDeque<f32>>>,
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

    pub fn audio_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.outbound_audio_tx.clone()
    }

    pub fn playback_queue(&self) -> Arc<Mutex<VecDeque<f32>>> {
        self.playback_queue.clone()
    }

    pub async fn create_offer(ice_config: IceConfig) -> Result<(Self, String)> {
        let session = Self::new(true, ice_config).await?;
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

    pub async fn create_answer(
        offer_payload: &str,
        ice_config: IceConfig,
    ) -> Result<(Self, String)> {
        let session = Self::new(false, ice_config).await?;
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

    async fn new(create_data_channel: bool, ice_config: IceConfig) -> Result<Self> {
        let api = APIBuilder::new().build();
        let peer_connection = Arc::new(api.new_peer_connection(rtc_config(ice_config)).await?);
        let status = Arc::new(Mutex::new("WebRTC starting".to_string()));
        let data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(None));
        let playback_queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let (outbound_audio_tx, outbound_audio_rx) = mpsc::channel::<Vec<u8>>();
        let data_channel_for_sender = data_channel.clone();
        let status_for_sender = status.clone();
        thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
            while let Ok(frame) = outbound_audio_rx.recv() {
                let channel = data_channel_for_sender
                    .lock()
                    .expect("webrtc_data_channel")
                    .clone();
                if let Some(channel) = channel {
                    if rt.block_on(channel.send(&Bytes::from(frame))).is_err() {
                        *status_for_sender.lock().expect("webrtc_status") =
                            "WebRTC audio send failed".to_string();
                    }
                }
            }
        });
        let status_for_state = status.clone();
        peer_connection.on_peer_connection_state_change(Box::new(move |state| {
            *status_for_state.lock().expect("webrtc_status") = format!("WebRTC {state}");
            Box::pin(async {})
        }));
        if create_data_channel {
            let channel = peer_connection
                .create_data_channel("rsmsg-call", None)
                .await?;
            *data_channel.lock().expect("webrtc_data_channel") = Some(channel.clone());
            let status_for_channel = status.clone();
            channel.on_open(Box::new(move || {
                *status_for_channel.lock().expect("webrtc_status") =
                    "WebRTC data channel open".to_string();
                Box::pin(async {})
            }));
            attach_audio_receiver(channel, playback_queue.clone(), status.clone());
        } else {
            let status_for_channel = status.clone();
            let data_channel_for_handler = data_channel.clone();
            let playback_queue_for_handler = playback_queue.clone();
            peer_connection.on_data_channel(Box::new(move |channel| {
                *data_channel_for_handler
                    .lock()
                    .expect("webrtc_data_channel") = Some(channel.clone());
                let status_for_open = status_for_channel.clone();
                channel.on_open(Box::new(move || {
                    *status_for_open.lock().expect("webrtc_status") =
                        "WebRTC data channel open".to_string();
                    Box::pin(async {})
                }));
                attach_audio_receiver(
                    channel,
                    playback_queue_for_handler.clone(),
                    status_for_channel.clone(),
                );
                Box::pin(async {})
            }));
        }
        Ok(Self {
            peer_connection,
            status,
            _data_channel: data_channel,
            outbound_audio_tx,
            playback_queue,
        })
    }
}

fn rtc_config(ice_config: IceConfig) -> RTCConfiguration {
    let servers = ice_config
        .servers
        .into_iter()
        .filter(|server| !server.trim().is_empty())
        .map(|server| RTCIceServer {
            urls: vec![server],
            username: ice_config.turn_username.clone(),
            credential: ice_config.turn_password.clone(),
        })
        .collect();
    RTCConfiguration {
        ice_servers: servers,
        ..Default::default()
    }
}

pub fn start_microphone_capture_with_sender(
    device_name: &str,
    audio_tx: Option<mpsc::Sender<Vec<u8>>>,
) -> Result<MediaSession> {
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
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_input_stream::<f32>(
            &device,
            config.into(),
            level.clone(),
            sample_rate,
            channels,
            audio_tx,
        )?,
        cpal::SampleFormat::I16 => build_input_stream::<i16>(
            &device,
            config.into(),
            level.clone(),
            sample_rate,
            channels,
            audio_tx,
        )?,
        cpal::SampleFormat::U16 => build_input_stream::<u16>(
            &device,
            config.into(),
            level.clone(),
            sample_rate,
            channels,
            audio_tx,
        )?,
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
    sample_rate: u32,
    channels: usize,
    audio_tx: Option<mpsc::Sender<Vec<u8>>>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + SampleLevel,
{
    let mut frame = Vec::with_capacity((sample_rate / 50) as usize);
    let frame_samples = (sample_rate / 50).max(160) as usize;
    Ok(device.build_input_stream(
        &config,
        move |data: &[T], _| {
            let peak = data
                .iter()
                .map(SampleLevel::sample_level)
                .fold(0.0_f32, f32::max)
                .clamp(0.0, 1.0);
            level.store((peak * 1000.0) as u32, Ordering::Relaxed);
            if let Some(audio_tx) = audio_tx.as_ref() {
                for chunk in data.chunks(channels.max(1)) {
                    let mono = chunk
                        .iter()
                        .map(SampleLevel::sample_level_signed)
                        .sum::<f32>()
                        / chunk.len().max(1) as f32;
                    frame.push(mono.clamp(-1.0, 1.0));
                    if frame.len() >= frame_samples {
                        let payload = encode_audio_frame(sample_rate, &frame);
                        let _ = audio_tx.send(payload);
                        frame.clear();
                    }
                }
            }
        },
        move |_| {},
        None,
    )?)
}

trait SampleLevel {
    fn sample_level(&self) -> f32;
    fn sample_level_signed(&self) -> f32;
}

impl SampleLevel for f32 {
    fn sample_level(&self) -> f32 {
        self.abs()
    }

    fn sample_level_signed(&self) -> f32 {
        *self
    }
}

impl SampleLevel for i16 {
    fn sample_level(&self) -> f32 {
        (*self as f32 / i16::MAX as f32).abs()
    }

    fn sample_level_signed(&self) -> f32 {
        *self as f32 / i16::MAX as f32
    }
}

impl SampleLevel for u16 {
    fn sample_level(&self) -> f32 {
        ((*self as f32 - 32768.0) / 32768.0).abs()
    }

    fn sample_level_signed(&self) -> f32 {
        (*self as f32 - 32768.0) / 32768.0
    }
}

fn attach_audio_receiver(
    channel: Arc<RTCDataChannel>,
    playback_queue: Arc<Mutex<VecDeque<f32>>>,
    status: Arc<Mutex<String>>,
) {
    channel.on_message(Box::new(move |message: DataChannelMessage| {
        if let Some(samples) = decode_audio_frame(&message.data) {
            let mut queue = playback_queue.lock().expect("audio_playback_queue");
            queue.extend(samples);
            while queue.len() > PLAYBACK_BUFFER_MAX {
                queue.pop_front();
            }
        } else {
            *status.lock().expect("webrtc_status") = "Unknown WebRTC message".to_string();
        }
        Box::pin(async {})
    }));
}

pub fn start_audio_playback(
    device_name: &str,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> Result<AudioPlayback> {
    let host = cpal::default_host();
    let device = if device_name == SYSTEM_DEFAULT_DEVICE {
        host.default_output_device()
    } else {
        host.output_devices()
            .ok()
            .into_iter()
            .flatten()
            .find(|device| device.name().ok().as_deref() == Some(device_name))
    }
    .ok_or_else(|| anyhow!("speaker not found"))?;
    let config = device.default_output_config()?;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_output_stream::<f32>(&device, config.into(), queue.clone())?
        }
        cpal::SampleFormat::I16 => {
            build_output_stream::<i16>(&device, config.into(), queue.clone())?
        }
        cpal::SampleFormat::U16 => {
            build_output_stream::<u16>(&device, config.into(), queue.clone())?
        }
        other => return Err(anyhow!("unsupported speaker sample format: {other:?}")),
    };
    stream.play()?;
    Ok(AudioPlayback {
        _stream: stream,
        _queue: queue,
    })
}

fn build_output_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    queue: Arc<Mutex<VecDeque<f32>>>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + OutputSample,
{
    let mut playing = false;
    Ok(device.build_output_stream(
        &config,
        move |out: &mut [T], _| {
            let output_len = out.len();
            let mut queue = queue.lock().expect("audio_playback_queue");
            for sample in out.iter_mut() {
                if !playing && queue.len() >= PLAYBACK_BUFFER_START {
                    playing = true;
                }
                if playing && queue.len() <= PLAYBACK_BUFFER_MIN {
                    playing = false;
                }
                while queue.len() > PLAYBACK_BUFFER_MAX {
                    queue.pop_front();
                }
                while queue.len() > PLAYBACK_BUFFER_TARGET + output_len {
                    queue.pop_front();
                }
                let value = if playing {
                    queue.pop_front().unwrap_or_default()
                } else {
                    0.0
                };
                *sample = T::from_f32(value);
            }
        },
        move |_| {},
        None,
    )?)
}

trait OutputSample {
    fn from_f32(value: f32) -> Self;
}

impl OutputSample for f32 {
    fn from_f32(value: f32) -> Self {
        value.clamp(-1.0, 1.0)
    }
}

impl OutputSample for i16 {
    fn from_f32(value: f32) -> Self {
        (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

impl OutputSample for u16 {
    fn from_f32(value: f32) -> Self {
        ((value.clamp(-1.0, 1.0) * 32767.0) + 32768.0) as u16
    }
}

fn encode_audio_frame(sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + samples.len() * 4);
    out.extend_from_slice(b"RSA1");
    out.extend_from_slice(&sample_rate.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn decode_audio_frame(payload: &[u8]) -> Option<Vec<f32>> {
    if payload.len() < 8 || &payload[..4] != b"RSA1" {
        return None;
    }
    let mut samples = Vec::with_capacity((payload.len() - 8) / 4);
    for chunk in payload[8..].chunks_exact(4) {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).clamp(-1.0, 1.0));
    }
    Some(samples)
}
