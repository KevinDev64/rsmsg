use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use nokhwa::{
    Camera, native_api_backend, nokhwa_initialize,
    pixel_format::RgbFormat,
    query,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use openh264::{
    decoder::Decoder as H264Decoder,
    encoder::Encoder as H264Encoder,
    formats::{RgbSliceU8, YUVBuffer, YUVSource},
};
use opus::{Application, Channels, Decoder, Encoder};
use tokio::runtime::Runtime;
use webrtc::{
    api::{
        APIBuilder,
        media_engine::{MIME_TYPE_H264, MIME_TYPE_OPUS, MediaEngine},
    },
    data_channel::{RTCDataChannel, data_channel_message::DataChannelMessage},
    ice_transport::ice_server::RTCIceServer,
    peer_connection::{
        RTCPeerConnection, configuration::RTCConfiguration,
        sdp::session_description::RTCSessionDescription,
    },
    rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
    track::track_local::{TrackLocal, track_local_static_sample::TrackLocalStaticSample},
};
use webrtc_media::Sample;

pub const SYSTEM_DEFAULT_DEVICE: &str = "System default";
const AUDIO_SAMPLE_RATE: usize = 48_000;
const PLAYBACK_BUFFER_START: usize = AUDIO_SAMPLE_RATE / 10;
const PLAYBACK_BUFFER_MIN: usize = AUDIO_SAMPLE_RATE / 25;
const PLAYBACK_BUFFER_TARGET: usize = AUDIO_SAMPLE_RATE / 5;
const PLAYBACK_BUFFER_MAX: usize = AUDIO_SAMPLE_RATE * 2;
const OPUS_SAMPLE_RATE: u32 = 48_000;
const OPUS_FRAME_SAMPLES: usize = 960;
const OPUS_MAX_PACKET_BYTES: usize = 1275;
const VIDEO_CLOCK_RATE: u32 = 90_000;
const VIDEO_FRAME_DURATION: Duration = Duration::from_millis(333);
const CAMERA_FRAME_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub struct IceConfig {
    pub servers: Vec<String>,
    pub turn_username: String,
    pub turn_password: String,
}

#[derive(Clone, Copy)]
pub struct AudioProcessingConfig {
    pub noise_suppression: bool,
    pub automatic_gain_control: bool,
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
    let _ = nokhwa_initialize(|_| {});
    let Some(backend) = native_api_backend() else {
        return Vec::new();
    };
    let mut devices = query(backend)
        .ok()
        .into_iter()
        .flatten()
        .map(|camera| camera.human_name())
        .filter(|name| !name.trim().is_empty())
        .collect::<Vec<_>>();
    devices.sort();
    devices.dedup();
    devices
}

pub struct MediaSession {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
}

pub struct AudioPlayback {
    _stream: cpal::Stream,
    _queue: Arc<Mutex<VecDeque<f32>>>,
}

#[derive(Clone, Default)]
pub struct VideoFrameInfo {
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub rgb: Vec<u8>,
}

pub struct VideoCaptureSession {
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<VideoFrameInfo>>>,
    status: Arc<Mutex<String>>,
}

#[derive(Clone)]
pub struct WebRtcSession {
    runtime: Arc<Runtime>,
    peer_connection: Arc<RTCPeerConnection>,
    status: Arc<Mutex<String>>,
    remote_audio_level: Arc<AtomicU32>,
    _data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>>,
    outbound_audio_tx: mpsc::Sender<Vec<u8>>,
    outbound_video_tx: mpsc::Sender<VideoFrameInfo>,
    playback_queue: Arc<Mutex<VecDeque<f32>>>,
    remote_video: Arc<Mutex<Option<VideoFrameInfo>>>,
    _audio_track: Arc<TrackLocalStaticSample>,
    _video_track: Arc<TrackLocalStaticSample>,
}

impl MediaSession {
    pub fn microphone_level(&self) -> f32 {
        self.level.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

impl VideoCaptureSession {
    pub fn latest(&self) -> Option<VideoFrameInfo> {
        self.latest.lock().expect("video_capture_latest").clone()
    }

    pub fn status(&self) -> String {
        self.status.lock().expect("video_capture_status").clone()
    }
}

impl Drop for VideoCaptureSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl WebRtcSession {
    pub fn status(&self) -> String {
        self.status.lock().expect("webrtc_status").clone()
    }

    pub fn close(&self) {
        let _ = self.runtime.block_on(self.peer_connection.close());
    }

    pub fn audio_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.outbound_audio_tx.clone()
    }

    pub fn playback_queue(&self) -> Arc<Mutex<VecDeque<f32>>> {
        self.playback_queue.clone()
    }

    pub fn video_sender(&self) -> mpsc::Sender<VideoFrameInfo> {
        self.outbound_video_tx.clone()
    }

    pub fn remote_video(&self) -> Option<VideoFrameInfo> {
        self.remote_video.lock().expect("remote_video").clone()
    }

    pub fn remote_audio_level(&self) -> f32 {
        self.remote_audio_level.load(Ordering::Relaxed) as f32 / 1000.0
    }

    pub fn create_offer(ice_config: IceConfig) -> Result<(Self, String)> {
        let runtime = Arc::new(Runtime::new()?);
        let session = runtime.block_on(Self::new(true, ice_config, runtime.clone()))?;
        let local = runtime.block_on(async {
            let offer = session.peer_connection.create_offer(None).await?;
            let mut gather_complete = session.peer_connection.gathering_complete_promise().await;
            session.peer_connection.set_local_description(offer).await?;
            let _ = gather_complete.recv().await;
            session
                .peer_connection
                .local_description()
                .await
                .ok_or_else(|| anyhow!("missing local WebRTC offer"))
        })?;
        Ok((session, serde_json::to_string(&local)?))
    }

    pub fn create_answer(offer_payload: &str, ice_config: IceConfig) -> Result<(Self, String)> {
        let runtime = Arc::new(Runtime::new()?);
        let session = runtime.block_on(Self::new(false, ice_config, runtime.clone()))?;
        let offer = serde_json::from_str::<RTCSessionDescription>(offer_payload)?;
        let local = runtime.block_on(async {
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
            session
                .peer_connection
                .local_description()
                .await
                .ok_or_else(|| anyhow!("missing local WebRTC answer"))
        })?;
        Ok((session, serde_json::to_string(&local)?))
    }

    pub fn apply_answer(&self, answer_payload: &str) -> Result<()> {
        let answer = serde_json::from_str::<RTCSessionDescription>(answer_payload)?;
        self.runtime
            .block_on(self.peer_connection.set_remote_description(answer))?;
        Ok(())
    }

    async fn new(
        create_data_channel: bool,
        ice_config: IceConfig,
        runtime: Arc<Runtime>,
    ) -> Result<Self> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;
        let api = APIBuilder::new().with_media_engine(media_engine).build();
        let peer_connection = Arc::new(api.new_peer_connection(rtc_config(ice_config)).await?);
        let status = Arc::new(Mutex::new("WebRTC starting".to_string()));
        let data_channel: Arc<Mutex<Option<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(None));
        let playback_queue = Arc::new(Mutex::new(VecDeque::<f32>::new()));
        let remote_video = Arc::new(Mutex::new(None));
        let remote_audio_level = Arc::new(AtomicU32::new(0));
        let audio_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_string(),
                clock_rate: OPUS_SAMPLE_RATE,
                channels: 1,
                ..Default::default()
            },
            "rsmsg-audio".to_string(),
            "rsmsg-call".to_string(),
        ));
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_string(),
                clock_rate: VIDEO_CLOCK_RATE,
                ..Default::default()
            },
            "rsmsg-video".to_string(),
            "rsmsg-call".to_string(),
        ));
        let rtp_sender = peer_connection
            .add_track(audio_track.clone() as Arc<dyn TrackLocal + Send + Sync>)
            .await?;
        tokio::spawn(async move { while rtp_sender.read_rtcp().await.is_ok() {} });
        let video_rtp_sender = peer_connection
            .add_track(video_track.clone() as Arc<dyn TrackLocal + Send + Sync>)
            .await?;
        tokio::spawn(async move { while video_rtp_sender.read_rtcp().await.is_ok() {} });
        let (outbound_audio_tx, outbound_audio_rx) = mpsc::channel::<Vec<u8>>();
        let (outbound_video_tx, outbound_video_rx) = mpsc::channel::<VideoFrameInfo>();
        let audio_track_for_sender = audio_track.clone();
        let status_for_sender = status.clone();
        thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
            let Ok(mut encoder) = Encoder::new(OPUS_SAMPLE_RATE, Channels::Mono, Application::Voip)
            else {
                *status_for_sender.lock().expect("webrtc_status") =
                    "Opus encoder failed".to_string();
                return;
            };
            let mut opus_output = vec![0_u8; OPUS_MAX_PACKET_BYTES];
            while let Ok(frame) = outbound_audio_rx.recv() {
                let Some((sample_rate, samples)) = decode_audio_frame_with_rate(&frame) else {
                    continue;
                };
                let opus_frame = resample_to_opus(sample_rate, &samples);
                let Ok(packet_len) = encoder.encode_float(&opus_frame, &mut opus_output) else {
                    *status_for_sender.lock().expect("webrtc_status") =
                        "Opus encode failed".to_string();
                    continue;
                };
                let sample = Sample {
                    data: Bytes::copy_from_slice(&opus_output[..packet_len]),
                    duration: Duration::from_millis(20),
                    ..Default::default()
                };
                if rt
                    .block_on(audio_track_for_sender.write_sample(&sample))
                    .is_err()
                {
                    *status_for_sender.lock().expect("webrtc_status") =
                        "WebRTC audio track send failed".to_string();
                }
            }
        });
        let video_track_for_sender = video_track.clone();
        let status_for_video = status.clone();
        thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Runtime::new() else {
                return;
            };
            let mut encoder = match H264Encoder::new() {
                Ok(encoder) => Some(encoder),
                Err(err) => {
                    *status_for_video.lock().expect("webrtc_status") =
                        format!("H264 encoder failed: {err}");
                    None
                }
            };
            while let Ok(frame) = outbound_video_rx.recv() {
                if let Some(encoder) = encoder.as_mut() {
                    let rgb =
                        RgbSliceU8::new(&frame.rgb, (frame.width as usize, frame.height as usize));
                    let yuv = YUVBuffer::from_rgb8_source(rgb);
                    match encoder.encode(&yuv) {
                        Ok(bitstream) => {
                            let sample = Sample {
                                data: Bytes::from(bitstream.to_vec()),
                                duration: VIDEO_FRAME_DURATION,
                                ..Default::default()
                            };
                            if rt
                                .block_on(video_track_for_sender.write_sample(&sample))
                                .is_err()
                            {
                                *status_for_video.lock().expect("webrtc_status") =
                                    "WebRTC video track send failed".to_string();
                            }
                        }
                        Err(err) => {
                            *status_for_video.lock().expect("webrtc_status") =
                                format!("H264 encode failed: {err}");
                        }
                    }
                }
            }
        });
        let playback_queue_for_track = playback_queue.clone();
        let status_for_track = status.clone();
        let remote_audio_level_for_track = remote_audio_level.clone();
        let remote_video_for_track = remote_video.clone();
        peer_connection.on_track(Box::new(move |track, _, _| {
            let playback_queue = playback_queue_for_track.clone();
            let status = status_for_track.clone();
            let remote_audio_level = remote_audio_level_for_track.clone();
            let remote_video = remote_video_for_track.clone();
            Box::pin(async move {
                let mime_type = track.codec().capability.mime_type;
                if mime_type.eq_ignore_ascii_case(MIME_TYPE_H264) {
                    *status.lock().expect("webrtc_status") =
                        "WebRTC video track receiving".to_string();
                    let mut builder = webrtc_media::io::sample_builder::SampleBuilder::new(
                        16,
                        webrtc::rtp::codecs::h264::H264Packet::default(),
                        VIDEO_CLOCK_RATE,
                    );
                    let Ok(mut decoder) = H264Decoder::new() else {
                        *status.lock().expect("webrtc_status") = "H264 decoder failed".to_string();
                        return;
                    };
                    let mut frames = 0_u64;
                    while let Ok((packet, _)) = track.read_rtp().await {
                        builder.push(packet);
                        while let Some(sample) = builder.pop() {
                            match decoder.decode(&sample.data) {
                                Ok(Some(decoded)) => {
                                    let (width, height) = decoded.dimensions();
                                    let mut rgb = vec![0_u8; decoded.rgb8_len()];
                                    decoded.write_rgb8(&mut rgb);
                                    frames = frames.saturating_add(1);
                                    *remote_video.lock().expect("remote_video") =
                                        Some(VideoFrameInfo {
                                            width: width as u32,
                                            height: height as u32,
                                            frames,
                                            rgb,
                                        });
                                }
                                Ok(None) => {}
                                Err(err) => {
                                    *status.lock().expect("webrtc_status") =
                                        format!("H264 decode failed: {err}");
                                }
                            }
                        }
                    }
                    return;
                }
                if !mime_type.eq_ignore_ascii_case(MIME_TYPE_OPUS) {
                    return;
                }
                let Ok(mut decoder) = Decoder::new(OPUS_SAMPLE_RATE, Channels::Mono) else {
                    *status.lock().expect("webrtc_status") = "Opus decoder failed".to_string();
                    return;
                };
                let mut output = vec![0_f32; OPUS_FRAME_SAMPLES * 6];
                while let Ok((packet, _)) = track.read_rtp().await {
                    let Ok(samples) = decoder.decode_float(&packet.payload, &mut output, false)
                    else {
                        *status.lock().expect("webrtc_status") = "Opus decode failed".to_string();
                        continue;
                    };
                    let mut queue = playback_queue.lock().expect("audio_playback_queue");
                    let peak = output[..samples]
                        .iter()
                        .map(|sample| sample.abs())
                        .fold(0.0_f32, f32::max)
                        .clamp(0.0, 1.0);
                    remote_audio_level.store((peak * 1000.0) as u32, Ordering::Relaxed);
                    queue.extend(output[..samples].iter().copied());
                    while queue.len() > PLAYBACK_BUFFER_MAX {
                        queue.pop_front();
                    }
                }
            })
        }));
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
            attach_data_receiver(
                channel,
                playback_queue.clone(),
                remote_video.clone(),
                status.clone(),
            );
        } else {
            let status_for_channel = status.clone();
            let data_channel_for_handler = data_channel.clone();
            let playback_queue_for_handler = playback_queue.clone();
            let remote_video_for_handler = remote_video.clone();
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
                attach_data_receiver(
                    channel,
                    playback_queue_for_handler.clone(),
                    remote_video_for_handler.clone(),
                    status_for_channel.clone(),
                );
                Box::pin(async {})
            }));
        }
        Ok(Self {
            runtime,
            peer_connection,
            status,
            remote_audio_level,
            _data_channel: data_channel,
            outbound_audio_tx,
            outbound_video_tx,
            playback_queue,
            remote_video,
            _audio_track: audio_track,
            _video_track: video_track,
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
    processing: AudioProcessingConfig,
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
            processing,
        )?,
        cpal::SampleFormat::I16 => build_input_stream::<i16>(
            &device,
            config.into(),
            level.clone(),
            sample_rate,
            channels,
            audio_tx,
            processing,
        )?,
        cpal::SampleFormat::U16 => build_input_stream::<u16>(
            &device,
            config.into(),
            level.clone(),
            sample_rate,
            channels,
            audio_tx,
            processing,
        )?,
        other => return Err(anyhow!("unsupported microphone sample format: {other:?}")),
    };
    stream.play()?;
    Ok(MediaSession {
        _stream: stream,
        level,
    })
}

