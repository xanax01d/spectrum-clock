use std::collections::VecDeque;

use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Widget};

use crate::clock::{ClockFont, render_big_time};
use crate::fft::FreqRange;
use crate::lyrics::{LyricWindow, LyricsSnapshot};
use crate::now_playing::NowPlaying;
use crate::particles::{BackgroundEngine, BgEffect};
use crate::theme::{SystemPalette, Theme, brighten_color};

const BLOCK_CHARS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
const TOP_BLOCK_CHARS: [char; 9] = [' ', '▔', '▀', '▀', '▀', '█', '█', '█', '█'];

/// Lyrics display mode (`1L`, `3L`, or full-screen `Full`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LyricsView {
    OneLine,
    #[default]
    ThreeLines,
    Full,
}

impl LyricsView {
    pub fn next(self) -> Self {
        match self {
            Self::OneLine => Self::ThreeLines,
            Self::ThreeLines => Self::Full,
            Self::Full => Self::OneLine,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::OneLine => "1line",
            Self::ThreeLines => "3lines",
            Self::Full => "full",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "1line" | "1" | "one" => Some(Self::OneLine),
            "3lines" | "3" | "three" => Some(Self::ThreeLines),
            "full" => Some(Self::Full),
            _ => None,
        }
    }
}

/// Bar width / spacing density preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarDensity {
    #[default]
    Auto,
    Thin,
    Wide,
}

impl BarDensity {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Thin,
            Self::Thin => Self::Wide,
            Self::Wide => Self::Auto,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Thin => "Thin",
            Self::Wide => "Wide",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "thin" => Some(Self::Thin),
            "wide" => Some(Self::Wide),
            _ => None,
        }
    }

    fn width_and_gap(self, area_width: u16) -> (u16, u16) {
        match self {
            Self::Auto => {
                if area_width >= 56 {
                    (2, 1)
                } else {
                    (1, 1)
                }
            }
            Self::Thin => (1, 1),
            Self::Wide => (3, 1),
        }
    }
}

/// Visualization mode for the audio area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrumMode {
    #[default]
    Bars,
    Mirror,
    Waveform,
    Waterfall,
    Circle,
    Horizon,
    Lissajous,
}

impl SpectrumMode {
    pub fn next(self) -> Self {
        match self {
            Self::Bars => Self::Mirror,
            Self::Mirror => Self::Waveform,
            Self::Waveform => Self::Waterfall,
            Self::Waterfall => Self::Circle,
            Self::Circle => Self::Horizon,
            Self::Horizon => Self::Lissajous,
            Self::Lissajous => Self::Bars,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bars => "Bars",
            Self::Mirror => "Mirror",
            Self::Waveform => "Waveform",
            Self::Waterfall => "Waterfall",
            Self::Circle => "Circle",
            Self::Horizon => "Horizon",
            Self::Lissajous => "Lissajous",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bars" => Some(Self::Bars),
            "mirror" => Some(Self::Mirror),
            "waveform" => Some(Self::Waveform),
            "waterfall" => Some(Self::Waterfall),
            "circle" => Some(Self::Circle),
            "horizon" => Some(Self::Horizon),
            "lissajous" => Some(Self::Lissajous),
            _ => None,
        }
    }
}

/// Snapshot of application state needed to render a single frame.
pub struct UiState<'a> {
    pub bars: &'a [f32],
    pub peaks: &'a [f32],
    pub history: &'a VecDeque<Vec<f32>>,
    pub samples: &'a [f32],
    pub stereo_samples: &'a [(f32, f32)],
    pub stereo_vu: (f32, f32),
    pub bass_pulse: f32,
    pub bpm: Option<u16>,
    pub theme: Theme,
    pub custom_theme_name: Option<&'a str>,
    pub mode: SpectrumMode,
    pub freq_range: FreqRange,
    pub bar_density: BarDensity,
    pub clock_font: ClockFont,
    pub sys_palette: SystemPalette,
    pub gain: f32,
    pub auto_gain: bool,
    pub paused: bool,
    pub show_seconds: bool,
    pub show_help: bool,
    pub zen_hidden: bool,
    pub compact: bool,
    pub reflection: bool,
    pub show_cover_art: bool,
    pub party_mode: bool,
    pub oled_drift: bool,
    pub alarm_active: Option<&'a str>,
    pub timer_override: Option<&'a str>,
    pub timer_label: Option<&'a str>,
    pub bg_effect: BgEffect,
    pub bg_engine: &'a BackgroundEngine,
    pub audio_status: &'a str,
    pub now_playing: Option<&'a NowPlaying>,
    pub lyric_window: Option<&'a LyricWindow>,
    pub lyrics_snapshot: Option<&'a LyricsSnapshot>,
    pub lyrics_view: LyricsView,
    pub weather: Option<&'a str>,
    pub idle_asleep: bool,
}

/// Computes how many spectrum bars fit nicely into the given terminal width.
pub fn calc_num_bars(terminal_width: u16, density: BarDensity) -> usize {
    let usable = terminal_width.saturating_sub(4) as usize;
    if usable == 0 {
        return 0;
    }
    let (w, g) = density.width_and_gap(terminal_width);
    let stride = usize::from(w + g);
    (usable + usize::from(g)) / stride
}

