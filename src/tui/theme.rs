//! Color themes.
//!
//! A theme names a color for each role the UI draws (text, dim text, accent,
//! errors, likes, ...), and every drawing function asks the theme for its
//! styles instead of naming colors itself.
//!
//! `bluesky`, the default, draws in the colors of Bluesky's own app.
//! `terminal` uses the sixteen ANSI colors, so it follows the terminal's own
//! palette; `monochrome` uses no color at all and marks emphasis with bold
//! and reverse video. The others are 24-bit palettes. On a terminal that does
//! not announce 24-bit color they are mapped to the nearest xterm-256 colors,
//! because sending 24-bit escapes there garbles the screen; with `NO_COLOR`
//! set, bsky uses `monochrome` whatever is chosen.

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

/// The built-in themes, in the order the picker lists them: Bluesky's own
/// looks first (`bluesky`, its "dim" look, is the default), then the
/// terminal's palette, then well-known editor and terminal themes by name,
/// and `monochrome` last.
pub static THEMES: [Theme; 42] = [
    palette(
        "bluesky",
        [
            0x161e27, 0xf1f3f5, 0x8b98a5, 0x1185fe, 0xffffff, 0xff5c5c, 0x20bc07, 0xec4899,
            0x20bc07,
        ],
    ),
    palette(
        "bluesky-dark",
        [
            0x000000, 0xffffff, 0x8b98a5, 0x1185fe, 0xffffff, 0xff5c5c, 0x20bc07, 0xec4899,
            0x20bc07,
        ],
    ),
    palette(
        "bluesky-light",
        [
            0xffffff, 0x0b0f14, 0x6e7d8c, 0x1185fe, 0xffffff, 0xe0245e, 0x13a60b, 0xec4899,
            0x13a60b,
        ],
    ),
    Theme {
        name: "terminal",
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
        "ayu-dark",
        [
            0x0b0e14, 0xbfbdb6, 0x565b66, 0xe6b450, 0x0b0e14, 0xd95757, 0x7fd962, 0xf07178,
            0x95e6cb,
        ],
    ),
    palette(
        "ayu-light",
        [
            0xfcfcfc, 0x5c6166, 0x8a9199, 0xff9940, 0xfcfcfc, 0xe65050, 0x86b300, 0xf07171,
            0x4cbf99,
        ],
    ),
    palette(
        "catppuccin-frappe",
        [
            0x303446, 0xc6d0f5, 0x838ba7, 0xca9ee6, 0x303446, 0xe78284, 0xa6d189, 0xf4b8e4,
            0x81c8be,
        ],
    ),
    palette(
        "catppuccin-latte",
        [
            0xeff1f5, 0x4c4f69, 0x8c8fa1, 0x8839ef, 0xeff1f5, 0xd20f39, 0x40a02b, 0xea76cb,
            0x179299,
        ],
    ),
    palette(
        "catppuccin-macchiato",
        [
            0x24273a, 0xcad3f5, 0x8087a2, 0xc6a0f6, 0x24273a, 0xed8796, 0xa6da95, 0xf5bde6,
            0x8bd5ca,
        ],
    ),
    palette(
        "catppuccin-mocha",
        [
            0x1e1e2e, 0xcdd6f4, 0x7f849c, 0xcba6f7, 0x1e1e2e, 0xf38ba8, 0xa6e3a1, 0xf5c2e7,
            0x94e2d5,
        ],
    ),
    palette(
        "cobalt2",
        [
            0x193549, 0xffffff, 0x8aa4b8, 0xffc600, 0x193549, 0xff628c, 0x3ad900, 0xff9d00,
            0x2affdf,
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
        "everforest",
        [
            0x2d353b, 0xd3c6aa, 0x859289, 0xa7c080, 0x2d353b, 0xe67e80, 0xa7c080, 0xd699b6,
            0x83c092,
        ],
    ),
    palette(
        "github-dark",
        [
            0x0d1117, 0xe6edf3, 0x7d8590, 0x2f81f7, 0xffffff, 0xf85149, 0x3fb950, 0xdb61a2,
            0x3fb950,
        ],
    ),
    palette(
        "github-light",
        [
            0xffffff, 0x1f2328, 0x656d76, 0x0969da, 0xffffff, 0xcf222e, 0x1a7f37, 0xbf3989,
            0x1a7f37,
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
        "gruvbox-light",
        [
            0xfbf1c7, 0x3c3836, 0x928374, 0xb57614, 0xfbf1c7, 0x9d0006, 0x79740e, 0x8f3f71,
            0x427b58,
        ],
    ),
    palette(
        "horizon",
        [
            0x1c1e26, 0xd5d8da, 0x6c6f93, 0x26bbd9, 0x1c1e26, 0xe95678, 0x29d398, 0xee64ac,
            0x59e3e3,
        ],
    ),
    palette(
        "iceberg",
        [
            0x161821, 0xc6c8d1, 0x6b7089, 0x84a0c6, 0x161821, 0xe27878, 0xb4be82, 0xa093c7,
            0x89b8c2,
        ],
    ),
    palette(
        "kanagawa",
        [
            0x1f1f28, 0xdcd7ba, 0x727169, 0x7e9cd8, 0x1f1f28, 0xe82424, 0x98bb6c, 0xd27e99,
            0x7aa89f,
        ],
    ),
    palette(
        "material",
        [
            0x263238, 0xeeffff, 0x546e7a, 0x82aaff, 0x263238, 0xf07178, 0xc3e88d, 0xc792ea,
            0x89ddff,
        ],
    ),
    palette(
        "monokai",
        [
            0x272822, 0xf8f8f2, 0x75715e, 0x66d9ef, 0x272822, 0xf92672, 0xa6e22e, 0xae81ff,
            0xa6e22e,
        ],
    ),
    palette(
        "night-owl",
        [
            0x011627, 0xd6deeb, 0x637777, 0x82aaff, 0x011627, 0xef5350, 0x22da6e, 0xc792ea,
            0x7fdbca,
        ],
    ),
    palette(
        "nightfox",
        [
            0x192330, 0xcdcecf, 0x738091, 0x719cd6, 0x192330, 0xc94f6d, 0x81b29a, 0x9d79d6,
            0x63cdcf,
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
        "oceanic-next",
        [
            0x1b2b34, 0xd8dee9, 0x65737e, 0x6699cc, 0x1b2b34, 0xec5f67, 0x99c794, 0xc594c5,
            0x5fb3b3,
        ],
    ),
    palette(
        "one-dark",
        [
            0x282c34, 0xabb2bf, 0x5c6370, 0x61afef, 0x282c34, 0xe06c75, 0x98c379, 0xc678dd,
            0x56b6c2,
        ],
    ),
    palette(
        "one-light",
        [
            0xfafafa, 0x383a42, 0xa0a1a7, 0x4078f2, 0xfafafa, 0xe45649, 0x50a14f, 0xa626a4,
            0x0184bc,
        ],
    ),
    palette(
        "palenight",
        [
            0x292d3e, 0xa6accd, 0x676e95, 0x82aaff, 0x292d3e, 0xff5370, 0xc3e88d, 0xc792ea,
            0x89ddff,
        ],
    ),
    palette(
        "papercolor-light",
        [
            0xeeeeee, 0x444444, 0x878787, 0x005f87, 0xeeeeee, 0xaf0000, 0x008700, 0xd7005f,
            0x0087af,
        ],
    ),
    palette(
        "rose-pine",
        [
            0x191724, 0xe0def4, 0x6e6a86, 0xc4a7e7, 0x191724, 0xeb6f92, 0x9ccfd8, 0xebbcba,
            0x9ccfd8,
        ],
    ),
    palette(
        "rose-pine-dawn",
        [
            0xfaf4ed, 0x575279, 0x9893a5, 0x907aa9, 0xfaf4ed, 0xb4637a, 0x286983, 0xd7827e,
            0x56949f,
        ],
    ),
    palette(
        "rose-pine-moon",
        [
            0x232136, 0xe0def4, 0x6e6a86, 0xc4a7e7, 0x232136, 0xeb6f92, 0x9ccfd8, 0xea9a97,
            0x9ccfd8,
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
        "solarized-light",
        [
            0xfdf6e3, 0x586e75, 0x93a1a1, 0x268bd2, 0xfdf6e3, 0xdc322f, 0x859900, 0xd33682,
            0x2aa198,
        ],
    ),
    palette(
        "synthwave-84",
        [
            0x262335, 0xffffff, 0x848bbd, 0xff7edb, 0x262335, 0xfe4450, 0x72f1b8, 0xff7edb,
            0x36f9f6,
        ],
    ),
    palette(
        "tokyo-night",
        [
            0x1a1b26, 0xc0caf5, 0x737aa2, 0x7aa2f7, 0x1a1b26, 0xf7768e, 0x9ece6a, 0xbb9af7,
            0x73daca,
        ],
    ),
    palette(
        "tokyo-night-day",
        [
            0xe1e2e7, 0x3760bf, 0x848cb5, 0x2e7de9, 0xe1e2e7, 0xf52a65, 0x587539, 0x9854f1,
            0x118c74,
        ],
    ),
    palette(
        "tokyo-night-storm",
        [
            0x24283b, 0xc0caf5, 0x737aa2, 0x7aa2f7, 0x24283b, 0xf7768e, 0x9ece6a, 0xbb9af7,
            0x73daca,
        ],
    ),
    palette(
        "tomorrow-night",
        [
            0x1d1f21, 0xc5c8c6, 0x969896, 0x81a2be, 0x1d1f21, 0xcc6666, 0xb5bd68, 0xb294bb,
            0x8abeb7,
        ],
    ),
    palette(
        "zenburn",
        [
            0x3f3f3f, 0xdcdccc, 0x7f9f7f, 0x8cd0d3, 0x3f3f3f, 0xcc9393, 0x9fc59f, 0xdc8cc3,
            0x93e0e3,
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

/// Names themes had before, still read from `settings.json`.
const OLD_NAMES: [(&str, &str); 3] = [
    ("default", "terminal"),
    ("light", "github-light"),
    ("catppuccin", "catppuccin-mocha"),
];

/// Index of the theme named `name`, ignoring case.
pub fn index_of(name: &str) -> Option<usize> {
    let name = name.trim();
    let name = OLD_NAMES
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(name))
        .map_or(name, |(_, new)| new);
    THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
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
    // The ramp is 8, 18, ..., 238: the nearest step, rounded.
    let gray_i = (avg.saturating_sub(3) / 10).min(23) as u8;
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
    fn the_default_is_bluesky_and_terminal_follows_the_palette() {
        assert_eq!(THEMES[0].name, "bluesky");
        assert_eq!(THEMES[0].accent, rgb(0x1185fe), "Bluesky's blue");
        let t = THEMES[index_of("terminal").unwrap()];
        for c in [t.bg, t.fg, t.dim, t.accent, t.error, t.like] {
            assert!(!matches!(c, Color::Rgb(..) | Color::Indexed(_)), "{c:?}");
        }
    }

    #[test]
    fn old_theme_names_still_load() {
        assert_eq!(THEMES[index_of("default").unwrap()].name, "terminal");
        assert_eq!(THEMES[index_of("Light").unwrap()].name, "github-light");
        assert_eq!(
            THEMES[index_of("catppuccin").unwrap()].name,
            "catppuccin-mocha"
        );
    }

    #[test]
    fn every_theme_keeps_its_text_readable() {
        // Rough WCAG luminance contrast: text and accents against the
        // background, so no theme hides what it draws.
        let lum = |c: Color| {
            let Color::Rgb(r, g, b) = c else { return None };
            let ch = |v: u8| {
                let v = f64::from(v) / 255.0;
                if v <= 0.039_28 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            Some(0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b))
        };
        let contrast = |a: f64, b: f64| (a.max(b) + 0.05) / (a.min(b) + 0.05);
        for t in THEMES.iter().filter(|t| matches!(t.bg, Color::Rgb(..))) {
            let bg = lum(t.bg).unwrap();
            assert!(contrast(bg, lum(t.fg).unwrap()) >= 4.5, "{}: text", t.name);
            for (role, c) in [("accent", t.accent), ("like", t.like), ("error", t.error)] {
                assert!(contrast(bg, lum(c).unwrap()) >= 2.0, "{}: {role}", t.name);
            }
        }
    }

    #[rstest]
    #[case(Color::Rgb(0, 0, 0), Color::Indexed(16))]
    #[case(Color::Rgb(255, 255, 255), Color::Indexed(231))]
    #[case(Color::Rgb(255, 0, 0), Color::Indexed(196))]
    #[case(Color::Rgb(0x28, 0x28, 0x28), Color::Indexed(235))]
    #[case(Color::Rgb(17, 17, 17), Color::Indexed(233))]
    #[case(Color::Cyan, Color::Cyan)]
    fn rgb_maps_to_the_nearest_xterm_color(#[case] c: Color, #[case] want: Color) {
        assert_eq!(to_256(c), want);
    }
}
