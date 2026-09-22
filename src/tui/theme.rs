//! Color themes.
//!
//! A theme names a color for each role the UI draws (text, dim text, accent,
//! errors, likes, ...), and every drawing function asks the theme for its
//! styles instead of naming colors itself.
//!
//! `default` uses the sixteen ANSI colors, so it follows the terminal's own
//! palette; `monochrome` uses no color at all and marks emphasis with bold
//! and reverse video. The others are 24-bit palettes. On a terminal that does
//! not announce 24-bit color they are mapped to the nearest xterm-256 colors,
//! because sending 24-bit escapes there garbles the screen; with `NO_COLOR`
//! set, bs uses `monochrome` whatever is chosen.

use ratatui::style::{Color, Modifier, Style};

/// Colors for every role the UI draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// Screen background and ordinary text.
    pub bg: Color,
    pub fg: Color,
    /// Secondary text: handles, times, hints.
    pub dim: Color,
    /// Selection, focus, links, borders.
    pub accent: Color,
    /// Text drawn on an accent background.
    pub on_accent: Color,
    pub error: Color,
    pub ok: Color,
    /// A post the viewer liked.
    pub like: Color,
    /// A post the viewer reposted.
    pub repost: Color,
    /// No color at all: emphasis comes from bold and reverse video.
    pub mono: bool,
}

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn palette(
    name: &'static str,
    [bg, fg, dim, accent, on_accent, error, ok, like, repost]: [u32; 9],
) -> Theme {
    Theme {
        name,
        bg: rgb(bg),
        fg: rgb(fg),
        dim: rgb(dim),
        accent: rgb(accent),
        on_accent: rgb(on_accent),
        error: rgb(error),
        ok: rgb(ok),
        like: rgb(like),
        repost: rgb(repost),
        mono: false,
    }
}

/// The built-in themes, in the order the picker lists them.
pub static THEMES: [Theme; 9] = [
    Theme {
        name: "default",
        bg: Color::Reset,
        fg: Color::Reset,
        dim: Color::DarkGray,
        accent: Color::Cyan,
        on_accent: Color::Black,
        error: Color::Red,
        ok: Color::Green,
        like: Color::Red,
        repost: Color::Green,
        mono: false,
    },
    palette(
        "light",
        [
            0xfafafa, 0x1f2328, 0x6e7781, 0x0969da, 0xffffff, 0xcf222e, 0x1a7f37, 0xbf3989,
            0x1a7f37,
        ],
    ),
    palette(
        "dracula",
        [
            0x282a36, 0xf8f8f2, 0x6272a4, 0xbd93f9, 0x282a36, 0xff5555, 0x50fa7b, 0xff79c6,
            0x50fa7b,
        ],
    ),
    palette(
        "nord",
        [
            0x2e3440, 0xeceff4, 0x7b88a1, 0x88c0d0, 0x2e3440, 0xbf616a, 0xa3be8c, 0xb48ead,
            0xa3be8c,
        ],
    ),
    palette(
        "gruvbox",
        [
            0x282828, 0xebdbb2, 0x928374, 0xfabd2f, 0x282828, 0xfb4934, 0xb8bb26, 0xd3869b,
            0x8ec07c,
        ],
    ),
    palette(
        "solarized",
        [
            0x002b36, 0x93a1a1, 0x657b83, 0x268bd2, 0xfdf6e3, 0xdc322f, 0x859900, 0xd33682,
            0x2aa198,
        ],
    ),
    palette(
        "catppuccin",
        [
            0x1e1e2e, 0xcdd6f4, 0x7f849c, 0xcba6f7, 0x1e1e2e, 0xf38ba8, 0xa6e3a1, 0xf5c2e7,
            0x94e2d5,
        ],
    ),
    palette(
        "tokyo-night",
        [
            0x1a1b26, 0xc0caf5, 0x737aa2, 0x7aa2f7, 0x1a1b26, 0xf7768e, 0x9ece6a, 0xbb9af7,
            0x73daca,
        ],
    ),
    Theme {
        name: "monochrome",
        bg: Color::Reset,
        fg: Color::Reset,
        dim: Color::Reset,
        accent: Color::Reset,
        on_accent: Color::Reset,
        error: Color::Reset,
        ok: Color::Reset,
        like: Color::Reset,
        repost: Color::Reset,
        mono: true,
    },
];

/// Index of the theme named `name`, ignoring case.
pub fn index_of(name: &str) -> Option<usize> {
    THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name.trim()))
}

/// Index of `monochrome`, which `NO_COLOR` forces.
pub fn monochrome() -> usize {
    index_of("monochrome").expect("monochrome is built in")
}

/// How many colors the terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// 24-bit color.
    TrueColor,
    /// The xterm-256 palette.
    Ansi256,
    /// `NO_COLOR` is set: no color at all.
    None,
}

/// Decide the color depth from the environment, read through `var`.
pub fn color_depth(var: impl Fn(&str) -> Option<String>) -> ColorDepth {
    let set = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    if set("NO_COLOR") {
        return ColorDepth::None;
    }
    let colorterm = var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    let term = var("TERM").unwrap_or_default();
    if colorterm == "truecolor"
        || colorterm == "24bit"
        || set("WT_SESSION")
        || ["xterm-kitty", "xterm-ghostty", "wezterm"].contains(&term.as_str())
    {
        ColorDepth::TrueColor
    } else {
        ColorDepth::Ansi256
    }
}