/// Renders the background particles, clock, now-playing, lyrics, spectrum, and footer.
pub fn render(frame: &mut Frame, state: &UiState<'_>) {
    let area = frame.area();
    if area.width < 10 || area.height < 6 {
        let msg = Paragraph::new("Too small").alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    state.bg_engine.render(
        state.bg_effect,
        area,
        frame.buffer_mut(),
        state.theme,
        state.sys_palette,
        state.bass_pulse,
    );

    let is_compact = state.compact || area.height < 16 || area.width < 46;
    if state.lyrics_view == LyricsView::Full {
        render_lyrics_mode(frame, area, state);
    } else if is_compact {
        render_compact(frame, area, state);
    } else {
        let has_track = state.now_playing.is_some();
        let has_progress = state
            .now_playing
            .and_then(NowPlaying::current_progress)
            .is_some();
        let lyric_rows = match state.lyric_window {
            Some(lw) if state.lyrics_view == LyricsView::ThreeLines && area.height >= 24 => {
                1 + u16::from(lw.prev.is_some()) + u16::from(lw.next.is_some())
            }
            Some(_) => 1,
            None => 0,
        };

        let extra_track_rows = u16::from(has_track)
            + u16::from(has_progress)
            + lyric_rows
            + u16::from(state.alarm_active.is_some());
        let clock_height = 8 + extra_track_rows;
        let footer_height = u16::from(!state.zen_hidden);

        let top_pad = if state.oled_drift && (state.sys_palette.time_secs * 0.2).sin() > 0.0 {
            2
        } else {
            1
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(top_pad),       // Top padding (drifts for OLED protection)
                Constraint::Length(clock_height),  // Clock + VU + Date + Now Playing + Lyrics
                Constraint::Min(4),                // Spectrum / Waveform
                Constraint::Length(footer_height), // Status footer (hidden in Zen mode)
            ])
            .split(area);

        let drift_x: i16 = if state.oled_drift {
            ((state.sys_palette.time_secs * 0.14).sin() * 2.0).round() as i16
        } else {
            0
        };
        let clock_area = Rect {
            x: (i16::try_from(chunks[1].x).unwrap_or(0) + drift_x).max(0) as u16,
            y: chunks[1].y,
            width: chunks[1].width.saturating_sub(drift_x.unsigned_abs()),
            height: chunks[1].height,
        };

        render_clock_full(frame, clock_area, state);
        if !state.idle_asleep {
            render_spectrum_area(frame, chunks[2], state);
        }

        if !state.zen_hidden {
            render_footer(frame, chunks[3], state);
        }
    }

    if state.show_help {
        render_help_overlay(frame, area, state);
    }
}

fn render_compact(frame: &mut Frame, area: Rect, state: &UiState<'_>) {
    let now = Local::now();
    let time_fmt = if state.show_seconds { "%H:%M:%S" } else { "%H:%M" };
    let time_str = now.format(time_fmt).to_string();
    let accent = state.theme.accent_color(state.sys_palette);
    let clock_color = brighten_color(
        state.theme.clock_row_color(0, 5, state.sys_palette),
        (state.bass_pulse * 0.45).clamp(0.0, 0.45),
    );

    let active_lyric = state.lyric_window.map(|w| w.active.as_str());
    let show_lyric = active_lyric.is_some() && area.height >= 7;
    let footer_h = u16::from(!state.zen_hidden && area.height >= 8);
    let header_h = 1 + u16::from(show_lyric);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(3),
            Constraint::Length(footer_h),
        ])
        .split(area);

    let mut header_spans = vec![Span::styled(
        format!(" {time_str}"),
        Style::default().fg(clock_color).add_modifier(Modifier::BOLD),
    )];

    if let Some(track) = state.now_playing {
        let icon = if track.is_playing { "▶" } else { "⏸" };
        let used = time_str.len() + 14;
        let max_track = (area.width as usize).saturating_sub(used);
        if max_track >= 6 {
            header_spans.push(Span::styled(format!("  {icon} "), Style::default().fg(accent)));
            header_spans.push(Span::styled(
                truncate_chars(&track.display_text(), max_track),
                Style::default().fg(Color::White),
            ));
        }
    }

    let mut header_lines = vec![Line::from(header_spans)];
    if show_lyric && let Some(lyric) = active_lyric {
        let trimmed = truncate_chars(lyric, area.width.saturating_sub(4) as usize);
        header_lines.push(Line::from(Span::styled(
            format!(" ♪ {trimmed}"),
            Style::default().fg(accent).add_modifier(Modifier::ITALIC),
        )));
    }

    frame.render_widget(Paragraph::new(header_lines), chunks[0]);
    if !state.idle_asleep {
        render_spectrum_area(frame, chunks[1], state);
    }

    if footer_h > 0 {
        render_footer(frame, chunks[2], state);
    }
}