pub fn start_camera_capture(
    device_name: String,
    video_tx: Option<mpsc::Sender<VideoFrameInfo>>,
) -> VideoCaptureSession {
    let stop = Arc::new(AtomicBool::new(false));
    let latest = Arc::new(Mutex::new(None));
    let status = Arc::new(Mutex::new("Camera starting".to_string()));
    let stop_for_thread = stop.clone();
    let latest_for_thread = latest.clone();
    let status_for_thread = status.clone();
    thread::spawn(move || {
        let result = run_camera_capture(
            &device_name,
            stop_for_thread,
            latest_for_thread,
            status_for_thread.clone(),
            video_tx,
        );
        if let Err(err) = result {
            *status_for_thread.lock().expect("video_capture_status") =
                format!("Camera capture failed: {err}");
        }
    });
    VideoCaptureSession {
        stop,
        latest,
        status,
    }
}

fn run_camera_capture(
    device_name: &str,
    stop: Arc<AtomicBool>,
    latest: Arc<Mutex<Option<VideoFrameInfo>>>,
    status: Arc<Mutex<String>>,
    video_tx: Option<mpsc::Sender<VideoFrameInfo>>,
) -> Result<()> {
    let _ = nokhwa_initialize(|_| {});
    let backend = native_api_backend().ok_or_else(|| anyhow!("camera backend unavailable"))?;
    let cameras = query(backend)?;
    let index = if device_name == SYSTEM_DEFAULT_DEVICE {
        cameras
            .first()
            .map(|camera| camera.index().clone())
            .unwrap_or_else(|| CameraIndex::Index(0))
    } else {
        cameras
            .iter()
            .find(|camera| camera.human_name() == device_name)
            .map(|camera| camera.index().clone())
            .ok_or_else(|| anyhow!("camera not found"))?
    };
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::with_backend(index, requested, backend)?;
    camera.open_stream()?;
    *status.lock().expect("video_capture_status") = "Camera capture active".to_string();
    let mut frames = 0_u64;
    let mut last_sent = Instant::now();
    let mut last_frame = Instant::now() - CAMERA_FRAME_INTERVAL;
    while !stop.load(Ordering::Relaxed) {
        let elapsed = last_frame.elapsed();
        if elapsed < CAMERA_FRAME_INTERVAL {
            thread::sleep(CAMERA_FRAME_INTERVAL - elapsed);
        }
        let frame = camera.frame()?;
        last_frame = Instant::now();
        let resolution = frame.resolution();
        let image = frame.decode_image::<RgbFormat>()?;
        let (width, height, rgb) = preview_rgb(image.width(), image.height(), image.as_raw());
        frames = frames.saturating_add(1);
        let info = VideoFrameInfo {
            width,
            height,
            frames,
            rgb,
        };
        *latest.lock().expect("video_capture_latest") = Some(info.clone());
        if last_sent.elapsed() >= VIDEO_FRAME_DURATION {
            if let Some(video_tx) = video_tx.as_ref() {
                let _ = video_tx.send(info);
            }
            last_sent = Instant::now();
        }
        let _ = resolution;
    }
    let _ = camera.stop_stream();
    Ok(())
}

