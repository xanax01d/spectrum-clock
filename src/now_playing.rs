use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::theme::SystemPalette;

/// Playback control commands sent to the OS media session.
#[derive(Debug, Clone, Copy)]
pub enum MediaCommand {
    TogglePlayPause,
    Next,
    Previous,
}

/// Metadata, progress, and optional album-art palette/thumbnail for the currently playing track.
#[derive(Debug, Clone)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub is_playing: bool,
    pub cover_palette: Option<SystemPalette>,
    pub cover_art: Option<Vec<(u8, u8, u8)>>,
    pub position_secs: Option<f64>,
    pub duration_secs: Option<f64>,
    pub updated_at: Instant,
}

impl NowPlaying {
    /// Formats the track as `"Artist — Title"` (or just `"Title"` if artist is empty).
    pub fn display_text(&self) -> String {
        if self.artist.is_empty() {
            self.title.clone()
        } else if self.title.is_empty() {
            self.artist.clone()
        } else {
            format!("{} — {}", self.artist, self.title)
        }
    }

    /// Returns `(current_position_secs, total_duration_secs)` interpolated smoothly when playing.
    pub fn current_progress(&self) -> Option<(f64, f64)> {
        let pos = self.position_secs?;
        let dur = self.duration_secs?;
        if dur <= 0.0 {
            return None;
        }
        let elapsed = if self.is_playing {
            self.updated_at.elapsed().as_secs_f64()
        } else {
            0.0
        };
        Some(((pos + elapsed).clamp(0.0, dur), dur))
    }
}

/// Polls the OS media session in the background and handles media control commands.
pub struct NowPlayingWatcher {
    current: Arc<Mutex<Option<NowPlaying>>>,
    cmd_tx: Option<Sender<MediaCommand>>,
}

impl NowPlayingWatcher {
    /// Starts the background media watcher (Windows GSMTC on Windows, no-op on other OSes).
    pub fn start() -> Self {
        let current = Arc::new(Mutex::new(None));

        #[cfg(target_os = "windows")]
        let cmd_tx = {
            let (tx, rx) = mpsc::channel();
            let shared = Arc::clone(&current);
            std::thread::Builder::new()
                .name("now-playing-watcher".to_string())
                .spawn(move || windows_impl::poll_loop(&shared, &rx))
                .ok();
            Some(tx)
        };

        #[cfg(not(target_os = "windows"))]
        let cmd_tx = {
            let _ = mpsc::channel::<MediaCommand>();
            None
        };

        Self { current, cmd_tx }
    }

    /// Returns a snapshot of the currently playing track, if available.
    pub fn latest(&self) -> Option<NowPlaying> {
        self.current.try_lock().ok().and_then(|g| g.clone())
    }

    /// Sends a playback command (play/pause, next, previous) to the OS media session.
    pub fn send_command(&self, cmd: MediaCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(cmd);
        }
    }
}

/// Extracts a 12x12 RGB pixel grid from encoded album art bytes for terminal half-block rendering.
pub fn extract_cover_art(bytes: &[u8]) -> Option<Vec<(u8, u8, u8)>> {
    let img = image::load_from_memory(bytes).ok()?;
    let small = img
        .resize_exact(12, 12, image::imageops::FilterType::Triangle)
        .to_rgb8();
    Some(small.pixels().map(|p| (p.0[0], p.0[1], p.0[2])).collect())
}

/// Extracts a 3-color terminal-friendly palette from encoded album art bytes (PNG/JPEG/WebP).
pub fn extract_cover_palette(bytes: &[u8]) -> Option<SystemPalette> {
    let img = image::load_from_memory(bytes).ok()?;
    let small = img
        .resize_exact(24, 24, image::imageops::FilterType::Triangle)
        .to_rgb8();

    let mut best_primary: Option<((u8, u8, u8), f32)> = None;
    let mut sum_r = 0u32;
    let mut sum_g = 0u32;
    let mut sum_b = 0u32;
    let mut count = 0u32;

    for pixel in small.pixels() {
        let [r, g, b] = pixel.0;
        sum_r += u32::from(r);
        sum_g += u32::from(g);
        sum_b += u32::from(b);
        count += 1;

        let score = vibrancy_score(r, g, b);
        if best_primary.is_none_or(|(_, best)| score > best) {
            best_primary = Some(((r, g, b), score));
        }
    }

    let avg = (
        sum_r.checked_div(count)? as u8,
        sum_g.checked_div(count)? as u8,
        sum_b.checked_div(count)? as u8,
    );

    let primary = match best_primary {
        Some((rgb, score)) if score > 0.08 => boost_visibility(rgb),
        _ => boost_visibility(avg),
    };

    // Find a distinct secondary vibrant color from the cover for gradient richness.
    let mut best_secondary: Option<((u8, u8, u8), f32)> = None;
    for pixel in small.pixels() {
        let [r, g, b] = pixel.0;
        let dist = color_distance(primary, (r, g, b));
        if dist < 65.0 {
            continue;
        }
        let score = vibrancy_score(r, g, b) * (dist / 255.0).min(1.5);
        if best_secondary.is_none_or(|(_, best)| score > best) {
            best_secondary = Some(((r, g, b), score));
        }
    }

    let dark = match best_secondary {
        Some((sec, score)) if score > 0.06 => darken_for_base(sec),
        _ => darken_for_base(primary),
    };

    let light = lighten_for_peak(primary);

    Some(SystemPalette {
        dark,
        base: primary,
        light,
        time_secs: 0.0,
    })
}