fn render_lyrics_mode(frame: &mut Frame, area: Rect, state: &UiState<'_>) {
    let now = Local::now();
    let time_fmt = if state.show_seconds { "%H:%M:%S" } else { "%H:%M" };
    let time_str = state
        .timer_override
        .map(str::to_string)
        .unwrap_or_else(|| now.format(time_fmt).to_string());
    let mut top_meta = format!("{time_str}  •  {}", now.format("%Y-%m-%d"));
    if let Some(w) = state.weather {
        top_meta.push_str(&format!("  •  {w}"));
    }

    let accent = state.theme.accent_color(state.sys_palette);
    let mut header_lines = vec![Line::from(Span::styled(top_meta, Style::default().fg(accent)))];

    let max_len = area.width.saturating_sub(6) as usize;
    if let Some(track) = state.now_playing {
        let icon = if track.is_playing { "▶" } else { "⏸" };
        header_lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(accent)),
            Span::styled(
                truncate_chars(&track.display_text(), max_len),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some((pos, dur)) = track.current_progress() {
            header_lines.push(build_progress_line(pos, dur, area.width, accent));
        }
    } else {
        header_lines.push(Line::from(Span::styled(
            "No track playing",
            Style::default().fg(Color::DarkGray),
        )));
    }
    header_lines.push(Line::from(""));

    let header_h = header_lines.len() as u16;
    let footer_h = u16::from(!state.zen_hidden);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(header_h),
            Constraint::Min(3),
            Constraint::Length(footer_h),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(header_lines).alignment(Alignment::Center),
        chunks[1],
    );

    let body = chunks[2];
    let visible_h = body.height as usize;
    let body_lines = if let Some(snap) = state.lyrics_snapshot
        && !snap.lines.is_empty()
    {
        let focus = snap.active_idx.unwrap_or(0);
        let center_row = visible_h / 2;
        let start = focus.saturating_sub(center_row);
        let end = (start + visible_h).min(snap.lines.len());
        let pad_top = if snap.active_idx.is_some() {
            center_row.saturating_sub(focus.saturating_sub(start))
        } else {
            0
        };

        let mut out: Vec<Line<'_>> = (0..pad_top).map(|_| Line::from("")).collect();
        for (offset, text) in snap.lines[start..end].iter().enumerate() {
            let abs_idx = start + offset;
            let trimmed = truncate_chars(text, max_len);
            if Some(abs_idx) == snap.active_idx {
                out.push(Line::from(vec![
                    Span::styled("♪ ", Style::default().fg(accent)),
                    Span::styled(
                        trimmed,
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else if snap.active_idx.is_some_and(|active| abs_idx < active) {
                out.push(Line::from(Span::styled(
                    trimmed,
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                out.push(Line::from(Span::styled(
                    trimmed,
                    Style::default().fg(Color::Gray),
                )));
            }
        }
        out
    } else {
        let msg = if state.now_playing.is_some() {
            "No lyrics available for this track"
        } else {
            "Play a track to view lyrics"
        };
        let pad_top = visible_h / 2;
        let mut out: Vec<Line<'_>> = (0..pad_top).map(|_| Line::from("")).collect();
        out.push(Line::from(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        )));
        out
    };

    frame.render_widget(
        Paragraph::new(body_lines).alignment(Alignment::Center),
        body,
    );

    if footer_h > 0 {
        render_footer(frame, chunks[3], state);
    }
}

fn render_spectrum_area(frame: &mut Frame, rect: Rect, state: &UiState<'_>) {
    let spectrum_area = Rect {
        x: rect.x.saturating_add(1),
        y: rect.y,
        width: rect.width.saturating_sub(2),
        height: rect.height,
    };

    match state.mode {
        SpectrumMode::Bars => frame.render_widget(
            BarsWidget {
                bars: state.bars,
                peaks: state.peaks,
                reflection: state.reflection,
                bar_density: state.bar_density,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Mirror => frame.render_widget(
            MirrorWidget {
                bars: state.bars,
                bar_density: state.bar_density,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Waveform => frame.render_widget(
            WaveformWidget {
                samples: state.samples,
                gain: state.gain,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Waterfall => frame.render_widget(
            WaterfallWidget {
                history: state.history,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Circle => frame.render_widget(
            CircleWidget {
                bars: state.bars,
                bass_pulse: state.bass_pulse,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Horizon => frame.render_widget(
            HorizonWidget {
                bars: state.bars,
                bass_pulse: state.bass_pulse,
                bar_density: state.bar_density,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
        SpectrumMode::Lissajous => frame.render_widget(
            LissajousWidget {
                stereo_samples: state.stereo_samples,
                gain: state.gain,
                theme: state.theme,
                sys_palette: state.sys_palette,
            },
            spectrum_area,
        ),
    }
}

fn render_clock_full(frame: &mut Frame, area: Rect, state: &UiState<'_>) {
    let now = Local::now();
    let time_fmt = if state.show_seconds { "%H:%M:%S" } else { "%H:%M" };
    let time_str = state
        .timer_override
        .map(str::to_string)
        .unwrap_or_else(|| now.format(time_fmt).to_string());
    let base_sub = match state.timer_label {
        Some(lbl) => lbl.to_string(),
        None => now.format("%Y-%m-%d  •  %A").to_string(),
    };
    let mut date_str = match state.bpm {
        Some(bpm) => format!("{base_sub}  •  ~{bpm} BPM"),
        None => base_sub,
    };
    if let Some(w) = state.weather {
        date_str.push_str(&format!("  •  {w}"));
    }

    let accent_color = state.theme.accent_color(state.sys_palette);
    let date_style = Style::default().fg(accent_color);
    let pulse = (state.bass_pulse * 0.45).clamp(0.0, 0.45);

    let mut lines = Vec::new();
    let display_time = if state.show_seconds && area.width < 54 {
        now.format("%H:%M").to_string()
    } else {
        time_str
    };
    let rows = render_big_time(&display_time, state.clock_font);
    let total = rows.len();
    for (idx, row) in rows.into_iter().enumerate() {
        let base = state.theme.clock_row_color(idx, total, state.sys_palette);
        let row_color = brighten_color(base, pulse);
        lines.push(Line::from(Span::styled(
            row,
            Style::default().fg(row_color).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(date_str, date_style)));

    if let Some(track) = state.now_playing {
        let icon = if track.is_playing { "▶" } else { "⏸" };
        let max_len = area.width.saturating_sub(6) as usize;
        let text = truncate_chars(&track.display_text(), max_len);
        lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(accent_color)),
            Span::styled(text, Style::default().fg(Color::White)),
        ]));

        if let Some((pos, dur)) = track.current_progress() {
            lines.push(build_progress_line(pos, dur, area.width, accent_color));
        }
    }

    if let Some(lw) = state.lyric_window {
        let max_len = area.width.saturating_sub(6) as usize;
        let needed_karaoke = 1
            + usize::from(lw.prev.is_some())
            + usize::from(lw.next.is_some())
            + usize::from(state.alarm_active.is_some());
        let show_context = state.lyrics_view == LyricsView::ThreeLines
            && lines.len() + needed_karaoke <= area.height as usize;

        if show_context && let Some(prev) = &lw.prev {
            lines.push(Line::from(Span::styled(
                truncate_chars(prev, max_len),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }

        let text = truncate_chars(&lw.active, max_len);
        lines.push(Line::from(vec![
            Span::styled("♪ ", Style::default().fg(accent_color)),
            Span::styled(
                text,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::ITALIC | Modifier::BOLD),
            ),
        ]));

        if show_context && let Some(next) = &lw.next {
            lines.push(Line::from(Span::styled(
                truncate_chars(next, max_len),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }
    }

    if let Some(alarm_msg) = state.alarm_active {
        let blink = (state.sys_palette.time_secs * 3.0).sin() > 0.0;
        let alarm_color = if blink { Color::Yellow } else { accent_color };
        lines.push(Line::from(Span::styled(
            format!("⏰ {alarm_msg}  [N] Dismiss"),
            Style::default().fg(alarm_color).add_modifier(Modifier::BOLD),
        )));
    }

    let widget = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(widget, area);

    // Draw vertical L/R Stereo VU meters flanking the 5-row clock when wide enough.
    if area.width >= 64 && area.height >= 5 {
        let vu_area_l = Rect {
            x: area.x + 3,
            y: area.y,
            width: 2,
            height: 5,
        };
        let vu_area_r = Rect {
            x: area.right().saturating_sub(5),
            y: area.y,
            width: 2,
            height: 5,
        };
        render_vu_column(frame.buffer_mut(), vu_area_l, state.stereo_vu.0, 'L', state);
        render_vu_column(frame.buffer_mut(), vu_area_r, state.stereo_vu.1, 'R', state);
    }

    // Draw 12x6 pixel album cover art when terminal is wide enough.
    if state.show_cover_art
        && area.width >= 86
        && area.height >= 6
        && let Some(track) = state.now_playing
        && let Some(art) = &track.cover_art
    {
        let art_area = Rect {
            x: area.x + 6,
            y: area.y,
            width: 12,
            height: 6,
        };
        render_cover_art_block(frame.buffer_mut(), art_area, art);
    }
}

fn render_cover_art_block(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    pixels: &[(u8, u8, u8)],
) {
    if pixels.len() < 144 {
        return;
    }
    for cy in 0..6u16 {
        let top_row = (cy as usize) * 2;
        let bot_row = top_row + 1;
        for cx in 0..12u16 {
            let (tr, tg, tb) = pixels[top_row * 12 + (cx as usize)];
            let (br, bg, bb) = pixels[bot_row * 12 + (cx as usize)];
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                cell.set_char('▀')
                    .set_fg(Color::Rgb(tr, tg, tb))
                    .set_bg(Color::Rgb(br, bg, bb));
            }
        }
    }
}

fn render_vu_column(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    level: f32,
    label: char,
    state: &UiState<'_>,
) {
    let bar_h = area.height.saturating_sub(1);
    if bar_h == 0 {
        return;
    }
    let filled = (level.clamp(0.0, 1.0) * f32::from(bar_h) * 8.0).round() as usize;
    for row in 0..bar_h {
        let y = area.y + bar_h - 1 - row;
        let cell_eighths = filled.saturating_sub((row as usize) * 8).min(8);
        let ch = if cell_eighths == 0 {
            '·'
        } else {
            BLOCK_CHARS[cell_eighths]
        };
        let norm_y = (f32::from(row) + 0.5) / f32::from(bar_h);
        let color = if cell_eighths == 0 {
            Color::DarkGray
        } else {
            state
                .theme
                .bar_color(norm_y, 0.5, level, state.sys_palette)
        };
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_char(ch).set_fg(color);
        }
    }
    if let Some(cell) = buf.cell_mut((area.x, area.y + bar_h)) {
        cell.set_char(label).set_fg(Color::DarkGray);
    }
}

fn build_progress_line(pos_secs: f64, dur_secs: f64, width: u16, accent: Color) -> Line<'static> {
    let ratio = (pos_secs / dur_secs).clamp(0.0, 1.0);
    let left_ts = format_mm_ss(pos_secs);
    let right_ts = format_mm_ss(dur_secs);
    let bar_len = (width as usize).saturating_sub(left_ts.len() + right_ts.len() + 6).clamp(10, 34);

    let filled = ((ratio * bar_len as f64).round() as usize).min(bar_len);
    let mut done = String::new();
    let mut rest = String::new();
    for i in 0..bar_len {
        if i < filled {
            done.push('━');
        } else if i == filled {
            done.push('●');
        } else {
            rest.push('─');
        }
    }

    Line::from(vec![
        Span::styled(format!("{left_ts} "), Style::default().fg(Color::DarkGray)),
        Span::styled(done, Style::default().fg(accent)),
        Span::styled(rest, Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" {right_ts}"), Style::default().fg(Color::DarkGray)),
    ])
}

fn format_mm_ss(secs: f64) -> String {
    let total = secs.max(0.0).floor() as u64;
    let mins = total / 60;
    let rem = total % 60;
    format!("{mins:02}:{rem:02}")
}

fn render_footer(frame: &mut Frame, area: Rect, state: &UiState<'_>) {
    let theme_label = state.custom_theme_name.unwrap_or_else(|| state.theme.name());
    let mut parts = vec![theme_label.to_string()];

    if !state.idle_asleep {
        parts.push(format!("{} ({})", state.mode.name(), state.freq_range.name()));
    }
    if state.bg_effect != BgEffect::Off {
        parts.push(format!("Bg:{}", state.bg_effect.name()));
    }
    if (state.gain - 1.0).abs() > 0.05 {
        parts.push(format!("{:.1}x", state.gain));
    }

    let active_flags = [
        (state.lyrics_view == LyricsView::OneLine, "[LYRICS:1L]"),
        (state.lyrics_view == LyricsView::Full, "[LYRICS]"),
        (state.auto_gain, "[AGC]"),
        (state.party_mode, "[PARTY]"),
        (state.reflection, "[REFL]"),
        (state.oled_drift, "[OLED]"),
        (state.compact, "[COMPACT]"),
        (state.idle_asleep, "[SLEEP]"),
        (state.paused, "[PAUSED]"),
    ];
    let tags: Vec<&str> = active_flags
        .into_iter()
        .filter_map(|(enabled, label)| enabled.then_some(label))
        .collect();
    if !tags.is_empty() {
        parts.push(tags.join(" "));
    }
    if !state.audio_status.is_empty() {
        parts.push(state.audio_status.to_string());
    }

    let text = truncate_chars(&format!(" {}", parts.join(" | ")), area.width as usize);
    let accent = state.theme.accent_color(state.sys_palette);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::default().fg(accent)))),
        area,
    );
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else if max_chars == 1 {
        "…".to_string()
    } else {
        let mut out: String = s.chars().take(max_chars - 1).collect();
        out.push('…');
        out
    }
}

struct BarsWidget<'a> {
    bars: &'a [f32],
    peaks: &'a [f32],
    reflection: bool,
    bar_density: BarDensity,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for BarsWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width == 0 || area.height == 0 || self.bars.is_empty() {
            return;
        }

        let bar_count = self.bars.len();
        let (bar_width, gap) = self.bar_density.width_and_gap(area.width);
        let stride = bar_width + gap;

        let total_used = (bar_count as u16)
            .saturating_mul(stride)
            .saturating_sub(gap);
        let start_x = area.x + area.width.saturating_sub(total_used) / 2;

        let reflect_rows: u16 = if self.reflection && area.height >= 7 {
            2
        } else {
            0
        };
        let main_height = area.height.saturating_sub(reflect_rows);
        let height_cells = main_height as f32;
        let peak_color = self.theme.peak_color();

        for (idx, &val) in self.bars.iter().enumerate() {
            let x0 = start_x + (idx as u16) * stride;
            if x0 >= area.right() {
                break;
            }

            let norm_x = if bar_count > 1 {
                idx as f32 / (bar_count - 1) as f32
            } else {
                0.5
            };

            let filled_cells = (val.clamp(0.0, 1.0) * height_cells).clamp(0.0, height_cells);
            let full_rows = filled_cells.floor() as u16;
            let frac_eighths = ((filled_cells - full_rows as f32) * 8.0).round() as usize;

            for row_from_bottom in 0..main_height {
                let y = area.y + main_height - 1 - row_from_bottom;
                let norm_y = (row_from_bottom as f32 + 0.5) / height_cells;
                let color = self
                    .theme
                    .bar_color(norm_y, norm_x, val, self.sys_palette);

                let ch = if row_from_bottom < full_rows {
                    BLOCK_CHARS[8]
                } else if row_from_bottom == full_rows && frac_eighths > 0 {
                    BLOCK_CHARS[frac_eighths.min(8)]
                } else {
                    continue;
                };

                for dx in 0..bar_width {
                    let x = x0 + dx;
                    if x < area.right()
                        && let Some(cell) = buf.cell_mut((x, y))
                    {
                        cell.set_char(ch).set_fg(color);
                    }
                }
            }

            if let Some(&peak) = self.peaks.get(idx)
                && peak > 0.02
            {
                let peak_row = ((peak.clamp(0.0, 1.0) * (height_cells - 1.0)).round()) as u16;
                let y = area.y + main_height - 1 - peak_row.min(main_height - 1);
                for dx in 0..bar_width {
                    let x = x0 + dx;
                    if x < area.right()
                        && let Some(cell) = buf.cell_mut((x, y))
                        && cell.symbol() != "█"
                    {
                        cell.set_char('─').set_fg(peak_color);
                    }
                }
            }

            // Draw water-style dimmed reflection below the horizon.
            if reflect_rows > 0 && val > 0.08 {
                let refl_cells = (val * f32::from(reflect_rows)).clamp(0.0, f32::from(reflect_rows));
                for r in 0..reflect_rows {
                    if refl_cells <= f32::from(r) {
                        break;
                    }
                    let y = area.y + main_height + r;
                    let fade = if r == 0 { 0.34 } else { 0.16 };
                    let base = self.theme.bar_color(0.1, norm_x, val, self.sys_palette);
                    let dim = dim_rgb(base, fade);
                    let ch = if r == 0 && refl_cells >= 0.8 {
                        '▀'
                    } else {
                        '░'
                    };
                    for dx in 0..bar_width {
                        let x = x0 + dx;
                        if x < area.right()
                            && let Some(cell) = buf.cell_mut((x, y))
                        {
                            cell.set_char(ch).set_fg(dim);
                        }
                    }
                }
            }
        }
    }
}

fn dim_rgb(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (f32::from(r) * factor).round().clamp(0.0, 255.0) as u8,
            (f32::from(g) * factor).round().clamp(0.0, 255.0) as u8,
            (f32::from(b) * factor).round().clamp(0.0, 255.0) as u8,
        ),
        other => other,
    }
}

struct MirrorWidget<'a> {
    bars: &'a [f32],
    bar_density: BarDensity,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for MirrorWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width == 0 || area.height < 2 || self.bars.is_empty() {
            return;
        }

        let bar_count = self.bars.len();
        let (bar_width, gap) = self.bar_density.width_and_gap(area.width);
        let stride = bar_width + gap;
        let total_used = (bar_count as u16)
            .saturating_mul(stride)
            .saturating_sub(gap);
        let start_x = area.x + area.width.saturating_sub(total_used) / 2;

        let half_h = (area.height / 2) as f32;
        let mid_top = area.y + area.height / 2 - 1;
        let mid_bot = area.y + area.height / 2;

        for (idx, &val) in self.bars.iter().enumerate() {
            let x0 = start_x + (idx as u16) * stride;
            if x0 >= area.right() {
                break;
            }
            let norm_x = if bar_count > 1 {
                idx as f32 / (bar_count - 1) as f32
            } else {
                0.5
            };

            let filled = (val.clamp(0.0, 1.0) * half_h).clamp(0.0, half_h);
            let full_rows = filled.floor() as u16;
            let frac_eighths = ((filled - full_rows as f32) * 8.0).round() as usize;

            for step in 0..(area.height / 2) {
                let norm_y = (step as f32 + 0.5) / half_h.max(1.0);
                let color = self
                    .theme
                    .bar_color(norm_y, norm_x, val, self.sys_palette);

                let (top_ch, bot_ch) = if step < full_rows {
                    (BLOCK_CHARS[8], BLOCK_CHARS[8])
                } else if step == full_rows && frac_eighths > 0 {
                    (
                        BLOCK_CHARS[frac_eighths.min(8)],
                        TOP_BLOCK_CHARS[frac_eighths.min(8)],
                    )
                } else {
                    continue;
                };

                let y_up = mid_top.saturating_sub(step);
                let y_dn = mid_bot + step;

                for dx in 0..bar_width {
                    let x = x0 + dx;
                    if x < area.right() {
                        if y_up >= area.y
                            && let Some(cell) = buf.cell_mut((x, y_up))
                        {
                            cell.set_char(top_ch).set_fg(color);
                        }
                        if y_dn < area.bottom()
                            && let Some(cell) = buf.cell_mut((x, y_dn))
                        {
                            cell.set_char(bot_ch).set_fg(color);
                        }
                    }
                }
            }
        }
    }
}

struct WaveformWidget<'a> {
    samples: &'a [f32],
    gain: f32,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for WaveformWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let px_w = (area.width as usize) * 2;
        let px_h = (area.height as usize) * 4;
        if px_w == 0 || px_h == 0 {
            return;
        }

        let mut grid = vec![0u8; (area.width as usize) * (area.height as usize)];
        let center_y = (px_h as f32) * 0.5;
        let amp_scale = center_y * 0.9 * self.gain;

        let usable_samples = self.samples.len().min(2048);
        let slice = &self.samples[self.samples.len() - usable_samples..];

        let mut prev_y: Option<usize> = None;
        for px_x in 0..px_w {
            let sample = if slice.is_empty() {
                0.0
            } else {
                let s_idx = (px_x * slice.len()) / px_w;
                slice[s_idx.min(slice.len() - 1)]
            };

            let py = (center_y - sample * amp_scale)
                .round()
                .clamp(0.0, (px_h - 1) as f32) as usize;

            let y_start = prev_y.unwrap_or(py).min(py);
            let y_end = prev_y.unwrap_or(py).max(py);
            prev_y = Some(py);

            for y in y_start..=y_end {
                let cell_x = px_x / 2;
                let cell_y = y / 4;
                let sub_x = px_x % 2;
                let sub_y = y % 4;
                let bit = braille_bit(sub_x, sub_y);
                grid[cell_y * (area.width as usize) + cell_x] |= bit;
            }
        }

        for cy in 0..area.height {
            let norm_y = 1.0 - (f32::from(cy) / f32::from(area.height.max(1)));
            for cx in 0..area.width {
                let bits = grid[(cy as usize) * (area.width as usize) + (cx as usize)];
                if bits == 0 {
                    continue;
                }
                let norm_x = f32::from(cx) / f32::from(area.width.max(1));
                let color = self.theme.bar_color(norm_y, norm_x, 0.7, self.sys_palette);
                if let Some(ch) = char::from_u32(0x2800 + u32::from(bits))
                    && let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy))
                {
                    cell.set_char(ch).set_fg(color);
                }
            }
        }
    }
}

struct WaterfallWidget<'a> {
    history: &'a VecDeque<Vec<f32>>,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for WaterfallWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width == 0 || area.height == 0 || self.history.is_empty() {
            return;
        }

        let sample_energy = |row_idx: usize, norm_x: f32| -> f32 {
            let Some(frame) = self.history.get(row_idx) else {
                return 0.0;
            };
            if frame.is_empty() {
                return 0.0;
            }
            let idx = ((norm_x * (frame.len() - 1) as f32).round() as usize).min(frame.len() - 1);
            frame[idx].clamp(0.0, 1.0)
        };

        let heat_color = |energy: f32, norm_x: f32| -> Color {
            if energy <= 0.03 {
                Color::Rgb(8, 8, 12)
            } else {
                let base = self.theme.bar_color(energy, norm_x, energy, self.sys_palette);
                dim_rgb(base, 0.18 + energy * 0.82)
            }
        };

        for cy in 0..area.height {
            let top_idx = (cy as usize) * 2;
            let bot_idx = top_idx + 1;
            let y = area.y + cy;

            for cx in 0..area.width {
                let norm_x = if area.width > 1 {
                    f32::from(cx) / f32::from(area.width - 1)
                } else {
                    0.5
                };
                let e_top = sample_energy(top_idx, norm_x);
                let e_bot = sample_energy(bot_idx, norm_x);
                let fg = heat_color(e_top, norm_x);
                let bg = heat_color(e_bot, norm_x);

                if let Some(cell) = buf.cell_mut((area.x + cx, y)) {
                    cell.set_char('▀').set_fg(fg).set_bg(bg);
                }
            }
        }
    }
}

