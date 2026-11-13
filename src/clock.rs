/// Renders time strings as 5-row Unicode art in multiple selectable font styles.
const DIGIT_HEIGHT: usize = 5;

/// Operating mode for the main clock display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockMode {
    #[default]
    Clock,
    Pomodoro,
    Stopwatch,
}

impl ClockMode {
    pub fn next(self) -> Self {
        match self {
            Self::Clock => Self::Pomodoro,
            Self::Pomodoro => Self::Stopwatch,
            Self::Stopwatch => Self::Clock,
        }
    }
}

/// Tracks Pomodoro countdown and Stopwatch elapsed time.
pub struct TimerState {
    pub mode: ClockMode,
    pub pomo_remaining: f32,
    pub pomo_is_break: bool,
    pub stopwatch_secs: f32,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            mode: ClockMode::Clock,
            pomo_remaining: 25.0 * 60.0,
            pomo_is_break: false,
            stopwatch_secs: 0.0,
        }
    }
}

impl TimerState {
    pub fn update(&mut self, dt_secs: f32) {
        match self.mode {
            ClockMode::Clock => {}
            ClockMode::Pomodoro => {
                self.pomo_remaining -= dt_secs;
                if self.pomo_remaining <= 0.0 {
                    self.pomo_is_break = !self.pomo_is_break;
                    self.pomo_remaining = if self.pomo_is_break {
                        5.0 * 60.0
                    } else {
                        25.0 * 60.0
                    };
                }
            }
            ClockMode::Stopwatch => {
                self.stopwatch_secs += dt_secs;
            }
        }
    }

    pub fn reset_current(&mut self) {
        match self.mode {
            ClockMode::Clock => {}
            ClockMode::Pomodoro => {
                self.pomo_is_break = false;
                self.pomo_remaining = 25.0 * 60.0;
            }
            ClockMode::Stopwatch => {
                self.stopwatch_secs = 0.0;
            }
        }
    }

    /// Formats the active timer as `"MM:SS"` (or `"HH:MM:SS"` if stopwatch >= 1 hour).
    pub fn formatted_time(&self) -> Option<String> {
        match self.mode {
            ClockMode::Clock => None,
            ClockMode::Pomodoro => {
                let total = self.pomo_remaining.ceil().max(0.0) as u32;
                let m = total / 60;
                let s = total % 60;
                Some(format!("{m:02}:{s:02}"))
            }
            ClockMode::Stopwatch => {
                let total = self.stopwatch_secs.floor().max(0.0) as u32;
                let h = total / 3600;
                let m = (total % 3600) / 60;
                let s = total % 60;
                if h > 0 {
                    Some(format!("{h:02}:{m:02}:{s:02}"))
                } else {
                    Some(format!("{m:02}:{s:02}"))
                }
            }
        }
    }

    pub fn label(&self) -> Option<&'static str> {
        match self.mode {
            ClockMode::Clock => None,
            ClockMode::Pomodoro => Some(if self.pomo_is_break {
                "POMODORO • BREAK"
            } else {
                "POMODORO • FOCUS"
            }),
            ClockMode::Stopwatch => Some("STOPWATCH"),
        }
    }
}

/// Visual font style for the big terminal clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockFont {
    #[default]
    Block,
    Neon,
    Segment,
    Dots,
}

impl ClockFont {
    pub fn next(self) -> Self {
        match self {
            Self::Block => Self::Neon,
            Self::Neon => Self::Segment,
            Self::Segment => Self::Dots,
            Self::Dots => Self::Block,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Block => "Block",
            Self::Neon => "Neon",
            Self::Segment => "Segment",
            Self::Dots => "Dots",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "block" => Some(Self::Block),
            "neon" => Some(Self::Neon),
            "segment" => Some(Self::Segment),
            "dots" => Some(Self::Dots),
            _ => None,
        }
    }
}

fn glyph_block(ch: char) -> [&'static str; DIGIT_HEIGHT] {
    match ch {
        '0' => ["█████", "█   █", "█   █", "█   █", "█████"],
        '1' => ["  ██ ", "   █ ", "   █ ", "   █ ", "█████"],
        '2' => ["█████", "    █", "█████", "█    ", "█████"],
        '3' => ["█████", "    █", "█████", "    █", "█████"],
        '4' => ["█   █", "█   █", "█████", "    █", "    █"],
        '5' => ["█████", "█    ", "█████", "    █", "█████"],
        '6' => ["█████", "█    ", "█████", "█   █", "█████"],
        '7' => ["█████", "    █", "   █ ", "  █  ", "  █  "],
        '8' => ["█████", "█   █", "█████", "█   █", "█████"],
        '9' => ["█████", "█   █", "█████", "    █", "█████"],
        ':' => ["  ", "██", "  ", "██", "  "],
        _ => ["  ", "  ", "  ", "  ", "  "],
    }
}

