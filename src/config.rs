use std::fs;
use std::path::PathBuf;

use crate::clock::ClockFont;
use crate::fft::FreqRange;
use crate::particles::BgEffect;
use crate::theme::{CustomTheme, SystemPalette, Theme, parse_hex_rgb};
use crate::ui::{BarDensity, LyricsView, SpectrumMode};

/// Persisted user preferences.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: Theme,
    pub custom_theme: Option<String>,
    pub mode: SpectrumMode,
    pub freq_range: FreqRange,
    pub bar_density: BarDensity,
    pub clock_font: ClockFont,
    pub gain: f32,
    pub auto_gain: bool,
    pub show_seconds: bool,
    pub zen_auto: bool,
    pub bg_effect: BgEffect,
    pub reflection: bool,
    pub show_cover_art: bool,
    pub party_mode: bool,
    pub oled_drift: bool,
    pub idle_sleep: bool,
    pub weather: bool,
    pub weather_city: Option<String>,
    pub alarm_time: Option<String>,
    pub alarm_message: String,
    pub compact: bool,
    pub lyrics_view: LyricsView,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Cover,
            custom_theme: None,
            mode: SpectrumMode::Bars,
            freq_range: FreqRange::Full,
            bar_density: BarDensity::Auto,
            clock_font: ClockFont::Block,
            gain: 1.4,
            auto_gain: true,
            show_seconds: true,
            zen_auto: true,
            bg_effect: BgEffect::Stars,
            reflection: true,
            show_cover_art: true,
            party_mode: false,
            oled_drift: false,
            idle_sleep: true,
            weather: true,
            weather_city: None,
            alarm_time: None,
            alarm_message: "Alarm!".to_string(),
            compact: false,
            lyrics_view: LyricsView::default(),
        }
    }
}

impl AppConfig {
    /// Loads configuration from disk, returning defaults if missing or unreadable.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(content) = fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&content)
    }

    /// Saves configuration to disk, ignoring I/O errors silently.
    pub fn save(&self) {
        let Some(path) = config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, self.serialize());
    }

    fn serialize(&self) -> String {
        let alarm_line = match &self.alarm_time {
            Some(t) => format!("alarm={t}|{}\n", self.alarm_message),
            None => String::new(),
        };
        let custom_line = match &self.custom_theme {
            Some(name) => format!("custom_theme={name}\n"),
            None => String::new(),
        };
        let city_line = match &self.weather_city {
            Some(c) => format!("weather_city={c}\n"),
            None => String::new(),
        };
        format!(
            "theme={}\n{custom_line}mode={}\nfreq_range={}\nbar_density={}\nclock_font={}\ngain={:.2}\nauto_gain={}\nshow_seconds={}\nzen_auto={}\nbg_effect={}\nreflection={}\nshow_cover_art={}\nparty_mode={}\noled_drift={}\nidle_sleep={}\nweather={}\n{city_line}{alarm_line}compact={}\nlyrics_view={}\n",
            self.theme.name(),
            self.mode.name(),
            self.freq_range.name(),
            self.bar_density.name(),
            self.clock_font.name(),
            self.gain,
            self.auto_gain,
            self.show_seconds,
            self.zen_auto,
            self.bg_effect.name(),
            self.reflection,
            self.show_cover_art,
            self.party_mode,
            self.oled_drift,
            self.idle_sleep,
            self.weather,
            self.compact,
            self.lyrics_view.name()
        )
    }

    fn parse(input: &str) -> Self {
        let mut cfg = Self::default();
        for line in input.lines() {
            let Some((key, val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let val = val.trim();
            match key {
                "theme" => {
                    if let Some(t) = Theme::from_name(val) {
                        cfg.theme = t;
                    }
                }
                "custom_theme" => {
                    if !val.is_empty() {
                        cfg.custom_theme = Some(val.to_string());
                    }
                }
                "mode" => {
                    if let Some(m) = SpectrumMode::from_name(val) {
                        cfg.mode = m;
                    }
                }
                "freq_range" => {
                    if let Some(r) = FreqRange::from_name(val) {
                        cfg.freq_range = r;
                    }
                }
                "bar_density" => {
                    if let Some(d) = BarDensity::from_name(val) {
                        cfg.bar_density = d;
                    }
                }
                "clock_font" => {
                    if let Some(f) = ClockFont::from_name(val) {
                        cfg.clock_font = f;
                    }
                }
                "gain" => {
                    if let Ok(g) = val.parse::<f32>() {
                        cfg.gain = g.clamp(0.2, 5.0);
                    }
                }
                "auto_gain" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.auto_gain = b;
                    }
                }
                "show_seconds" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_seconds = b;
                    }
                }
                "zen_auto" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.zen_auto = b;
                    }
                }
                "bg_effect" | "particles" => {
                    if let Some(bg) = BgEffect::from_name(val) {
                        cfg.bg_effect = bg;
                    }
                }
                "reflection" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.reflection = b;
                    }
                }
                "show_cover_art" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.show_cover_art = b;
                    }
                }
                "party_mode" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.party_mode = b;
                    }
                }
                "oled_drift" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.oled_drift = b;
                    }
                }
                "idle_sleep" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.idle_sleep = b;
                    }
                }
                "weather" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.weather = b;
                    }
                }
                "weather_city" => {
                    if !val.is_empty() {
                        cfg.weather_city = Some(val.to_string());
                    }
                }
                "alarm" => {
                    if let Some((t, msg)) = val.split_once('|') {
                        let time = t.trim();
                        if time.len() == 5 && time.as_bytes()[2] == b':' {
                            cfg.alarm_time = Some(time.to_string());
                            if !msg.trim().is_empty() {
                                cfg.alarm_message = msg.trim().to_string();
                            }
                        }
                    } else if val.len() == 5 && val.as_bytes()[2] == b':' {
                        cfg.alarm_time = Some(val.to_string());
                    }
                }
                "compact" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.compact = b;
                    }
                }
                "lyrics_view" => {
                    if let Some(v) = LyricsView::from_name(val) {
                        cfg.lyrics_view = v;
                    }
                }
                "lyrics_mode" => {
                    if let Ok(b) = val.parse::<bool>() {
                        cfg.lyrics_view = if b {
                            LyricsView::Full
                        } else {
                            LyricsView::ThreeLines
                        };
                    }
                }
                _ => {}
            }
        }
        cfg
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(not(target_os = "windows"))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));

    base.map(|dir| dir.join("spectrum-clock"))
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.conf"))
}