struct CircleWidget<'a> {
    bars: &'a [f32],
    bass_pulse: f32,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for CircleWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width < 4 || area.height < 3 || self.bars.is_empty() {
            return;
        }

        let px_w = (area.width as usize) * 2;
        let px_h = (area.height as usize) * 4;
        let mut grid = vec![0u8; (area.width as usize) * (area.height as usize)];

        let cx = (px_w as f32) * 0.5;
        let cy = (px_h as f32) * 0.5;
        let max_r = cx.min(cy) * 0.92;
        let base_r = max_r * (0.28 + self.bass_pulse * 0.08);
        let span_r = (max_r - base_r).max(2.0);

        let rays = self.bars.len().clamp(24, 96);
        for i in 0..rays {
            let angle = (i as f32 / rays as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let bar_idx = (i * self.bars.len()) / rays;
            let val = self.bars[bar_idx].clamp(0.0, 1.0);
            let outer_r = base_r + val * span_r;

            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let steps = ((outer_r - base_r).ceil() as usize).max(1);
            for s in 0..=steps {
                let r = base_r + (s as f32 / steps as f32) * (outer_r - base_r);
                let px = (cx + cos_a * r).round();
                let py = (cy + sin_a * r).round();
                if px >= 0.0 && px < px_w as f32 && py >= 0.0 && py < px_h as f32 {
                    let x = px as usize;
                    let y = py as usize;
                    grid[(y / 4) * (area.width as usize) + (x / 2)] |= braille_bit(x % 2, y % 4);
                }
            }
        }

        for cy_cell in 0..area.height {
            for cx_cell in 0..area.width {
                let bits = grid[(cy_cell as usize) * (area.width as usize) + (cx_cell as usize)];
                if bits == 0 {
                    continue;
                }
                let dx = f32::from(cx_cell) - f32::from(area.width) * 0.5;
                let dy = (f32::from(cy_cell) - f32::from(area.height) * 0.5) * 2.0;
                let norm_r = ((dx * dx + dy * dy).sqrt() / (max_r * 0.5).max(1.0)).clamp(0.0, 1.0);
                let norm_x = f32::from(cx_cell) / f32::from(area.width.max(1));
                let color = self.theme.bar_color(norm_r, norm_x, norm_r, self.sys_palette);
                if let Some(ch) = char::from_u32(0x2800 + u32::from(bits))
                    && let Some(cell) = buf.cell_mut((area.x + cx_cell, area.y + cy_cell))
                {
                    cell.set_char(ch).set_fg(color);
                }
            }
        }
    }
}