fn preview_rgb(width: u32, height: u32, rgb: &[u8]) -> (u32, u32, Vec<u8>) {
    let target_width = even_dimension(width.min(320));
    let target_height =
        ((height.max(1) as u64 * target_width as u64) / width.max(1) as u64).max(1) as u32;
    let target_height = even_dimension(target_height);
    if target_width == width && target_height == height {
        return (width, height, rgb.to_vec());
    }
    let mut out = vec![0_u8; target_width as usize * target_height as usize * 3];
    for y in 0..target_height {
        let source_y = (y as u64 * height as u64 / target_height as u64) as u32;
        for x in 0..target_width {
            let source_x = (x as u64 * width as u64 / target_width as u64) as u32;
            let source = ((source_y * width + source_x) * 3) as usize;
            let target = ((y * target_width + x) * 3) as usize;
            if source + 2 < rgb.len() && target + 2 < out.len() {
                out[target..target + 3].copy_from_slice(&rgb[source..source + 3]);
            }
        }
    }
    (target_width, target_height, out)
}

fn even_dimension(value: u32) -> u32 {
    value.clamp(2, u32::MAX) & !1
}

fn build_input_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    level: Arc<AtomicU32>,
    sample_rate: u32,
    channels: usize,
    audio_tx: Option<mpsc::Sender<Vec<u8>>>,
    processing: AudioProcessingConfig,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + SampleLevel,
{
    let mut frame = Vec::with_capacity((sample_rate / 50) as usize);
    let frame_samples = (sample_rate / 50).max(160) as usize;
    let mut agc_gain = 1.0_f32;
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
                    let mut mono = chunk
                        .iter()
                        .map(SampleLevel::sample_level_signed)
                        .sum::<f32>()
                        / chunk.len().max(1) as f32;
                    if processing.noise_suppression && mono.abs() < 0.012 {
                        mono = 0.0;
                    }
                    if processing.automatic_gain_control {
                        let level = mono.abs();
                        if level > 0.001 {
                            let target_gain = (0.18 / level).clamp(0.5, 4.0);
                            agc_gain = (agc_gain * 0.995 + target_gain * 0.005).clamp(0.5, 4.0);
                        }
                        mono *= agc_gain;
                    }
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

fn attach_data_receiver(
    channel: Arc<RTCDataChannel>,
    playback_queue: Arc<Mutex<VecDeque<f32>>>,
    remote_video: Arc<Mutex<Option<VideoFrameInfo>>>,
    status: Arc<Mutex<String>>,
) {
    channel.on_message(Box::new(move |message: DataChannelMessage| {
        if let Some(frame) = decode_video_frame(&message.data) {
            *remote_video.lock().expect("remote_video") = Some(frame);
        } else if let Some(samples) = decode_audio_frame(&message.data) {
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

pub fn play_call_tone(device_name: String) {
    thread::spawn(move || {
        let _ = run_call_tone(&device_name);
    });
}

fn run_call_tone(device_name: &str) -> Result<()> {
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
    .or_else(|| host.default_output_device())
    .ok_or_else(|| anyhow!("speaker not found"))?;
    let config = device.default_output_config()?;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_tone_stream::<f32>(&device, config.into())?,
        cpal::SampleFormat::I16 => build_tone_stream::<i16>(&device, config.into())?,
        cpal::SampleFormat::U16 => build_tone_stream::<u16>(&device, config.into())?,
        other => return Err(anyhow!("unsupported speaker sample format: {other:?}")),
    };
    stream.play()?;
    thread::sleep(Duration::from_millis(1200));
    Ok(())
}

fn build_tone_stream<T>(device: &cpal::Device, config: cpal::StreamConfig) -> Result<cpal::Stream>
where
    T: cpal::SizedSample + OutputSample,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    let mut sample_index = 0_u64;
    Ok(device.build_output_stream(
        &config,
        move |out: &mut [T], _| {
            for frame in out.chunks_mut(channels) {
                let t = sample_index as f32 / sample_rate;
                let frequency = if (t * 4.0) as u32 % 2 == 0 {
                    740.0
                } else {
                    920.0
                };
                let value = (t * frequency * std::f32::consts::TAU).sin() * 0.18;
                for sample in frame {
                    *sample = T::from_f32(value);
                }
                sample_index = sample_index.saturating_add(1);
            }
        },
        move |_| {},
        None,
    )?)
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

fn decode_video_frame(payload: &[u8]) -> Option<VideoFrameInfo> {
    if payload.len() < 20 || &payload[..4] != b"RSV1" {
        return None;
    }
    let width = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let height = u32::from_le_bytes([payload[8], payload[9], payload[10], payload[11]]);
    let frames = u64::from_le_bytes([
        payload[12],
        payload[13],
        payload[14],
        payload[15],
        payload[16],
        payload[17],
        payload[18],
        payload[19],
    ]);
    let expected = width as usize * height as usize * 3;
    if payload.len() - 20 != expected {
        return None;
    }
    Some(VideoFrameInfo {
        width,
        height,
        frames,
        rgb: payload[20..].to_vec(),
    })
}

fn decode_audio_frame(payload: &[u8]) -> Option<Vec<f32>> {
    decode_audio_frame_with_rate(payload).map(|(_, samples)| samples)
}

fn decode_audio_frame_with_rate(payload: &[u8]) -> Option<(u32, Vec<f32>)> {
    if payload.len() < 8 || &payload[..4] != b"RSA1" {
        return None;
    }
    let sample_rate = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    let mut samples = Vec::with_capacity((payload.len() - 8) / 4);
    for chunk in payload[8..].chunks_exact(4) {
        samples.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).clamp(-1.0, 1.0));
    }
    Some((sample_rate, samples))
}

fn resample_to_opus(sample_rate: u32, samples: &[f32]) -> Vec<f32> {
    if sample_rate == OPUS_SAMPLE_RATE && samples.len() == OPUS_FRAME_SAMPLES {
        return samples.to_vec();
    }
    if samples.is_empty() {
        return vec![0.0; OPUS_FRAME_SAMPLES];
    }
    let ratio = sample_rate as f32 / OPUS_SAMPLE_RATE as f32;
    (0..OPUS_FRAME_SAMPLES)
        .map(|index| {
            let source = index as f32 * ratio;
            let lower = source.floor() as usize;
            let upper = (lower + 1).min(samples.len() - 1);
            let mix = source - lower as f32;
            (samples[lower] * (1.0 - mix) + samples[upper] * mix).clamp(-1.0, 1.0)
        })
        .collect()
}
