# spectrum-clock

A real-time terminal audio spectrum analyzer, clock, and synced lyrics viewer built in Rust with `ratatui`, `rustfft`, and `cpal`.

Designed primarily for Windows Terminal (with WASAPI loopback capture and Windows Media Session integration), with cross-platform audio fallback.

## Features

- **7 Spectrum Visualizations (`M`)**: `Bars`, `Mirror`, `Waveform`, `Waterfall`, `Circle`, `Horizon`, `Lissajous`
- **9 Color Themes + Custom Themes (`T`)**:
  - `Cover` (adaptive palette extracted from the current track's album art)
  - `Cyberpunk`, `Synthwave`, `Neon`, `Rainbow`, `Matrix`, `Fire`, `Ocean`
  - `System` (OS accent color)
  - Custom themes loaded from `themes.conf`
- **4 Big Clock Fonts (`F`)**: `Block`, `Neon`, `Segment`, `Dots`
- **6 Animated Background Effects (`B`)**: `Stars`, `Matrix`, `Aurora`, `Pulse`, `Fire`, `Off`
- **Synced Lyrics & Dedicated Lyrics Mode (`Y`)**:
  - Cycles between **1-line**, **3-line karaoke** (`Prev / Active / Next`), and **Full-screen Lyrics Mode**
  - Automatically fetches `.lrc` synced lyrics (with `plainLyrics` fallback) from `lrclib.net`
- **Now Playing & Media Controls (Windows SMTC)**:
  - Displays current track, progress bar, and 12×6 pixel album cover art (`I`)
  - Play/Pause (`Space`), Previous (`←`), Next (`→`)
- **Weather Next to Date (`K`)**:
  - Background poller via `wttr.in` every 30 minutes (auto-detected location or custom `weather_city`)
- **Idle Sleep (`L`)**:
  - Automatically hides the spectrum after 15 seconds of silence, leaving only the clock, lyrics, and background effect; wakes up immediately when audio resumes
- **Audio & Display Controls**:
  - Real-time FFT with `Full` (30 Hz–16 kHz), `Bass` (25–250 Hz), and `Mids` (250 Hz–4 kHz) ranges (`E`)
  - Auto-Gain Control (`A`) and manual gain (`+` / `-` or `↑` / `↓`)
  - Stereo L/R VU meters and live BPM detection
  - Water reflection (`R`), Bar density (`D`), OLED burn-in drift (`O`), Compact widget mode (`C`), Zen auto-hide footer (`Z`), Party shuffle (`X`)
  - Built-in Clock / Pomodoro / Stopwatch (`W`, `Enter` to reset) and daily alarm (`N` to dismiss)
  - Footer displays only currently active features and modes

## Requirements

- **Rust** (Edition 2024, e.g. Rust 1.85+)
- A terminal with **TrueColor (24-bit RGB)** and Unicode support (Windows Terminal recommended)

## Installation & Running

```bash
git clone <your-repo-url>
cd spectrum-clock
cargo run --release
```

To run tests and lints:

```bash
cargo clippy --all-targets -- -D warnings
cargo test
```

## Controls & Hotkeys

| Key | Action | Key | Action |
| --- | --- | --- | --- |
| `T` | Cycle color theme | `Space` | Play / Pause media |
| `M` | Cycle spectrum mode | `←` / `→` | Previous / Next track |
| `F` | Cycle clock font | `+` / `-` (or `↑` / `↓`) | Adjust gain |
| `B` | Cycle background FX | `A` | Toggle Auto-Gain (AGC) |
| `Y` | Cycle lyrics (`1L` / `3L` / `Full`) | `Enter` | Reset Pomodoro / Stopwatch |
| `W` | Switch Clock / Pomo / Stopwatch | `X` | Toggle Party shuffle |
| `I` | Toggle album cover art | `S` | Toggle seconds |
| `R` | Toggle water reflection | `D` | Cycle bar density |
| `E` | Cycle frequency range | `N` | Dismiss active alarm |
| `O` | Toggle OLED pixel drift | `P` | Freeze / Pause spectrum |
| `C` | Toggle compact widget mode | `K` | Toggle weather display |
| `Z` | Toggle Zen auto-hide footer | `L` | Toggle Idle Sleep |
| `?` / `H` | Toggle help overlay | `Q` / `Esc` | Quit |

## Configuration

Settings are automatically saved to `config.conf` on change:

- **Windows**: `%APPDATA%\spectrum-clock\config.conf`
- **Linux / macOS**: `~/.config/spectrum-clock/config.conf`

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for all available options, alarm setup, and custom theme syntax (`themes.conf`).

## Architecture

- `src/main.rs` — Event loop, input handling, audio/lyrics/weather coordination, and config persistence
- `src/audio.rs` — WASAPI loopback (Windows) and `cpal` audio capture
- `src/fft.rs` — 2048-point FFT spectrum analyzer, AGC, bass pulse, and BPM estimation
- `src/ui.rs` — `ratatui` widgets, spectrum modes, lyrics mode, VU meters, and overlays
- `src/clock.rs` — Big ASCII/Unicode clock fonts, Pomodoro timer, and stopwatch
- `src/theme.rs` — Built-in color palettes, system accent detection, and custom theme parser
- `src/particles.rs` — Animated background effect engine
- `src/now_playing.rs` — Windows SMTC integration and album art palette extraction
- `src/lyrics.rs` — Background `.lrc` / plain lyrics fetcher (`lrclib.net`)
- `src/weather.rs` — Background weather poller (`wttr.in`)
- `src/config.rs` — `config.conf` and `themes.conf` serialization/parsing

## License

Licensed under the [MIT License](LICENSE).
