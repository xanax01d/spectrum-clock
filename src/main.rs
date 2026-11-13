mod audio;
mod clock;
mod config;
mod fft;
mod lyrics;
mod now_playing;
mod particles;
mod theme;
mod ui;
mod weather;

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::audio::AudioCapture;
use crate::clock::TimerState;
use crate::config::{AppConfig, load_custom_themes};
use crate::fft::{AnalyzeParams, SpectrumAnalyzer};
use crate::lyrics::LyricsWatcher;
use crate::now_playing::{MediaCommand, NowPlayingWatcher};
use crate::particles::BackgroundEngine;
use crate::theme::{SystemPalette, Theme};
use crate::ui::{LyricsView, UiState, calc_num_bars};
use crate::weather::WeatherWatcher;

const TARGET_FRAME_TIME: Duration = Duration::from_millis(16);
const ZEN_IDLE_TIMEOUT: Duration = Duration::from_secs(4);

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        enable_raw_mode().context("Failed to enable terminal raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create ratatui terminal")?;
        terminal.hide_cursor().context("Failed to hide cursor")?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn main() -> Result<()> {
    let mut guard = TerminalGuard::new()?;

    let (capture, audio_status) = match AudioCapture::start_loopback() {
        Ok(cap) => {
            let prefix = if cfg!(target_os = "windows") {
                "WASAPI"
            } else {
                "Input"
            };
            let status = format!("{prefix}: {}", cap.device_name());
            (Some(cap), status)
        }
        Err(err) => (None, format!("Audio off ({err})")),
    };

    let now_playing_watcher = NowPlayingWatcher::start();
    let mut lyrics_watcher = LyricsWatcher::start();
    let mut bg_engine = BackgroundEngine::new(36);
    let sys_palette = SystemPalette::detect();
    let mut active_palette = sys_palette;

    let mut cfg = AppConfig::load();
    let weather_watcher = WeatherWatcher::start(cfg.weather_city.clone());
    let custom_themes = load_custom_themes();
    let mut custom_idx = cfg
        .custom_theme
        .as_deref()
        .and_then(|name| custom_themes.iter().position(|t| t.name == name));
    let mut timer = TimerState::default();
    let mut analyzer = SpectrumAnalyzer::new();
    let mut samples = Vec::with_capacity(4096);
    let mut stereo_samples = Vec::with_capacity(1024);
    let mut stereo_vu = (0.0_f32, 0.0_f32);
    let mut silence_secs = 0.0_f32;
    let mut paused = false;
    let mut show_help = false;
    let mut dismissed_alarm_minute = String::new();
    let start_time = Instant::now();
    let mut last_frame = start_time;
    let mut last_input = start_time;
    let mut last_party_switch = start_time;
    let mut last_party_track = String::new();

    loop {
        let now = Instant::now();
        let dt_secs = now.duration_since(last_frame).as_secs_f32().clamp(0.001, 0.1);
        last_frame = now;

        let term_size = guard.terminal.size()?;
        let num_bars = calc_num_bars(term_size.width, cfg.bar_density);

        if !paused {
            let params = AnalyzeParams {
                num_bars,
                freq_range: cfg.freq_range,
                gain: cfg.gain,
                auto_gain: cfg.auto_gain,
                dt_secs,
            };
            let raw_vu = if let Some(cap) = &capture {
                let levels = cap.read_samples(&mut samples, &mut stereo_samples);
                analyzer.process(&samples, cap.sample_rate(), params);
                levels
            } else {
                analyzer.process(&[], 48_000, params);
                (0.0, 0.0)
            };

            let vu_smooth = 1.0 - (-dt_secs * 18.0).exp();
            stereo_vu.0 += (raw_vu.0 - stereo_vu.0) * vu_smooth;
            stereo_vu.1 += (raw_vu.1 - stereo_vu.1) * vu_smooth;

            let max_bar = analyzer.bars().iter().copied().fold(0.0_f32, f32::max);
            if stereo_vu.0 < 0.005 && stereo_vu.1 < 0.005 && max_bar < 0.015 {
                silence_secs += dt_secs;
            } else {
                silence_secs = 0.0;
            }
        }

        let idle_asleep = cfg.idle_sleep && !paused && silence_secs >= 15.0;
        let bass_pulse = analyzer.bass_pulse();
        bg_engine.update(cfg.bg_effect, dt_secs, bass_pulse);
        timer.update(dt_secs);

        let current_track = now_playing_watcher.latest();
        let (lyric_window, lyrics_snapshot) = if let Some(track) = &current_track {
            lyrics_watcher.update_track(&track.artist, &track.title);
            if cfg.party_mode {
                let key = format!("{}\0{}", track.artist, track.title);
                if key != last_party_track {
                    if !last_party_track.is_empty() {
                        cfg.mode = cfg.mode.next();
                        cfg.bg_effect = cfg.bg_effect.next();
                        last_party_switch = now;
                    }
                    last_party_track = key;
                }
            }
            if cfg.lyrics_view == LyricsView::Full {
                let snap = lyrics_watcher.snapshot(
                    &track.artist,
                    &track.title,
                    track.current_progress().map(|(pos, _)| pos),
                );
                (None, snap)
            } else {
                let win = track
                    .current_progress()
                    .and_then(|(pos, _)| lyrics_watcher.active_window(&track.artist, &track.title, pos));
                (win, None)
            }
        } else {
            (None, None)
        };

        if cfg.party_mode && last_party_switch.elapsed() >= Duration::from_secs(45) {
            cfg.mode = cfg.mode.next();
            cfg.bg_effect = cfg.bg_effect.next();
            last_party_switch = now;
        }

        let timer_override = timer.formatted_time();

        let (effective_theme, custom_theme_name, target_palette) = if let Some(ci) = custom_idx
            && let Some(ct) = custom_themes.get(ci)
        {
            (Theme::System, Some(ct.name.as_str()), ct.palette)
        } else if cfg.theme == Theme::Cover {
            let pal = current_track
                .as_ref()
                .and_then(|t| t.cover_palette)
                .unwrap_or(sys_palette);
            (Theme::Cover, None, pal)
        } else {
            (cfg.theme, None, sys_palette)
        };

        let morph_t = 1.0 - (-dt_secs * 3.5).exp();
        active_palette = active_palette.lerp(target_palette, morph_t);
        active_palette.time_secs = start_time.elapsed().as_secs_f32();

        let now_hhmm = Local::now().format("%H:%M").to_string();
        let alarm_active = cfg
            .alarm_time
            .as_deref()
            .filter(|&t| t == now_hhmm && dismissed_alarm_minute != now_hhmm)
            .map(|_| cfg.alarm_message.as_str());

        let zen_hidden = cfg.zen_auto && last_input.elapsed() >= ZEN_IDLE_TIMEOUT;
        let weather_text = if cfg.weather {
            weather_watcher.latest()
        } else {
            None
        };

        let state = UiState {
            bars: analyzer.bars(),
            peaks: analyzer.peaks(),
            history: analyzer.history(),
            samples: &samples,
            stereo_samples: &stereo_samples,
            stereo_vu,
            bass_pulse,
            bpm: analyzer.bpm(),
            theme: effective_theme,
            custom_theme_name,
            mode: cfg.mode,
            freq_range: cfg.freq_range,
            bar_density: cfg.bar_density,
            clock_font: cfg.clock_font,
            sys_palette: active_palette,
            gain: cfg.gain,
            auto_gain: cfg.auto_gain,
            paused,
            show_seconds: cfg.show_seconds,
            show_help,
            zen_hidden,
            compact: cfg.compact,
            reflection: cfg.reflection,
            show_cover_art: cfg.show_cover_art,
            party_mode: cfg.party_mode,
            oled_drift: cfg.oled_drift,
            alarm_active,
            timer_override: timer_override.as_deref(),
            timer_label: timer.label(),
            bg_effect: cfg.bg_effect,
            bg_engine: &bg_engine,
            audio_status: &audio_status,
            now_playing: current_track.as_ref(),
            lyric_window: lyric_window.as_ref(),
            lyrics_snapshot: lyrics_snapshot.as_ref(),
            lyrics_view: cfg.lyrics_view,
            weather: weather_text.as_deref(),
            idle_asleep,
        };

        guard.terminal.draw(|frame| ui::render(frame, &state))?;

        let elapsed = now.elapsed();
        let poll_timeout = TARGET_FRAME_TIME.saturating_sub(elapsed);

        if event::poll(poll_timeout)?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            last_input = Instant::now();

            let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'));
            if ctrl_c {
                break;
            }

            let mut dirty = false;
            match key.code {
                KeyCode::Esc if show_help => {
                    show_help = false;
                }
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::Char('?') => {
                    show_help = !show_help;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    if let Some(ci) = custom_idx {
                        if ci + 1 < custom_themes.len() {
                            custom_idx = Some(ci + 1);
                            cfg.custom_theme = Some(custom_themes[ci + 1].name.clone());
                        } else {
                            custom_idx = None;
                            cfg.custom_theme = None;
                            cfg.theme = Theme::Cover;
                        }
                    } else if cfg.theme == Theme::System && !custom_themes.is_empty() {
                        custom_idx = Some(0);
                        cfg.custom_theme = Some(custom_themes[0].name.clone());
                    } else {
                        cfg.theme = cfg.theme.next();
                    }
                    dirty = true;
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    cfg.freq_range = cfg.freq_range.next();
                    dirty = true;
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    cfg.bar_density = cfg.bar_density.next();
                    dirty = true;
                }
                KeyCode::Char('o') | KeyCode::Char('O') => {
                    cfg.oled_drift = !cfg.oled_drift;
                    dirty = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    dismissed_alarm_minute = now_hhmm;
                }
                KeyCode::Char('m') | KeyCode::Char('M') => {
                    cfg.mode = cfg.mode.next();
                    dirty = true;
                }
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    cfg.clock_font = cfg.clock_font.next();
                    dirty = true;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    cfg.reflection = !cfg.reflection;
                    dirty = true;
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    cfg.show_cover_art = !cfg.show_cover_art;
                    dirty = true;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    cfg.party_mode = !cfg.party_mode;
                    last_party_switch = Instant::now();
                    dirty = true;
                }
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    timer.mode = timer.mode.next();
                }
                KeyCode::Enter => {
                    timer.reset_current();
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    cfg.auto_gain = !cfg.auto_gain;
                    dirty = true;
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    cfg.bg_effect = cfg.bg_effect.next();
                    dirty = true;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    cfg.compact = !cfg.compact;
                    dirty = true;
                }
                KeyCode::Char('z') | KeyCode::Char('Z') => {
                    cfg.zen_auto = !cfg.zen_auto;
                    dirty = true;
                }
                KeyCode::Char('k') | KeyCode::Char('K') => {
                    cfg.weather = !cfg.weather;
                    dirty = true;
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    cfg.idle_sleep = !cfg.idle_sleep;
                    dirty = true;
                }
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    cfg.lyrics_view = cfg.lyrics_view.next();
                    dirty = true;
                }
                KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Up => {
                    cfg.gain = (cfg.gain + 0.2).min(5.0);
                    dirty = true;
                }
                KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Down => {
                    cfg.gain = (cfg.gain - 0.2).max(0.2);
                    dirty = true;
                }
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    paused = !paused;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    cfg.show_seconds = !cfg.show_seconds;
                    dirty = true;
                }
                KeyCode::Char(' ') => {
                    now_playing_watcher.send_command(MediaCommand::TogglePlayPause);
                }
                KeyCode::Right => {
                    now_playing_watcher.send_command(MediaCommand::Next);
                }
                KeyCode::Left => {
                    now_playing_watcher.send_command(MediaCommand::Previous);
                }
                _ => {}
            }

            if dirty {
                cfg.save();
            }
        }
    }

    cfg.save();
    Ok(())
}