fn vibrancy_score(r: u8, g: u8, b: u8) -> f32 {
    let rf = f32::from(r) / 255.0;
    let gf = f32::from(g) / 255.0;
    let bf = f32::from(b) / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;
    let luma = 0.299 * rf + 0.587 * gf + 0.114 * bf;

    let sat = if max > 0.0 { delta / max } else { 0.0 };
    // Prefer rich mid-brightness colors over near-black or near-white.
    let luma_weight = (1.0 - (luma - 0.55).abs() * 1.4).max(0.1);
    sat * luma_weight
}

fn color_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let dr = f32::from(a.0) - f32::from(b.0);
    let dg = f32::from(a.1) - f32::from(b.1);
    let db = f32::from(a.2) - f32::from(b.2);
    (dr * dr + dg * dg + db * db).sqrt()
}

fn boost_visibility(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let max = rgb.0.max(rgb.1).max(rgb.2);
    if max < 90 && max > 0 {
        let factor = 140.0 / f32::from(max);
        (
            (f32::from(rgb.0) * factor).min(255.0) as u8,
            (f32::from(rgb.1) * factor).min(255.0) as u8,
            (f32::from(rgb.2) * factor).min(255.0) as u8,
        )
    } else if max == 0 {
        (120, 140, 180)
    } else {
        rgb
    }
}

fn darken_for_base(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    (
        (f32::from(rgb.0) * 0.55).max(18.0) as u8,
        (f32::from(rgb.1) * 0.55).max(18.0) as u8,
        (f32::from(rgb.2) * 0.55).max(18.0) as u8,
    )
}