struct HorizonWidget<'a> {
    bars: &'a [f32],
    bass_pulse: f32,
    bar_density: BarDensity,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for HorizonWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width < 8 || area.height < 4 || self.bars.is_empty() {
            return;
        }

        let horizon_y = area.y + area.height / 2;
        let top_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: horizon_y - area.y,
        };

        // Render spectrum bars rising above the horizon line.
        BarsWidget {
            bars: self.bars,
            peaks: &[],
            reflection: false,
            bar_density: self.bar_density,
            theme: self.theme,
            sys_palette: self.sys_palette,
        }
        .render(top_area, buf);

        // Render Synthwave 3D perspective grid below the horizon line.
        let grid_rows = area.bottom().saturating_sub(horizon_y);
        if grid_rows == 0 {
            return;
        }
        let accent = self.theme.accent_color(self.sys_palette);
        let grid_dim = dim_rgb(accent, 0.35 + self.bass_pulse * 0.25);
        let cx = f32::from(area.width) * 0.5;
        let scroll = (self.sys_palette.time_secs * (1.2 + self.bass_pulse * 1.5)).fract();

        for r in 0..grid_rows {
            let y = horizon_y + r;
            let depth = (f32::from(r) + 1.0) / f32::from(grid_rows);
            // Perspective horizontal scanlines accelerating toward viewer.
            let z = (depth * 4.0 - scroll).rem_euclid(1.0);
            let is_h_line = r == 0 || z < 0.22;

            for cx_i in 0..area.width {
                let x = area.x + cx_i;
                let dx = f32::from(cx_i) - cx;
                // Converging perspective rays from horizon center.
                let ray_pos = (dx / (0.15 + depth * 0.85)) * 0.12;
                let is_v_line = (ray_pos - ray_pos.round()).abs() < 0.08;

                let ch = match (is_h_line, is_v_line) {
                    (true, true) => '┼',
                    (true, false) => '─',
                    (false, true) => {
                        if dx < -1.0 {
                            '/'
                        } else if dx > 1.0 {
                            '\\'
                        } else {
                            '│'
                        }
                    }
                    (false, false) => continue,
                };
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch).set_fg(if r == 0 { accent } else { grid_dim });
                }
            }
        }
    }
}