fn glyph_neon(ch: char) -> [&'static str; DIGIT_HEIGHT] {
    match ch {
        '0' => ["╔═══╗", "║   ║", "║   ║", "║   ║", "╚═══╝"],
        '1' => ["  ═╗ ", "   ║ ", "   ║ ", "   ║ ", "═══╩═"],
        '2' => ["╔═══╗", "    ║", "╔═══╝", "║    ", "╚═══╝"],
        '3' => ["╔═══╗", "    ║", " ═══╣", "    ║", "╚═══╝"],
        '4' => ["╗   ╗", "║   ║", "╚═══╣", "    ║", "    ╩"],
        '5' => ["╔═══╗", "║    ", "╚═══╗", "    ║", "╚═══╝"],
        '6' => ["╔═══╗", "║    ", "╠═══╗", "║   ║", "╚═══╝"],
        '7' => ["╔═══╗", "    ║", "    ║", "    ║", "    ╩"],
        '8' => ["╔═══╗", "║   ║", "╠═══╣", "║   ║", "╚═══╝"],
        '9' => ["╔═══╗", "║   ║", "╚═══╣", "    ║", "╚═══╝"],
        ':' => ["  ", "◈ ", "  ", "◈ ", "  "],
        _ => ["  ", "  ", "  ", "  ", "  "],
    }
}

fn glyph_segment(ch: char) -> [&'static str; DIGIT_HEIGHT] {
    match ch {
        '0' => ["┌───┐", "│   │", "│   │", "│   │", "└───┘"],
        '1' => ["  ─┐ ", "   │ ", "   │ ", "   │ ", "───┴─"],
        '2' => ["┌───┐", "    │", "┌───┘", "│    ", "└───┘"],
        '3' => ["┌───┐", "    │", " ───┤", "    │", "└───┘"],
        '4' => ["┐   ┐", "│   │", "└───┤", "    │", "    ┴"],
        '5' => ["┌───┐", "│    ", "└───┐", "    │", "└───┘"],
        '6' => ["┌───┐", "│    ", "├───┐", "│   │", "└───┘"],
        '7' => ["┌───┐", "    │", "    │", "    │", "    ┴"],
        '8' => ["┌───┐", "│   │", "├───┤", "│   │", "└───┘"],
        '9' => ["┌───┐", "│   │", "└───┤", "    │", "└───┘"],
        ':' => ["  ", "• ", "  ", "• ", "  "],
        _ => ["  ", "  ", "  ", "  ", "  "],
    }
}

fn glyph_dots(ch: char) -> [&'static str; DIGIT_HEIGHT] {
    match ch {
        '0' => ["•●●●•", "●   ●", "●   ●", "●   ●", "•●●●•"],
        '1' => ["  ●● ", "   ● ", "   ● ", "   ● ", "●●●●●"],
        '2' => ["●●●●•", "    ●", "•●●●•", "●    ", "●●●●●"],
        '3' => ["●●●●•", "    ●", " •●●●", "    ●", "●●●●•"],
        '4' => ["●   ●", "●   ●", "●●●●●", "    ●", "    ●"],
        '5' => ["●●●●●", "●    ", "●●●●•", "    ●", "●●●●•"],
        '6' => ["•●●●•", "●    ", "●●●●•", "●   ●", "•●●●•"],
        '7' => ["●●●●●", "    ●", "   ● ", "  ●  ", "  ●  "],
        '8' => ["•●●●•", "●   ●", "•●●●•", "●   ●", "•●●●•"],
        '9' => ["•●●●•", "●   ●", "•●●●●", "    ●", "•●●●•"],
        ':' => ["  ", "● ", "  ", "● ", "  "],
        _ => ["  ", "  ", "  ", "  ", "  "],
    }
}

/// Renders a time string (e.g. `"21:09:38"` or `"21:09"`) into 5 lines of Unicode art.
pub fn render_big_time(time_str: &str, font: ClockFont) -> [String; DIGIT_HEIGHT] {
    let mut lines: [String; DIGIT_HEIGHT] = std::array::from_fn(|_| String::new());
    let chars: Vec<char> = time_str.chars().collect();

    for (idx, &ch) in chars.iter().enumerate() {
        let rows = match font {
            ClockFont::Block => glyph_block(ch),
            ClockFont::Neon => glyph_neon(ch),
            ClockFont::Segment => glyph_segment(ch),
            ClockFont::Dots => glyph_dots(ch),
        };
        for (row_idx, row_str) in rows.iter().enumerate() {
            if idx > 0 {
                lines[row_idx].push(' ');
            }
            lines[row_idx].push_str(row_str);
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fonts_have_five_uniform_rows() {
        for font in [
            ClockFont::Block,
            ClockFont::Neon,
            ClockFont::Segment,
            ClockFont::Dots,
        ] {
            let lines = render_big_time("21:09:38", font);
            assert_eq!(lines.len(), 5);
            let expected_len = lines[0].chars().count();
            assert!(expected_len > 0);
            for line in &lines {
                assert_eq!(line.chars().count(), expected_len);
            }
        }
    }

    #[test]
    fn clock_font_cycles_all_variants() {
        let start = ClockFont::Block;
        let mut cur = start;
        for _ in 0..4 {
            cur = cur.next();
        }
        assert_eq!(cur, start);
    }
}
