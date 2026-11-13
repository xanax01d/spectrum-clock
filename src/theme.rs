use ratatui::style::Color;

/// User-defined 3-color theme loaded from `themes.conf`.
#[derive(Debug, Clone)]
pub struct CustomTheme {
    pub name: String,
    pub palette: SystemPalette,
}

/// Parses a `#RRGGBB` hex color string into `(r, g, b)`.
pub fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Built-in color themes for the spectrum visualizer and clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Cover,
    Cyberpunk,
    Synthwave,
    Neon,
    Rainbow,
    Matrix,
    Fire,
    Ocean,
    System,
}

/// Holds resolved system accent palette (dark, base, light) and animation time.
#[derive(Debug, Clone, Copy)]
pub struct SystemPalette {
    pub dark: (u8, u8, u8),
    pub base: (u8, u8, u8),
    pub light: (u8, u8, u8),
    pub time_secs: f32,
}

impl Default for SystemPalette {
    fn default() -> Self {
        Self {
            dark: (48, 63, 159),
            base: (0, 188, 212),
            light: (178, 235, 242),
            time_secs: 0.0,
        }
    }
}

impl SystemPalette {
    /// Queries the OS accent color on Windows; falls back to a default palette on other OSes.
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Some(palette) = query_windows_accent() {
                return palette;
            }
        }
        Self::default()
    }

    /// Smoothly interpolates between `self` and `target` by factor `t` in `[0.0, 1.0]`.
    pub fn lerp(self, target: Self, t: f32) -> Self {
        Self {
            dark: lerp_rgb(self.dark, target.dark, t),
            base: lerp_rgb(self.base, target.base, t),
            light: lerp_rgb(self.light, target.light, t),
            time_secs: target.time_secs,
        }
    }
}

#[cfg(target_os = "windows")]
fn query_windows_accent() -> Option<SystemPalette> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    let settings = UISettings::new().ok()?;
    let to_rgb = |kind: UIColorType| -> Option<(u8, u8, u8)> {
        let c = settings.GetColorValue(kind).ok()?;
        Some((c.R, c.G, c.B))
    };

    Some(SystemPalette {
        dark: to_rgb(UIColorType::AccentDark1)?,
        base: to_rgb(UIColorType::Accent)?,
        light: to_rgb(UIColorType::AccentLight2)?,
        time_secs: 0.0,
    })
}

impl Theme {
    /// Returns the next theme in rotation.
    pub fn next(self) -> Self {
        match self {
            Self::Cover => Self::Cyberpunk,
            Self::Cyberpunk => Self::Synthwave,
            Self::Synthwave => Self::Neon,
            Self::Neon => Self::Rainbow,
            Self::Rainbow => Self::Matrix,
            Self::Matrix => Self::Fire,
            Self::Fire => Self::Ocean,
            Self::Ocean => Self::System,
            Self::System => Self::Cover,
        }
    }