fn lighten_for_peak(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let blend = |c: u8| -> u8 { (f32::from(c) + (255.0 - f32::from(c)) * 0.45) as u8 };
    (blend(rgb.0), blend(rgb.1), blend(rgb.2))
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::sync::mpsc::Receiver;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus,
    };
    use windows::Storage::Streams::DataReader;

    use super::{MediaCommand, NowPlaying, SystemPalette, extract_cover_art, extract_cover_palette};

    const MAX_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;
    const TICKS_PER_SEC: f64 = 10_000_000.0;
    const FILETIME_UNIX_EPOCH_SECS: i64 = 11_644_473_600;

    #[derive(Default)]
    struct PollState {
        track_key: String,
        palette: Option<SystemPalette>,
        art: Option<Vec<(u8, u8, u8)>>,
        last_raw_pos: Option<f64>,
        anchor_pos: Option<f64>,
        anchor_time: Option<Instant>,
        was_playing: bool,
    }

    pub fn poll_loop(shared: &Arc<Mutex<Option<NowPlaying>>>, rx: &Receiver<MediaCommand>) {
        let manager = match GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
            .and_then(|op| op.get())
        {
            Ok(m) => m,
            Err(_) => return,
        };

        let mut state = PollState::default();
        let mut last_poll = Instant::now() - Duration::from_secs(5);

        loop {
            if last_poll.elapsed() >= Duration::from_millis(1000) {
                let track = fetch_current(&manager, &mut state);
                if let Ok(mut guard) = shared.lock() {
                    *guard = track;
                }
                last_poll = Instant::now();
            }

            if let Ok(cmd) = rx.recv_timeout(Duration::from_millis(150)) {
                handle_command(&manager, cmd);
                // Force a quick refresh after a media command.
                last_poll = Instant::now() - Duration::from_millis(850);
            }
        }
    }

    fn handle_command(
        manager: &GlobalSystemMediaTransportControlsSessionManager,
        cmd: MediaCommand,
    ) {
        let Ok(session) = manager.GetCurrentSession() else {
            return;
        };
        match cmd {
            MediaCommand::TogglePlayPause => {
                if let Ok(op) = session.TryTogglePlayPauseAsync() {
                    let _ = op.get();
                }
            }
            MediaCommand::Next => {
                if let Ok(op) = session.TrySkipNextAsync() {
                    let _ = op.get();
                }
            }
            MediaCommand::Previous => {
                if let Ok(op) = session.TrySkipPreviousAsync() {
                    let _ = op.get();
                }
            }
        }
    }

    fn fetch_current(
        manager: &GlobalSystemMediaTransportControlsSessionManager,
        state: &mut PollState,
    ) -> Option<NowPlaying> {
        let session = manager.GetCurrentSession().ok()?;
        let playback = session.GetPlaybackInfo().ok()?;
        let status = playback.PlaybackStatus().ok()?;
        let is_playing =
            status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;

        let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let title = props.Title().ok()?.to_string_lossy().trim().to_string();
        let artist = props.Artist().ok()?.to_string_lossy().trim().to_string();

        if title.is_empty() && artist.is_empty() {
            *state = PollState::default();
            return None;
        }

        let track_key = format!("{artist}\0{title}");
        let track_changed = track_key != state.track_key;
        if track_changed {
            state.track_key = track_key;
            let (palette, art) = read_thumbnail_data(&props);
            state.palette = palette;
            state.art = art;
            state.last_raw_pos = None;
            state.anchor_pos = None;
            state.anchor_time = None;
        }

        let (raw_pos, duration_secs) = read_timeline(&session, is_playing);
        let now = Instant::now();

        let (position_secs, updated_at) = match raw_pos {
            Some(pos) => {
                let play_state_changed = is_playing != state.was_playing;
                let interpolated_now = match (state.anchor_pos, state.anchor_time) {
                    (Some(ap), Some(at)) if state.was_playing => ap + at.elapsed().as_secs_f64(),
                    (Some(ap), _) => ap,
                    _ => pos,
                };
                let jumped_or_seeked = (pos - interpolated_now).abs() > 1.5;

                if track_changed || play_state_changed || state.anchor_pos.is_none() || jumped_or_seeked
                {
                    let effective_pos = if !track_changed && !jumped_or_seeked && !is_playing {
                        interpolated_now
                    } else {
                        pos
                    };
                    state.last_raw_pos = Some(pos);
                    state.anchor_pos = Some(effective_pos);
                    state.anchor_time = Some(now);
                }

                (state.anchor_pos, state.anchor_time.unwrap_or(now))
            }
            None => (None, now),
        };
        state.was_playing = is_playing;

        Some(NowPlaying {
            title,
            artist,
            is_playing,
            cover_palette: state.palette,
            cover_art: state.art.clone(),
            position_secs,
            duration_secs,
            updated_at,
        })
    }

    fn read_timeline(
        session: &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
        is_playing: bool,
    ) -> (Option<f64>, Option<f64>) {
        let Ok(timeline) = session.GetTimelineProperties() else {
            return (None, None);
        };
        let mut pos = timeline
            .Position()
            .ok()
            .map(|t| (t.Duration as f64 / TICKS_PER_SEC).max(0.0));
        let dur = timeline
            .EndTime()
            .ok()
            .map(|t| (t.Duration as f64 / TICKS_PER_SEC).max(0.0));

        // Account for time elapsed since the player last updated GSMTC timeline.
        if is_playing
            && let (Some(p), Ok(last_updated)) = (pos, timeline.LastUpdatedTime())
            && last_updated.UniversalTime > 0
            && let Ok(now_unix) = SystemTime::now().duration_since(UNIX_EPOCH)
        {
            let now_ticks = (now_unix.as_secs() as i64 + FILETIME_UNIX_EPOCH_SECS)
                .saturating_mul(10_000_000)
                + i64::from(now_unix.subsec_nanos() / 100);
            let age_secs = (now_ticks - last_updated.UniversalTime) as f64 / TICKS_PER_SEC;
            if (0.0..3600.0).contains(&age_secs) {
                let adjusted = p + age_secs;
                pos = Some(match dur {
                    Some(d) if d > 0.0 => adjusted.min(d),
                    _ => adjusted,
                });
            }
        }

        (pos, dur)
    }

    type ThumbnailData = (Option<SystemPalette>, Option<Vec<(u8, u8, u8)>>);

    fn read_thumbnail_data(
        props: &GlobalSystemMediaTransportControlsSessionMediaProperties,
    ) -> ThumbnailData {
        let Some(thumb_ref) = props.Thumbnail().ok() else {
            return (None, None);
        };
        let Some(stream) = thumb_ref.OpenReadAsync().ok().and_then(|op| op.get().ok()) else {
            return (None, None);
        };
        let Ok(size) = stream.Size() else {
            return (None, None);
        };
        if size == 0 || size > MAX_THUMBNAIL_BYTES {
            return (None, None);
        }

        let Ok(reader) = DataReader::CreateDataReader(&stream) else {
            return (None, None);
        };
        let Some(loaded) = reader.LoadAsync(size as u32).ok().and_then(|op| op.get().ok()) else {
            return (None, None);
        };
        if loaded == 0 {
            return (None, None);
        }

        let mut bytes = vec![0u8; loaded as usize];
        if reader.ReadBytes(&mut bytes).is_err() {
            return (None, None);
        }
        (extract_cover_palette(&bytes), extract_cover_art(&bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{ImageBuffer, Rgb};

    use super::*;

    #[test]
    fn extracts_palette_from_png_cover() {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(16, 16, |x, _| {
                if x < 8 {
                    Rgb([240, 40, 120])
                } else {
                    Rgb([30, 180, 240])
                }
            });

        let mut png_bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .expect("encode png");

        let palette = extract_cover_palette(&png_bytes).expect("palette");
        assert!(palette.base.0 > 100 || palette.base.1 > 100 || palette.base.2 > 100);
        assert!(palette.light.0 >= palette.base.0);
    }
}
