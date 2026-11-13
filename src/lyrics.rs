use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;

/// A single timestamped lyric line from an `.lrc` file.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
}

/// Surrounding lyric window (`prev`, `active`, `next`) for karaoke display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricWindow {
    pub prev: Option<String>,
    pub active: String,
    pub next: Option<String>,
}

/// Full track lyrics snapshot for dedicated Lyrics Mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsSnapshot {
    pub lines: Vec<String>,
    pub active_idx: Option<usize>,
}

#[derive(Debug, Clone, Default)]
struct LyricsCache {
    track_key: String,
    lines: Vec<LyricLine>,
    plain_lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// Fetches synchronized `.lrc` lyrics in a background thread via `lrclib.net`.
pub struct LyricsWatcher {
    cache: Arc<Mutex<LyricsCache>>,
    tx: Option<Sender<(String, String)>>,
    last_requested: String,
}

impl LyricsWatcher {
    pub fn start() -> Self {
        let cache = Arc::new(Mutex::new(LyricsCache::default()));
        let (tx, rx) = mpsc::channel::<(String, String)>();
        let shared = Arc::clone(&cache);

        let spawned = std::thread::Builder::new()
            .name("lyrics-watcher".to_string())
            .spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(4))
                    .user_agent("spectrum-clock/0.1")
                    .build();

                let mut current_key = String::new();
                while let Ok((artist, title)) = rx.recv() {
                    // Drain queued requests so we only fetch the latest track.
                    let (mut latest_artist, mut latest_title) = (artist, title);
                    while let Ok((a, t)) = rx.try_recv() {
                        latest_artist = a;
                        latest_title = t;
                    }

                    let key = format!("{latest_artist}\0{latest_title}");
                    if key == current_key {
                        continue;
                    }
                    current_key = key.clone();

                    let (lines, plain_lines) =
                        fetch_lrc(&agent, &latest_artist, &latest_title).unwrap_or_default();
                    if let Ok(mut guard) = shared.lock() {
                        guard.track_key = key;
                        guard.lines = lines;
                        guard.plain_lines = plain_lines;
                    }
                }
            })
            .is_ok();

        Self {
            cache,
            tx: if spawned { Some(tx) } else { None },
            last_requested: String::new(),
        }
    }

    /// Requests lyrics if the track changed since the last request.
    pub fn update_track(&mut self, artist: &str, title: &str) {
        if title.is_empty() {
            return;
        }
        let key = format!("{artist}\0{title}");
        if key == self.last_requested {
            return;
        }
        self.last_requested = key;
        if let Some(tx) = &self.tx {
            let _ = tx.send((artist.to_string(), title.to_string()));
        }
    }

    /// Returns the active lyric window (`prev`, `active`, `next`) for the given track and position.
    pub fn active_window(&self, artist: &str, title: &str, pos_secs: f64) -> Option<LyricWindow> {
        let key = format!("{artist}\0{title}");
        let guard = self.cache.try_lock().ok()?;
        if guard.track_key != key || guard.lines.is_empty() {
            return None;
        }
        find_lyric_window(&guard.lines, pos_secs)
    }

    /// Returns all lyrics lines and the currently active line index (if synced).
    pub fn snapshot(
        &self,
        artist: &str,
        title: &str,
        pos_secs: Option<f64>,
    ) -> Option<LyricsSnapshot> {
        let key = format!("{artist}\0{title}");
        let guard = self.cache.try_lock().ok()?;
        if guard.track_key != key {
            return None;
        }
        if !guard.lines.is_empty() {
            let active_idx = pos_secs.and_then(|pos| {
                let idx = guard.lines.partition_point(|l| l.time_secs <= pos + 0.15);
                idx.checked_sub(1)
            });
            return Some(LyricsSnapshot {
                lines: guard.lines.iter().map(|l| l.text.clone()).collect(),
                active_idx,
            });
        }
        if !guard.plain_lines.is_empty() {
            return Some(LyricsSnapshot {
                lines: guard.plain_lines.clone(),
                active_idx: None,
            });
        }
        None
    }
}

fn fetch_lrc(
    agent: &ureq::Agent,
    artist: &str,
    title: &str,
) -> Option<(Vec<LyricLine>, Vec<String>)> {
    let resp = agent
        .get("https://lrclib.net/api/get")
        .query("artist_name", artist)
        .query("track_name", title)
        .call()
        .ok()?;

    let body: LrcLibResponse = serde_json::from_reader(resp.into_reader()).ok()?;
    let synced = body
        .synced_lyrics
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default();
    let plain: Vec<String> = body
        .plain_lyrics
        .as_deref()
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if synced.is_empty() && plain.is_empty() {
        None
    } else {
        Some((synced, plain))
    }
}

/// Parses `[mm:ss.xx]` LRC lines into sorted `LyricLine` entries.
pub fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in lrc.lines() {
        let trimmed = raw.trim();
        let Some(rest) = trimmed.strip_prefix('[') else {
            continue;
        };
        let Some((ts, text)) = rest.split_once(']') else {
            continue;
        };
        let Some(time_secs) = parse_lrc_timestamp(ts) else {
            continue;
        };
        let clean_text = text.trim();
        if !clean_text.is_empty() {
            lines.push(LyricLine {
                time_secs,
                text: clean_text.to_string(),
            });
        }
    }
    lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
    lines
}

fn parse_lrc_timestamp(ts: &str) -> Option<f64> {
    let (mins_str, secs_str) = ts.split_once(':')?;
    let mins: f64 = mins_str.trim().parse().ok()?;
    let secs: f64 = secs_str.trim().parse().ok()?;
    if mins < 0.0 || secs < 0.0 {
        return None;
    }
    Some(mins * 60.0 + secs)
}

/// Finds the active lyric line and its immediate neighbors for `pos_secs`.
pub fn find_lyric_window(lines: &[LyricLine], pos_secs: f64) -> Option<LyricWindow> {
    let target = pos_secs + 0.15;
    let idx = lines.partition_point(|l| l.time_secs <= target);
    if idx == 0 {
        return None;
    }
    let active_idx = idx - 1;
    Some(LyricWindow {
        prev: active_idx
            .checked_sub(1)
            .and_then(|i| lines.get(i))
            .map(|l| l.text.clone()),
        active: lines[active_idx].text.clone(),
        next: lines.get(active_idx + 1).map(|l| l.text.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_and_finds_active_line() {
        let sample = "\
[00:05.50] First line
[00:12.00] Second line
[01:02.25] Third line
";
        let lines = parse_lrc(sample);
        assert_eq!(lines.len(), 3);
        assert_eq!(find_lyric_window(&lines, 2.0), None);
        let w1 = find_lyric_window(&lines, 5.5).expect("line 1");
        assert_eq!(w1.prev, None);
        assert_eq!(w1.active, "First line");
        assert_eq!(w1.next.as_deref(), Some("Second line"));

        let w2 = find_lyric_window(&lines, 15.0).expect("line 2");
        assert_eq!(w2.prev.as_deref(), Some("First line"));
        assert_eq!(w2.active, "Second line");
        assert_eq!(w2.next.as_deref(), Some("Third line"));
    }
}
