# Configuration Guide

`spectrum-clock` stores its configuration and custom themes in your user config directory:

- **Windows**: `%APPDATA%\spectrum-clock\`
- **Linux / macOS**: `~/.config/spectrum-clock/`

---

## `config.conf`

Created and updated automatically whenever you toggle settings via hotkeys. You can also edit it manually while the app is closed.

### Keys & Defaults

| Key | Values | Default | Description |
| --- | --- | --- | --- |
| `theme` | `Cover`, `Cyberpunk`, `Synthwave`, `Neon`, `Rainbow`, `Matrix`, `Fire`, `Ocean`, `System` | `Cover` | Active built-in color theme |
| `custom_theme` | Theme name from `themes.conf` | *(none)* | Overrides built-in `theme` when set |
| `mode` | `Bars`, `Mirror`, `Waveform`, `Waterfall`, `Circle`, `Horizon`, `Lissajous` | `Bars` | Spectrum visualization mode |
| `freq_range` | `Full`, `Bass`, `Mids` | `Full` | Frequency band filter |
| `bar_density` | `Auto`, `Thin`, `Wide` | `Auto` | Bar width and spacing |
| `clock_font` | `Block`, `Neon`, `Segment`, `Dots` | `Block` | Big clock font style |
| `gain` | `0.2` – `5.0` | `1.40` | Manual spectrum gain multiplier |
| `auto_gain` | `true` / `false` | `true` | Automatic Gain Control (AGC) |
| `show_seconds` | `true` / `false` | `true` | Show seconds in clock |
| `zen_auto` | `true` / `false` | `true` | Auto-hide bottom bar after 4s idle |
| `bg_effect` | `Stars`, `Matrix`, `Aurora`, `Pulse`, `Fire`, `Off` | `Stars` | Background particle/shader effect |
| `reflection` | `true` / `false` | `true` | Water reflection under bars |
| `show_cover_art` | `true` / `false` | `true` | Show 12×6 album art next to clock |
| `party_mode` | `true` / `false` | `false` | Auto-cycle mode & background every track / 45s |
| `oled_drift` | `true` / `false` | `false` | Subtle pixel shifting to prevent OLED burn-in |
| `idle_sleep` | `true` / `false` | `true` | Hide spectrum after 15s of silence |
| `weather` | `true` / `false` | `true` | Show weather next to date |
| `weather_city` | City name (e.g. `London`, `Moscow`) | *(auto)* | Optional city override for `wttr.in` |
| `alarm` | `HH:MM\|Message` (or `HH:MM`) | *(none)* | Daily visual alarm trigger |
| `compact` | `true` / `false` | `false` | Compact single-line header widget mode |
| `lyrics_view` | `1line`, `3lines`, `full` | `3lines` | Lyrics display mode |

### Example `config.conf`

```ini
theme=Cover
mode=Bars
freq_range=Full
bar_density=Auto
clock_font=Block
gain=1.40
auto_gain=true
show_seconds=true
zen_auto=true
bg_effect=Stars
reflection=true
show_cover_art=true
party_mode=false
oled_drift=false
idle_sleep=true
weather=true
weather_city=Moscow
alarm=08:30|Wake up!
compact=false
lyrics_view=3lines
```

---

## Custom Themes (`themes.conf`)

You can define custom color themes in `themes.conf` inside the same config directory. Each line defines one theme using 3 hex RGB colors (`dark`, `base`, `light`):

```ini
# Format: Name = #dark, #base, #light
Vaporwave = #3b0a45, #ff71ce, #01cdfe
Nord = #2e3440, #88c0d0, #eceff4
AmberCRT = #3d1e00, #ff8c00, #ffd27d
```

Custom themes appear in the `T` theme cycle right after `System`.
