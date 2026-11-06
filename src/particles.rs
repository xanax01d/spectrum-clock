use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::theme::{SystemPalette, Theme};

const PARTICLE_GLYPHS: [char; 4] = ['·', '•', '✦', '*'];
const MATRIX_GLYPHS: [char; 12] = ['0', '1', 'ｱ', 'ｶ', 'ｻ', 'ﾀ', 'ﾅ', 'ﾊ', 'ﾐ', 'ﾒ', 'ﾗ', 'ﾜ'];

/// Selectable animated background effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BgEffect {
    #[default]
    Stars,
    Matrix,
    Aurora,
    Pulse,
    Fire,
    Off,
}

impl BgEffect {
    pub fn next(self) -> Self {
        match self {
            Self::Stars => Self::Matrix,
            Self::Matrix => Self::Aurora,
            Self::Aurora => Self::Pulse,
            Self::Pulse => Self::Fire,
            Self::Fire => Self::Off,
            Self::Off => Self::Stars,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Stars => "Stars",
            Self::Matrix => "Matrix",
            Self::Aurora => "Aurora",
            Self::Pulse => "Pulse",
            Self::Fire => "Fire",
            Self::Off => "Off",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stars" | "true" => Some(Self::Stars),
            "matrix" => Some(Self::Matrix),
            "aurora" => Some(Self::Aurora),
            "pulse" => Some(Self::Pulse),
            "fire" => Some(Self::Fire),
            "off" | "false" => Some(Self::Off),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    speed_y: f32,
    drift_x: f32,
    glyph_idx: usize,
    phase: f32,
}

#[derive(Debug, Clone, Copy)]
struct MatrixDrop {
    x_norm: f32,
    y_norm: f32,
    speed: f32,
    length: u16,
    seed: usize,
}

#[derive(Debug, Clone, Copy)]
struct PulseRing {
    radius: f32,
    alpha: f32,
}

/// Manages and renders all background visual effects.
pub struct BackgroundEngine {
    particles: Vec<Particle>,
    drops: Vec<MatrixDrop>,
    rings: Vec<PulseRing>,
    time: f32,
    last_bass: f32,
}

impl BackgroundEngine {
    pub fn new(count: usize) -> Self {
        let mut seed: u32 = 0x9E37_79B9;
        let mut next_f32 = || -> f32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (u32::MAX >> 8) as f32
        };

        let particles = (0..count)
            .map(|i| Particle {
                x: next_f32(),
                y: next_f32(),
                speed_y: 0.015 + next_f32() * 0.045,
                drift_x: (next_f32() - 0.5) * 0.012,
                glyph_idx: i % PARTICLE_GLYPHS.len(),
                phase: next_f32() * std::f32::consts::TAU,
            })
            .collect();

        let drops = (0..26)
            .map(|i| MatrixDrop {
                x_norm: next_f32(),
                y_norm: next_f32(),
                speed: 0.14 + next_f32() * 0.22,
                length: 4 + (next_f32() * 6.0) as u16,
                seed: i,
            })
            .collect();

        Self {
            particles,
            drops,
            rings: Vec::new(),
            time: 0.0,
            last_bass: 0.0,
        }
    }

    pub fn update(&mut self, effect: BgEffect, dt_secs: f32, bass_pulse: f32) {
        self.time += dt_secs;
        match effect {
            BgEffect::Stars => {
                let boost = 1.0 + bass_pulse * 2.2;
                for p in &mut self.particles {
                    p.y = (p.y + p.speed_y * boost * dt_secs).rem_euclid(1.0);
                    p.x = (p.x + p.drift_x * dt_secs).rem_euclid(1.0);
                    p.phase = (p.phase + dt_secs * 2.0).rem_euclid(std::f32::consts::TAU);
                }
            }
            BgEffect::Matrix => {
                let boost = 1.0 + bass_pulse * 1.4;
                for d in &mut self.drops {
                    d.y_norm = (d.y_norm + d.speed * boost * dt_secs).rem_euclid(1.25);
                }
            }
            BgEffect::Aurora | BgEffect::Fire | BgEffect::Off => {}
            BgEffect::Pulse => {
                if bass_pulse > 0.42 && self.last_bass <= 0.42 && self.rings.len() < 6 {
                    self.rings.push(PulseRing {
                        radius: 0.02,
                        alpha: 1.0,
                    });
                }
                for ring in &mut self.rings {
                    ring.radius += dt_secs * 0.55;
                    ring.alpha = (1.0 - ring.radius * 1.15).max(0.0);
                }
                self.rings.retain(|r| r.alpha > 0.02);
            }
        }
        self.last_bass = bass_pulse;
    }

    pub fn render(
        &self,
        effect: BgEffect,
        area: Rect,
        buf: &mut Buffer,
        theme: Theme,
        sys: SystemPalette,
        bass_pulse: f32,
    ) {
        if area.width < 4 || area.height < 4 {
            return;
        }
        match effect {
            BgEffect::Stars => self.render_stars(area, buf, theme, sys),
            BgEffect::Matrix => self.render_matrix(area, buf, theme, sys),
            BgEffect::Aurora => self.render_aurora(area, buf, theme, sys, bass_pulse),
            BgEffect::Pulse => self.render_pulse(area, buf, theme, sys),
            BgEffect::Fire => self.render_fire(area, buf, bass_pulse),
            BgEffect::Off => {}
        }
    }

    fn render_stars(&self, area: Rect, buf: &mut Buffer, theme: Theme, sys: SystemPalette) {
        let dim_color = scale_color(theme.accent_color(sys), 0.28, 12);
        for p in &self.particles {
            let x = area.x + (p.x * f32::from(area.width)).floor() as u16;
            let y = area.y + (p.y * f32::from(area.height)).floor() as u16;
            if x < area.right()
                && y < area.bottom()
                && let Some(cell) = buf.cell_mut((x, y))
            {
                let ch = if p.phase.sin() > 0.2 {
                    PARTICLE_GLYPHS[p.glyph_idx]
                } else {
                    '·'
                };
                cell.set_char(ch).set_fg(dim_color);
            }
        }
    }

    fn render_matrix(&self, area: Rect, buf: &mut Buffer, theme: Theme, sys: SystemPalette) {
        let accent = theme.accent_color(sys);
        let head_color = scale_color(accent, 0.55, 20);

        for d in &self.drops {
            let x = area.x + (d.x_norm * f32::from(area.width)).floor() as u16;
            if x >= area.right() {
                continue;
            }
            let head_y = (d.y_norm * f32::from(area.height)).floor() as i32;

            for step in 0..i32::from(d.length) {
                let y_i32 = head_y - step;
                if y_i32 < i32::from(area.y) || y_i32 >= i32::from(area.bottom()) {
                    continue;
                }
                let y = y_i32 as u16;
                let fade = 1.0 - (step as f32 / f32::from(d.length));
                let color = if step == 0 {
                    head_color
                } else {
                    scale_color(accent, 0.14 + 0.26 * fade, 8)
                };
                let glyph_idx = (d.seed + (y as usize) + (self.time * 4.0) as usize) % MATRIX_GLYPHS.len();
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(MATRIX_GLYPHS[glyph_idx]).set_fg(color);
                }
            }
        }
    }