struct LissajousWidget<'a> {
    stereo_samples: &'a [(f32, f32)],
    gain: f32,
    theme: Theme,
    sys_palette: SystemPalette,
}

impl Widget for LissajousWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        if area.width < 4 || area.height < 3 || self.stereo_samples.is_empty() {
            return;
        }

        let px_w = (area.width as usize) * 2;
        let px_h = (area.height as usize) * 4;
        let mut grid = vec![0u8; (area.width as usize) * (area.height as usize)];

        let cx = (px_w as f32) * 0.5;
        let cy = (px_h as f32) * 0.5;
        let radius = cx.min(cy) * 0.9 * self.gain.clamp(0.4, 2.5);

        for &(l, r) in self.stereo_samples {
            // Classic 45-degree rotated stereo Lissajous: X = (L - R), Y = (L + R).
            let x_norm = (l - r) * 0.707;
            let y_norm = (l + r) * 0.707;
            let px = (cx + x_norm * radius).round();
            let py = (cy - y_norm * radius).round();
            if px >= 0.0 && px < px_w as f32 && py >= 0.0 && py < px_h as f32 {
                let x = px as usize;
                let y = py as usize;
                grid[(y / 4) * (area.width as usize) + (x / 2)] |= braille_bit(x % 2, y % 4);
            }
        }

        for cy_cell in 0..area.height {
            let norm_y = 1.0 - f32::from(cy_cell) / f32::from(area.height.max(1));
            for cx_cell in 0..area.width {
                let bits = grid[(cy_cell as usize) * (area.width as usize) + (cx_cell as usize)];
                if bits == 0 {
                    continue;
                }
                let norm_x = f32::from(cx_cell) / f32::from(area.width.max(1));
                let color = self.theme.bar_color(norm_y, norm_x, 0.8, self.sys_palette);
                if let Some(ch) = char::from_u32(0x2800 + u32::from(bits))
                    && let Some(cell) = buf.cell_mut((area.x + cx_cell, area.y + cy_cell))
                {
                    cell.set_char(ch).set_fg(color);
                }
            }
        }
    }
}

