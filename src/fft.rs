use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

const FFT_SIZE: usize = 2048;

/// Frequency range filter for spectrum bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FreqRange {
    #[default]
    Full,
    Bass,
    Mids,
}

impl FreqRange {
    pub fn next(self) -> Self {
        match self {
            Self::Full => Self::Bass,
            Self::Bass => Self::Mids,
            Self::Mids => Self::Full,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Bass => "Bass",
            Self::Mids => "Mids",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "bass" => Some(Self::Bass),
            "mids" => Some(Self::Mids),
            _ => None,
        }
    }

    fn bounds_hz(self) -> (f32, f32) {
        match self {
            Self::Full => (30.0, 16_000.0),
            Self::Bass => (25.0, 250.0),
            Self::Mids => (250.0, 4_000.0),
        }
    }
}

/// Parameters for a single spectrum analysis frame.
#[derive(Debug, Clone, Copy)]
pub struct AnalyzeParams {
    pub num_bars: usize,
    pub freq_range: FreqRange,
    pub gain: f32,
    pub auto_gain: bool,
    pub dt_secs: f32,
}

/// Computes smoothed frequency-band magnitudes, peak-hold values, and beat/note metrics.
pub struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    bars: Vec<f32>,
    peaks: Vec<f32>,
    peak_velocities: Vec<f32>,
    history: VecDeque<Vec<f32>>,
    bass_avg: f32,
    bass_pulse: f32,
    prev_bass_pulse: f32,
    agc_gain: f32,
    elapsed_secs: f64,
    last_beat_time: Option<f64>,
    beat_intervals: VecDeque<f64>,
    estimated_bpm: Option<f32>,
}

impl SpectrumAnalyzer {
    /// Creates a new `SpectrumAnalyzer` with a fixed 2048-sample FFT window.
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
            .collect();