    /// Returns the display name of the theme.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cover => "Cover",
            Self::Cyberpunk => "Cyberpunk",
            Self::Synthwave => "Synthwave",
            Self::Neon => "Neon",
            Self::Rainbow => "Rainbow",
            Self::Matrix => "Matrix",
            Self::Fire => "Fire",
            Self::Ocean => "Ocean",
            Self::System => "System",
        }
    }

    /// Parses a theme from its display name (case-insensitive).
    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cover" => Some(Self::Cover),
            "cyberpunk" => Some(Self::Cyberpunk),
            "synthwave" => Some(Self::Synthwave),
            "neon" => Some(Self::Neon),
            "rainbow" => Some(Self::Rainbow),
            "matrix" => Some(Self::Matrix),
            "fire" => Some(Self::Fire),
            "ocean" => Some(Self::Ocean),
            "system" => Some(Self::System),
            _ => None,
        }
    }

    /// Computes a smooth, energy-reactive RGB gradient color for a spectrum cell.
    /// `norm_y`, `norm_x`, and `energy` are in `[0.0, 1.0]`.
    pub fn bar_color(
        self,
        norm_y: f32,
        norm_x: f32,
        energy: f32,
        sys: SystemPalette,
    ) -> Color {
        let y = norm_y.clamp(0.0, 1.0);
        let x = norm_x.clamp(0.0, 1.0);
        let boost = (energy.clamp(0.0, 1.0) * 0.25).min(0.25);

        let rgb = match self {
            Self::Cyberpunk => {
                // Electric Cyan -> Violet -> Hot Magenta
                let bottom = lerp_rgb((0, 255, 224), (0, 175, 255), x);
                let mid = lerp_rgb((138, 43, 226), (255, 42, 165), x);
                let top = lerp_rgb((255, 85, 210), (255, 220, 255), x);
                gradient3(bottom, mid, top, y)
            }
            Self::Synthwave => {
                // Deep Indigo -> Hot Pink -> Sunset Gold
                gradient3((92, 15, 196), (255, 40, 130), (255, 210, 40), y)
            }
            Self::Neon => {
                // Smooth horizontal spectrum (Cyan -> Green -> Yellow -> Pink) + vertical glow
                let base = if x < 0.33 {
                    lerp_rgb((0, 230, 255), (0, 255, 140), x / 0.33)
                } else if x < 0.66 {
                    lerp_rgb((0, 255, 140), (255, 230, 40), (x - 0.33) / 0.33)
                } else {
                    lerp_rgb((255, 230, 40), (255, 50, 180), (x - 0.66) / 0.34)
                };
                let dark = scale_rgb(base, 0.45);
                let bright = lerp_rgb(base, (255, 255, 255), 0.35);
                gradient3(dark, base, bright, y)
            }
            Self::Rainbow => {
                let hue = (x * 0.85 + sys.time_secs * 0.18 - y * 0.15).rem_euclid(1.0);
                hsv_to_rgb(hue, 0.92, 0.48 + 0.52 * y)
            }
            Self::Matrix => {
                // Deep Emerald -> Neon Green -> Mint Glow
                gradient3((0, 85, 25), (0, 255, 75), (190, 255, 210), y)
            }
            Self::Fire => {
                // Deep Crimson -> Fiery Orange -> White-Hot Yellow
                gradient3((180, 12, 12), (255, 115, 0), (255, 245, 140), y)
            }
            Self::Ocean => {
                // Abyssal Blue -> Turquoise -> Seafoam White
                let bottom = lerp_rgb((10, 36, 140), (0, 130, 200), x);
                let mid = lerp_rgb((0, 195, 235), (40, 235, 215), x);
                let top = (195, 250, 255);
                gradient3(bottom, mid, top, y)
            }
            Self::System | Self::Cover => gradient3(sys.dark, sys.base, sys.light, y),
        };

        let boosted = lerp_rgb(rgb, (255, 255, 255), boost * y);
        Color::Rgb(boosted.0, boosted.1, boosted.2)
    }

    /// Vertical gradient color for each row of the big clock.
    pub fn clock_row_color(self, row: usize, total_rows: usize, sys: SystemPalette) -> Color {
        let t = if total_rows <= 1 {
            0.0
        } else {
            row as f32 / (total_rows - 1) as f32
        };

        let (top, bottom) = match self {
            Self::Cyberpunk => ((140, 255, 245), (255, 95, 195)),
            Self::Synthwave => ((255, 220, 90), (255, 65, 165)),
            Self::Neon => ((120, 255, 220), (140, 180, 255)),
            Self::Rainbow => {
                let c1 = hsv_to_rgb((sys.time_secs * 0.18).rem_euclid(1.0), 0.85, 1.0);
                let c2 = hsv_to_rgb((sys.time_secs * 0.18 + 0.22).rem_euclid(1.0), 0.9, 0.95);
                (c1, c2)
            }
            Self::Matrix => ((185, 255, 205), (0, 210, 60)),
            Self::Fire => ((255, 240, 130), (255, 85, 25)),
            Self::Ocean => ((210, 250, 255), (0, 165, 230)),
            Self::System | Self::Cover => (sys.light, sys.base),
        };

        let (r, g, b) = lerp_rgb(top, bottom, t);
        Color::Rgb(r, g, b)
    }

    /// Color used for peak-hold markers above the bars.
    pub fn peak_color(self) -> Color {
        match self {
            Self::Cyberpunk => Color::Rgb(255, 240, 250),
            Self::Synthwave => Color::Rgb(255, 240, 160),
            Self::Neon => Color::Rgb(255, 255, 255),
            Self::Rainbow => Color::Rgb(255, 255, 255),
            Self::Matrix => Color::Rgb(215, 255, 225),
            Self::Fire => Color::Rgb(255, 255, 215),
            Self::Ocean => Color::Rgb(225, 252, 255),
            Self::System | Self::Cover => Color::Rgb(245, 248, 255),
        }
    }

    /// Secondary accent color for date, Now Playing, and status bar.
    pub fn accent_color(self, sys: SystemPalette) -> Color {
        match self {
            Self::Cyberpunk => Color::Rgb(255, 105, 195),
            Self::Synthwave => Color::Rgb(255, 200, 30),
            Self::Neon => Color::Rgb(0, 245, 180),
            Self::Rainbow => {
                let (r, g, b) = hsv_to_rgb((sys.time_secs * 0.18 + 0.5).rem_euclid(1.0), 0.85, 1.0);
                Color::Rgb(r, g, b)
            }
            Self::Matrix => Color::Rgb(0, 195, 55),
            Self::Fire => Color::Rgb(255, 120, 45),
            Self::Ocean => Color::Rgb(0, 205, 235),
            Self::System | Self::Cover => Color::Rgb(sys.light.0, sys.light.1, sys.light.2),
        }
    }
}