    fn render_aurora(
        &self,
        area: Rect,
        buf: &mut Buffer,
        theme: Theme,
        sys: SystemPalette,
        bass_pulse: f32,
    ) {
        let w = f32::from(area.width.max(1));
        let h = f32::from(area.height.max(1));
        let intensity = 0.22 + bass_pulse * 0.14;

        for cy in (0..area.height).step_by(2) {
            let ny = f32::from(cy) / h;
            for cx in (0..area.width).step_by(2) {
                let nx = f32::from(cx) / w;
                let wave = ((nx * 5.0 + self.time * 0.7).sin()
                    + (ny * 4.0 - self.time * 0.5).cos()
                    + ((nx + ny) * 3.5 + self.time * 0.4).sin())
                    / 3.0;

                if wave < 0.25 {
                    continue;
                }
                let ch = if wave > 0.62 {
                    '▒'
                } else if wave > 0.42 {
                    '░'
                } else {
                    '·'
                };
                let base = theme.bar_color(1.0 - ny, nx, wave, sys);
                let color = scale_color(base, intensity * wave, 6);
                if let Some(cell) = buf.cell_mut((area.x + cx, area.y + cy)) {
                    cell.set_char(ch).set_fg(color);
                }
            }
        }
    }

    fn render_fire(&self, area: Rect, buf: &mut Buffer, bass_pulse: f32) {
        let flame_rows = (area.height / 2).max(3);
        let start_y = area.bottom().saturating_sub(flame_rows);
        let boost = 0.75 + bass_pulse * 0.6;

        for y in start_y..area.bottom() {
            let from_bottom = f32::from(area.bottom() - 1 - y) / f32::from(flame_rows);
            for x in area.x..area.right() {
                let nx = f32::from(x - area.x);
                let flicker = ((nx * 0.55 + self.time * 6.5).sin()
                    + (nx * 1.15 - self.time * 9.0 + from_bottom * 4.0).cos())
                    * 0.5;
                let heat = (1.0 - from_bottom) * boost + flicker * 0.25;
                if heat < 0.36 {
                    continue;
                }
                let ch = if heat > 0.88 {
                    '▓'
                } else if heat > 0.62 {
                    '▒'
                } else {
                    '░'
                };
                let color = if heat > 0.85 {
                    Color::Rgb(160, 110, 25)
                } else if heat > 0.58 {
                    Color::Rgb(120, 45, 10)
                } else {
                    Color::Rgb(70, 15, 8)
                };
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch).set_fg(color);
                }
            }
        }
    }

    fn render_pulse(&self, area: Rect, buf: &mut Buffer, theme: Theme, sys: SystemPalette) {
        if self.rings.is_empty() {
            return;
        }
        let cx = f32::from(area.x) + f32::from(area.width) * 0.5;
        let cy = f32::from(area.y) + f32::from(area.height) * 0.5;
        let scale_x = f32::from(area.width) * 0.5;
        let scale_y = f32::from(area.height) * 0.5;
        let accent = theme.accent_color(sys);

        for y in area.y..area.bottom() {
            let dy = (f32::from(y) - cy) / scale_y.max(1.0);
            for x in area.x..area.right() {
                let dx = (f32::from(x) - cx) / scale_x.max(1.0);
                let dist = (dx * dx + dy * dy).sqrt();

                for ring in &self.rings {
                    if (dist - ring.radius).abs() < 0.045 {
                        let color = scale_color(accent, 0.35 * ring.alpha, 8);
                        let ch = if ring.alpha > 0.5 { '◦' } else { '·' };
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            cell.set_char(ch).set_fg(color);
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn scale_color(color: Color, factor: f32, floor: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (f32::from(r) * factor).round().clamp(f32::from(floor), 255.0) as u8,
            (f32::from(g) * factor).round().clamp(f32::from(floor), 255.0) as u8,
            (f32::from(b) * factor).round().clamp(f32::from(floor), 255.0) as u8,
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_effect_cycles_all_variants() {
        let start = BgEffect::Stars;
        let mut cur = start;
        for _ in 0..6 {
            cur = cur.next();
        }
        assert_eq!(cur, start);
    }
}