/// Loads user-defined 3-color themes from `themes.conf`, creating an example file if missing.
pub fn load_custom_themes() -> Vec<CustomTheme> {
    let Some(dir) = config_dir() else {
        return Vec::new();
    };
    let path = dir.join("themes.conf");
    if !path.exists() {
        let _ = fs::create_dir_all(&dir);
        let template = "\
# Custom 3-color themes: Name=#dark,#base,#light\n\
# Vaporwave=#3b0a45,#ff71ce,#01cdfe\n\
# GoldDark=#4a2c00,#ffb300,#fff3b0\n";
        let _ = fs::write(&path, template);
        return Vec::new();
    }

    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_custom_themes(&content)
}

fn parse_custom_themes(content: &str) -> Vec<CustomTheme> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, colors)) = trimmed.split_once('=') else {
            continue;
        };
        let parts: Vec<&str> = colors.split(',').collect();
        if parts.len() != 3 {
            continue;
        }
        if let (Some(dark), Some(base), Some(light)) = (
            parse_hex_rgb(parts[0]),
            parse_hex_rgb(parts[1]),
            parse_hex_rgb(parts[2]),
        ) {
            out.push(CustomTheme {
                name: name.trim().to_string(),
                palette: SystemPalette {
                    dark,
                    base,
                    light,
                    time_secs: 0.0,
                },
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let orig = AppConfig {
            theme: Theme::Synthwave,
            custom_theme: Some("Vaporwave".to_string()),
            mode: SpectrumMode::Mirror,
            freq_range: FreqRange::Bass,
            bar_density: BarDensity::Wide,
            clock_font: ClockFont::Neon,
            gain: 2.2,
            auto_gain: false,
            show_seconds: false,
            zen_auto: false,
            bg_effect: BgEffect::Aurora,
            reflection: false,
            show_cover_art: false,
            party_mode: true,
            oled_drift: true,
            idle_sleep: false,
            weather: false,
            weather_city: Some("Moscow".to_string()),
            alarm_time: Some("23:30".to_string()),
            alarm_message: "Sleep time".to_string(),
            compact: true,
            lyrics_view: LyricsView::OneLine,
        };
        let parsed = AppConfig::parse(&orig.serialize());
        assert_eq!(parsed.theme, orig.theme);
        assert_eq!(parsed.custom_theme, orig.custom_theme);
        assert_eq!(parsed.mode, orig.mode);
        assert_eq!(parsed.freq_range, orig.freq_range);
        assert_eq!(parsed.bar_density, orig.bar_density);
        assert_eq!(parsed.clock_font, orig.clock_font);
        assert!((parsed.gain - orig.gain).abs() < 0.01);
        assert_eq!(parsed.auto_gain, orig.auto_gain);
        assert_eq!(parsed.show_seconds, orig.show_seconds);
        assert_eq!(parsed.zen_auto, orig.zen_auto);
        assert_eq!(parsed.bg_effect, orig.bg_effect);
        assert_eq!(parsed.reflection, orig.reflection);
        assert_eq!(parsed.show_cover_art, orig.show_cover_art);
        assert_eq!(parsed.party_mode, orig.party_mode);
        assert_eq!(parsed.oled_drift, orig.oled_drift);
        assert_eq!(parsed.idle_sleep, orig.idle_sleep);
        assert_eq!(parsed.weather, orig.weather);
        assert_eq!(parsed.weather_city, orig.weather_city);
        assert_eq!(parsed.alarm_time, orig.alarm_time);
        assert_eq!(parsed.alarm_message, orig.alarm_message);
        assert_eq!(parsed.compact, orig.compact);
    }

    #[test]
    fn parses_custom_themes_file() {
        let input = "# comment\nVaporwave=#3b0a45,#ff71ce,#01cdfe\nBadLine\n";
        let themes = parse_custom_themes(input);
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].name, "Vaporwave");
        assert_eq!(themes[0].palette.base, (255, 113, 206));
    }
}
