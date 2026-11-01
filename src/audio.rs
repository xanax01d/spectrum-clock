use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};

const RING_CAPACITY: usize = 4096;
const STEREO_CAPACITY: usize = 1024;

#[derive(Default)]
struct SharedAudio {
    mono: VecDeque<f32>,
    stereo_pairs: VecDeque<(f32, f32)>,
    stereo_peak: (f32, f32),
}

/// Captures system audio via Windows WASAPI loopback (or default input on other OSes).
pub struct AudioCapture {
    _stream: Stream,
    shared: Arc<Mutex<SharedAudio>>,
    sample_rate: u32,
    device_name: String,
}

impl AudioCapture {
    /// Starts capturing audio: WASAPI loopback on Windows, default input device on other OSes.
    pub fn start_loopback() -> Result<Self> {
        let host = cpal::default_host();

        #[cfg(target_os = "windows")]
        let (device, supported_config) = {
            let device = host
                .default_output_device()
                .ok_or_else(|| anyhow!("No default audio output device found"))?;
            let config = device
                .default_output_config()
                .context("Failed to query default output audio format")?;
            (device, config)
        };

        #[cfg(not(target_os = "windows"))]
        let (device, supported_config) = {
            let device = host
                .default_input_device()
                .ok_or_else(|| anyhow!("No default audio input device found"))?;
            let config = device
                .default_input_config()
                .context("Failed to query default input audio format")?;
            (device, config)
        };

        let device_name = device
            .name()
            .unwrap_or_else(|_| "Default Audio".to_string());

        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels.max(1) as usize;

        let shared = Arc::new(Mutex::new(SharedAudio {
            mono: VecDeque::with_capacity(RING_CAPACITY),
            stereo_pairs: VecDeque::with_capacity(STEREO_CAPACITY),
            stereo_peak: (0.0, 0.0),
        }));
        let shared_writer = Arc::clone(&shared);

        let stream = build_loopback_stream(&device, &config, sample_format, channels, shared_writer)?;
        stream.play().context("Failed to start audio loopback stream")?;

        Ok(Self {
            _stream: stream,
            shared,
            sample_rate,
            device_name,
        })
    }

    /// Copies the latest mono PCM samples and stereo `(L, R)` pairs, returning `(left_vu, right_vu)`.
    pub fn read_samples(&self, out: &mut Vec<f32>, stereo_out: &mut Vec<(f32, f32)>) -> (f32, f32) {
        out.clear();
        stereo_out.clear();
        if let Ok(mut guard) = self.shared.lock() {
            out.extend(guard.mono.iter().copied());
            stereo_out.extend(guard.stereo_pairs.iter().copied());
            let levels = guard.stereo_peak;
            // Decay peak between UI reads so silence drops to 0.
            guard.stereo_peak.0 *= 0.85;
            guard.stereo_peak.1 *= 0.85;
            levels
        } else {
            (0.0, 0.0)
        }
    }

    /// Returns the stream sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the name of the captured audio device.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

fn build_loopback_stream(
    device: &Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    channels: usize,
    shared: Arc<Mutex<SharedAudio>>,
) -> Result<Stream> {
    let err_fn = |err| {
        let _ = err;
    };

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| push_audio_samples(data, channels, &shared, |s| s),
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                push_audio_samples(data, channels, &shared, |s| f32::from(s) / f32::from(i16::MAX))
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                push_audio_samples(data, channels, &shared, |s| {
                    (f32::from(s) - 32768.0) / 32768.0
                })
            },
            err_fn,
            None,
        ),
        other => {
            return Err(anyhow!("Unsupported audio sample format: {other:?}"));
        }
    }
    .context("Failed to build audio input stream")?;

    Ok(stream)
}

fn push_audio_samples<T: Copy>(
    data: &[T],
    channels: usize,
    shared: &Arc<Mutex<SharedAudio>>,
    to_f32: impl Fn(T) -> f32,
) {
    let Ok(mut guard) = shared.lock() else {
        return;
    };

    let mut peak_l = 0.0_f32;
    let mut peak_r = 0.0_f32;

    for frame in data.chunks_exact(channels) {
        let l_raw = to_f32(frame[0]);
        let r_raw = if channels > 1 {
            to_f32(frame[1])
        } else {
            l_raw
        };
        let l = l_raw.abs();
        let r = r_raw.abs();
        if l > peak_l {
            peak_l = l;
        }
        if r > peak_r {
            peak_r = r;
        }

        let sum: f32 = frame.iter().map(|&s| to_f32(s)).sum();
        let mono = sum / channels as f32;
        if guard.mono.len() >= RING_CAPACITY {
            guard.mono.pop_front();
        }
        guard.mono.push_back(mono);

        if guard.stereo_pairs.len() >= STEREO_CAPACITY {
            guard.stereo_pairs.pop_front();
        }
        guard.stereo_pairs.push_back((l_raw, r_raw));
    }

    guard.stereo_peak.0 = guard.stereo_peak.0.max(peak_l.min(1.0));
    guard.stereo_peak.1 = guard.stereo_peak.1.max(peak_r.min(1.0));
}