impl Theme {
    /// This theme as a terminal of `depth` can show it.
    pub fn for_depth(&self, depth: ColorDepth) -> Theme {
        match depth {
            ColorDepth::TrueColor => *self,
            ColorDepth::Ansi256 => Theme {
                bg: to_256(self.bg),
                fg: to_256(self.fg),
                dim: to_256(self.dim),
                accent: to_256(self.accent),
                on_accent: to_256(self.on_accent),
                error: to_256(self.error),
                ok: to_256(self.ok),
                like: to_256(self.like),
                repost: to_256(self.repost),
                ..*self
            },
            ColorDepth::None => THEMES[monochrome()],
        }
    }

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

    /// The " bs " badge.
    pub fn badge(&self) -> Style {
        let s = Style::new().add_modifier(Modifier::BOLD);
        if self.mono {
            s.add_modifier(Modifier::REVERSED)
        } else {
            s.fg(self.on_accent).bg(self.accent)
        }
    }

    pub fn error(&self) -> Style {
        Style::new().fg(self.error).add_modifier(Modifier::BOLD)
    }

    pub fn ok(&self) -> Style {
        Style::new().fg(self.ok)
    }

    /// A liked heart; bold without color, so it still stands out.
    pub fn like(&self) -> Style {
        let s = Style::new().fg(self.like);
        if self.mono {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }

    pub fn repost(&self) -> Style {
        let s = Style::new().fg(self.repost);
        if self.mono {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    }
}

/// Map a 24-bit color to the nearest xterm-256 color; others pass through.
fn to_256(c: Color) -> Color {
    let Color::Rgb(r, g, b) = c else { return c };
    // The 6x6x6 cube (16..=231) and the gray ramp (232..=255), whichever is
    // closer, as xterm lays them out.
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| {
        LEVELS
            .iter()
            .enumerate()
            .min_by_key(|(_, l)| (i32::from(**l) - i32::from(v)).abs())
            .map(|(i, _)| i as u8)
            .unwrap()
    };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = (
        LEVELS[ri as usize],
        LEVELS[gi as usize],
        LEVELS[bi as usize],
    );
    let avg = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let gray_i = if avg < 8 {
        0
    } else {
        ((avg - 8) / 10).min(23) as u8
    };
    let gray = 8 + 10 * gray_i;
    let dist = |(x, y, z): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(x, r) + d(y, g) + d(z, b)
    };
    if dist((gray, gray, gray)) < dist(cube) {
        Color::Indexed(232 + gray_i)
    } else {
        Color::Indexed(16 + 36 * ri + 6 * gi + bi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn theme_names_are_unique_and_found_ignoring_case() {
        for (i, t) in THEMES.iter().enumerate() {
            assert_eq!(index_of(t.name), Some(i));
            assert_eq!(index_of(&t.name.to_uppercase()), Some(i));
        }
        assert_eq!(index_of("no-such-theme"), None);
    }

    #[rstest]
    #[case(&[("COLORTERM", "truecolor")], ColorDepth::TrueColor)]
    #[case(&[("COLORTERM", "24bit")], ColorDepth::TrueColor)]
    #[case(&[("WT_SESSION", "abc")], ColorDepth::TrueColor)]
    #[case(&[("TERM", "xterm-kitty")], ColorDepth::TrueColor)]
    #[case(&[("TERM", "xterm-256color")], ColorDepth::Ansi256)]
    #[case(&[], ColorDepth::Ansi256)]
    #[case(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")], ColorDepth::None)]
    #[case(&[("NO_COLOR", "")], ColorDepth::Ansi256)]
    fn color_depth_follows_the_environment(
        #[case] vars: &[(&str, &str)],
        #[case] want: ColorDepth,
    ) {
        assert_eq!(color_depth(env(vars)), want);
    }

    #[test]
    fn a_256_color_terminal_gets_no_24_bit_colors() {
        for t in &THEMES {
            let t = t.for_depth(ColorDepth::Ansi256);
            for c in [
                t.bg,
                t.fg,
                t.dim,
                t.accent,
                t.on_accent,
                t.error,
                t.ok,
                t.like,
                t.repost,
            ] {
                assert!(!matches!(c, Color::Rgb(..)), "{}: {c:?}", t.name);
            }
        }
    }

    #[test]
    fn no_color_means_monochrome_whatever_was_chosen() {
        let t = THEMES[index_of("dracula").unwrap()].for_depth(ColorDepth::None);
        assert!(t.mono);
        assert_eq!(t.name, "monochrome");
    }

    #[test]
    fn monochrome_uses_no_color_but_keeps_emphasis() {
        let t = THEMES[monochrome()];
        for c in [t.bg, t.fg, t.dim, t.accent, t.error, t.like] {
            assert_eq!(c, Color::Reset);
        }
        assert!(t.accent().add_modifier.contains(Modifier::BOLD));
        assert!(t.like().add_modifier.contains(Modifier::BOLD));
        assert!(t.selected().add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn the_default_theme_follows_the_terminal_palette() {
        let t = THEMES[0];
        assert_eq!(t.name, "default");
        for c in [t.bg, t.fg, t.dim, t.accent, t.error, t.like] {
            assert!(!matches!(c, Color::Rgb(..) | Color::Indexed(_)), "{c:?}");
        }
    }

    #[rstest]
    #[case(Color::Rgb(0, 0, 0), Color::Indexed(16))]
    #[case(Color::Rgb(255, 255, 255), Color::Indexed(231))]
    #[case(Color::Rgb(255, 0, 0), Color::Indexed(196))]
    #[case(Color::Rgb(0x28, 0x28, 0x28), Color::Indexed(235))]
    #[case(Color::Cyan, Color::Cyan)]
    fn rgb_maps_to_the_nearest_xterm_color(#[case] c: Color, #[case] want: Color) {
        assert_eq!(to_256(c), want);
    }
}