/// Brightens an RGB `Color` toward white by `amount` in `[0.0, 1.0]`.
pub fn brighten_color(color: Color, amount: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let (nr, ng, nb) = lerp_rgb((r, g, b), (255, 255, 255), amount);
            Color::Rgb(nr, ng, nb)
        }
        other => other,
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h6 = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h6 % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h6 as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn gradient3(low: (u8, u8, u8), mid: (u8, u8, u8), high: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    if t < 0.5 {
        lerp_rgb(low, mid, t * 2.0)
    } else {
        lerp_rgb(mid, high, (t - 0.5) * 2.0)
    }
}

fn scale_rgb(c: (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    let scale = |v: u8| -> u8 { (f32::from(v) * factor).round().clamp(0.0, 255.0) as u8 };
    (scale(c.0), scale(c.1), scale(c.2))
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let lerp = |c1: u8, c2: u8| -> u8 {
        let v = f32::from(c1) + (f32::from(c2) - f32::from(c1)) * t;
        v.round().clamp(0.0, 255.0) as u8
    };
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_rgb_colors() {
        assert_eq!(parse_hex_rgb("#ff0077"), Some((255, 0, 119)));
        assert_eq!(parse_hex_rgb("112233"), Some((17, 34, 51)));
        assert_eq!(parse_hex_rgb("#xyz"), None);
    }

    #[test]
    fn theme_cycles_all_variants() {
        let start = Theme::Cover;
        let mut current = start;
        for _ in 0..9 {
            current = current.next();
        }
        assert_eq!(current, start);
    }

    #[test]
    fn bar_color_clamps_out_of_range() {
        let sys = SystemPalette::default();
        let c1 = Theme::Fire.bar_color(-1.0, -0.5, 0.0, sys);
        let c2 = Theme::Fire.bar_color(0.0, 0.0, 0.0, sys);
        assert_eq!(c1, c2);

        let c3 = Theme::Fire.bar_color(2.0, 1.5, 1.0, sys);
        let c4 = Theme::Fire.bar_color(1.0, 1.0, 1.0, sys);
        assert_eq!(c3, c4);
    }
}
