//! The colors the client draws with.
//!
//! Every drawing function asks the theme for its styles instead of naming
//! colors itself, as bsky's do. msky has bsky's `terminal` theme, which
//! uses the sixteen ANSI colors and so follows the terminal's own palette,
//! and its `monochrome` one, which `NO_COLOR` turns on: no color at all,
//! emphasis by bold and reverse video. bsky's theme picker (`T`) is for a
//! later version.

use ratatui::style::{Color, Modifier, Style};

/// Colors for every role the UI draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// Screen background and ordinary text.
    pub bg: Color,
    pub fg: Color,
    /// Secondary text: names after the display name, times, hints.
    pub dim: Color,
    /// Selection, focus, links, borders.
    pub accent: Color,
    pub error: Color,
    pub ok: Color,
    /// A content warning.
    pub warn: Color,
    /// A renote.
    pub renote: Color,
    /// No color at all: emphasis comes from bold and reverse video.
    pub mono: bool,
}

/// bsky's `terminal` theme.
pub const TERMINAL: Theme = Theme {
    name: "terminal",
    bg: Color::Reset,
    fg: Color::Reset,
    dim: Color::DarkGray,
    accent: Color::Cyan,
    error: Color::Red,
    ok: Color::Green,
    warn: Color::Yellow,
    renote: Color::Green,
    mono: false,
};

/// bsky's `monochrome` theme.
pub const MONOCHROME: Theme = Theme {
    name: "monochrome",
    bg: Color::Reset,
    fg: Color::Reset,
    dim: Color::Reset,
    accent: Color::Reset,
    error: Color::Reset,
    ok: Color::Reset,
    warn: Color::Reset,
    renote: Color::Reset,
    mono: true,
};

/// The theme for the environment `var` reads: monochrome when `NO_COLOR`
/// is set to anything but the empty string, as bsky and
/// <https://no-color.org> have it.
pub fn for_env(var: impl Fn(&str) -> Option<String>) -> Theme {
    if var("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        MONOCHROME
    } else {
        TERMINAL
    }
}

impl Theme {
    /// Ordinary text on the screen background.
    pub fn base(&self) -> Style {
        Style::new().fg(self.fg).bg(self.bg)
    }

    /// Secondary text.
    pub fn dim(&self) -> Style {
        let s = Style::new().fg(self.dim);
        if self.mono {
            s.add_modifier(Modifier::DIM)
        } else {
            s
        }
    }

    /// Accent text: focus, selection marker, links, keys in the hints.
    pub fn accent(&self) -> Style {
        let s = Style::new().fg(self.accent);
        if self.mono {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }

    /// The active tab or a focused prompt.
    pub fn selected(&self) -> Style {
        Style::new()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn ok(&self) -> Style {
        Style::new().fg(self.ok)
    }

    /// A content warning, which stands out without color too.
    pub fn warn(&self) -> Style {
        Style::new().fg(self.warn).add_modifier(Modifier::BOLD)
    }

    pub fn renote(&self) -> Style {
        let s = Style::new().fg(self.renote);
        if self.mono {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_set_to_anything_turns_color_off() {
        assert_eq!(for_env(|_| None), TERMINAL);
        assert_eq!(
            for_env(|k| (k == "NO_COLOR").then(|| "1".into())),
            MONOCHROME
        );
        assert_eq!(for_env(|k| (k == "NO_COLOR").then(String::new)), TERMINAL);
    }
}