fn render_help_overlay(frame: &mut Frame, area: Rect, state: &UiState<'_>) {
    let popup_w = area.width.saturating_sub(4).min(62);
    let popup_h = area.height.saturating_sub(2).min(21);
    if popup_w < 24 || popup_h < 10 {
        return;
    }

    let popup = Rect {
        x: area.x + (area.width - popup_w) / 2,
        y: area.y + (area.height - popup_h) / 2,
        width: popup_w,
        height: popup_h,
    };

    let accent = state.theme.accent_color(state.sys_palette);
    let key_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(Color::White);
    let dim_style = Style::default().fg(Color::DarkGray);

    let row = |k1: &'static str, d1: &'static str, k2: &'static str, d2: &'static str| {
        Line::from(vec![
            Span::styled(format!(" {k1:<7}"), key_style),
            Span::styled(format!("{d1:<20}"), text_style),
            Span::styled(format!("{k2:<7}"), key_style),
            Span::styled(d2, text_style),
        ])
    };

    let lines = vec![
        Line::from(""),
        row("T", "Theme cycle", "Space", "Play / Pause"),
        row("M", "Spectrum mode", "← / →", "Prev / Next track"),
        row("F", "Clock font", "+ / -", "Gain adjust"),
        row("B", "Background FX", "A", "Auto-Gain (AGC)"),
        row("W", "Clock/Pomo/Watch", "Enter", "Reset timer"),
        row("I", "Album cover art", "X", "Party shuffle"),
        row("R", "Water reflection", "S", "Toggle seconds"),
        row("E", "Freq range", "D", "Bar density"),
        row("O", "OLED drift", "N", "Dismiss alarm"),
        row("C", "Compact widget", "P", "Freeze spectrum"),
        row("Z", "Zen auto-hide", "K", "Weather"),
        row("L", "Idle sleep", "Y", "Lyrics 1L/3L/Full"),
        row("Q/Esc", "Quit", "? / H", "Close help"),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " Active: {} • {} • {} • Bg:{}",
                state.theme.name(),
                state.mode.name(),
                state.clock_font.name(),
                state.bg_effect.name()
            ),
            dim_style,
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            " Hotkeys & Controls ",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ))
        .style(Style::default().bg(Color::Rgb(14, 14, 20)));

    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn braille_bit(sub_x: usize, sub_y: usize) -> u8 {
    match (sub_x, sub_y) {
        (0, 0) => 0x01,
        (0, 1) => 0x02,
        (0, 2) => 0x04,
        (1, 0) => 0x08,
        (1, 1) => 0x10,
        (1, 2) => 0x20,
        (0, 3) => 0x40,
        (1, 3) => 0x80,
        _ => 0,
    }
}