        Self {
            fft,
            window,
            buffer: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            bars: Vec::new(),
            peaks: Vec::new(),
            peak_velocities: Vec::new(),
            history: VecDeque::with_capacity(96),
            bass_avg: 0.0,
            bass_pulse: 0.0,
            prev_bass_pulse: 0.0,
            agc_gain: 1.0,
            elapsed_secs: 0.0,
            last_beat_time: None,
            beat_intervals: VecDeque::with_capacity(12),
            estimated_bpm: None,
        }
    }

    /// Processes the latest mono PCM samples and updates smoothed bands.
    pub fn process(&mut self, samples: &[f32], sample_rate: u32, params: AnalyzeParams) {
        let AnalyzeParams {
            num_bars,
            freq_range,
            gain,
            auto_gain,
            dt_secs,
        } = params;
        if num_bars == 0 {
            self.bars.clear();
            self.peaks.clear();
            self.peak_velocities.clear();
            return;
        }

        if self.bars.len() != num_bars {
            self.bars.resize(num_bars, 0.0);
            self.peaks.resize(num_bars, 0.0);
            self.peak_velocities.resize(num_bars, 0.0);
        }

        // Fill FFT buffer with latest samples (zero-padded on the left if fewer than FFT_SIZE).
        let available = samples.len().min(FFT_SIZE);
        let pad = FFT_SIZE - available;
        for (i, cell) in self.buffer.iter_mut().enumerate() {
            let sample = if i < pad {
                0.0
            } else {
                samples[samples.len() - available + (i - pad)]
            };
            *cell = Complex::new(sample * self.window[i], 0.0);
        }

        self.fft.process(&mut self.buffer);

        let (range_min, range_max) = freq_range.bounds_hz();
        let nyquist = (sample_rate.max(8000) as f32) * 0.5;
        let max_freq = range_max.min(nyquist * 0.95);
        let min_freq = range_min.min(max_freq * 0.5);
        let half_bins = FFT_SIZE / 2;
        let bin_hz = (sample_rate.max(8000) as f32) / (FFT_SIZE as f32);

        let attack = 1.0 - (-dt_secs * 28.0).exp();
        let decay = 1.0 - (-dt_secs * 7.5).exp();
        let gravity = 1.6 * dt_secs;

        let effective_gain = if auto_gain {
            gain * self.agc_gain
        } else {
            gain
        };
        let mut frame_raw_peak = 0.0_f32;

        for bar_idx in 0..num_bars {
            let t0 = bar_idx as f32 / num_bars as f32;
            let t1 = (bar_idx + 1) as f32 / num_bars as f32;

            let f0 = min_freq * (max_freq / min_freq).powf(t0);
            let f1 = min_freq * (max_freq / min_freq).powf(t1);

            let bin_start = ((f0 / bin_hz).floor() as usize).clamp(1, half_bins.saturating_sub(1));
            let bin_end = ((f1 / bin_hz).ceil() as usize).clamp(bin_start + 1, half_bins);

            let mut max_mag = 0.0_f32;
            for bin in &self.buffer[bin_start..bin_end] {
                let mag = bin.norm() / (FFT_SIZE as f32 * 0.25);
                if mag > max_mag {
                    max_mag = mag;
                }
            }

            // Slight high-frequency tilt compensation so treble isn't drowned out by bass.
            let tilt = 1.0 + 1.4 * t0;
            let raw_tilted = max_mag * tilt;
            if raw_tilted > frame_raw_peak {
                frame_raw_peak = raw_tilted;
            }
            let scaled = raw_tilted * effective_gain;

            // Convert to perceptual 0..1 range via soft log compression.
            let db = 20.0 * (scaled + 1e-6).log10();
            let target = ((db + 54.0) / 54.0).clamp(0.0, 1.0);

            let prev = self.bars[bar_idx];
            let next = if target > prev {
                prev + (target - prev) * attack
            } else {
                prev + (target - prev) * decay
            };
            self.bars[bar_idx] = next;

            // Peak-hold with gravity.
            if next >= self.peaks[bar_idx] {
                self.peaks[bar_idx] = next;
                self.peak_velocities[bar_idx] = 0.0;
            } else {
                self.peak_velocities[bar_idx] += gravity;
                self.peaks[bar_idx] = (self.peaks[bar_idx] - self.peak_velocities[bar_idx] * dt_secs).max(next);
            }
        }

        // Compute Bass Pulse from the lowest ~15% of frequency bands (roughly 30-120 Hz).
        let bass_count = (num_bars / 6).max(1).min(num_bars);
        let bass_now: f32 =
            self.bars[..bass_count].iter().sum::<f32>() / (bass_count as f32);
        let avg_rate = 1.0 - (-dt_secs * 2.5).exp();
        self.bass_avg += (bass_now - self.bass_avg) * avg_rate;

        let transient = ((bass_now - self.bass_avg * 0.85) * 2.2).clamp(0.0, 1.0);
        let pulse_rate = if transient > self.bass_pulse {
            1.0 - (-dt_secs * 30.0).exp()
        } else {
            1.0 - (-dt_secs * 8.0).exp()
        };
        self.prev_bass_pulse = self.bass_pulse;
        self.bass_pulse += (transient - self.bass_pulse) * pulse_rate;
        self.update_bpm(f64::from(dt_secs));

        if auto_gain && frame_raw_peak > 0.002 {
            let desired_gain = (0.38 / (frame_raw_peak * gain.max(0.1))).clamp(0.35, 6.0);
            let agc_rate = if desired_gain < self.agc_gain {
                1.0 - (-dt_secs * 3.5).exp()
            } else {
                1.0 - (-dt_secs * 0.9).exp()
            };
            self.agc_gain += (desired_gain - self.agc_gain) * agc_rate;
        } else if !auto_gain {
            self.agc_gain = 1.0;
        }

        if self.history.len() >= 96 {
            self.history.pop_back();
        }
        self.history.push_front(self.bars.clone());
    }

    /// Returns the current smoothed bar heights in `[0.0, 1.0]`.
    pub fn bars(&self) -> &[f32] {
        &self.bars
    }

    /// Returns the current peak-hold positions in `[0.0, 1.0]`.
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    /// Returns a beat-reactive bass pulse in `[0.0, 1.0]`.
    pub fn bass_pulse(&self) -> f32 {
        self.bass_pulse
    }

    /// Returns the estimated tempo in BPM when steady beats are detected.
    pub fn bpm(&self) -> Option<u16> {
        self.estimated_bpm.map(|b| b.round() as u16)
    }

    /// Returns recent spectrum frames (newest first) for waterfall rendering.
    pub fn history(&self) -> &VecDeque<Vec<f32>> {
        &self.history
    }

    fn update_bpm(&mut self, dt_secs: f64) {
        self.elapsed_secs += dt_secs;

        if let Some(last) = self.last_beat_time
            && self.elapsed_secs - last > 3.5
        {
            self.beat_intervals.clear();
            self.estimated_bpm = None;
            self.last_beat_time = None;
        }

        if self.bass_pulse >= 0.42 && self.prev_bass_pulse < 0.42 {
            if let Some(last) = self.last_beat_time {
                let delta = self.elapsed_secs - last;
                if delta < 0.28 {
                    return;
                }
                if delta <= 1.6 {
                    if self.beat_intervals.len() >= 10 {
                        self.beat_intervals.pop_front();
                    }
                    self.beat_intervals.push_back(delta);
                } else {
                    self.beat_intervals.clear();
                }
            }
            self.last_beat_time = Some(self.elapsed_secs);

            if self.beat_intervals.len() >= 3 {
                let mut sorted: Vec<f64> = self.beat_intervals.iter().copied().collect();
                sorted.sort_by(f64::total_cmp);
                let median = sorted[sorted.len() / 2];
                if median > 0.0 {
                    let mut bpm = (60.0 / median) as f32;
                    while bpm > 178.0 {
                        bpm *= 0.5;
                    }
                    while bpm < 68.0 {
                        bpm *= 2.0;
                    }
                    self.estimated_bpm = Some(match self.estimated_bpm {
                        Some(prev) => prev + (bpm - prev) * 0.25,
                        None => bpm,
                    });
                }
            }
        }
    }
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_produces_zero_bars() {
        let mut analyzer = SpectrumAnalyzer::new();
        let silence = vec![0.0_f32; FFT_SIZE];
        analyzer.process(
            &silence,
            48_000,
            AnalyzeParams {
                num_bars: 32,
                freq_range: FreqRange::Full,
                gain: 1.0,
                auto_gain: false,
                dt_secs: 1.0 / 60.0,
            },
        );
        assert_eq!(analyzer.bars().len(), 32);
        assert!(analyzer.bars().iter().all(|&v| v < 0.01));
    }

    #[test]
    fn sine_wave_excites_spectrum() {
        let mut analyzer = SpectrumAnalyzer::new();
        let sr = 48_000.0_f32;
        let freq = 440.0_f32;
        let samples: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (2.0 * PI * freq * i as f32 / sr).sin())
            .collect();

        for _ in 0..10 {
            analyzer.process(
                &samples,
                48_000,
                AnalyzeParams {
                    num_bars: 32,
                    freq_range: FreqRange::Full,
                    gain: 1.0,
                    auto_gain: false,
                    dt_secs: 1.0 / 60.0,
                },
            );
        }

        let max_bar = analyzer.bars().iter().copied().fold(0.0_f32, f32::max);
        assert!(max_bar > 0.3, "expected 440Hz sine to excite bars, got {max_bar}");
    }
}
