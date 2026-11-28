//! Color themes.
//!
//! The themes and their styles are terminal-kit's; this module adds
//! Bluesky's own looks and lists every theme the picker offers.
//!
//! `bluesky`, the default, draws in the colors of Bluesky's own app.
//! `terminal` uses the sixteen ANSI colors, so it follows the terminal's own
//! palette; `monochrome` uses no color at all and marks emphasis with bold
//! and reverse video. The others are 24-bit palettes. On a terminal that does
//! not announce 24-bit color they are mapped to the nearest xterm-256 colors,
//! because sending 24-bit escapes there garbles the screen; with `NO_COLOR`
//! set, bsky uses `monochrome` whatever is chosen.

use terminal_kit::theme::{EDITOR_THEMES, MONOCHROME, TERMINAL, palette};

pub(crate) use terminal_kit::theme::{ColorDepth, Theme, color_depth};

/// Bluesky's own looks: `bluesky` is its "dim" look.
const BLUESKY: [Theme; 3] = [
    palette(
        "bluesky",
        [
            0x161e27, 0xf1f3f5, 0x8b98a5, 0x1185fe, 0xff5c5c, 0x20bc07, 0xec4899, 0x20bc07,
        ],
    ),
    palette(
        "bluesky-dark",
        [
            0x000000, 0xffffff, 0x8b98a5, 0x1185fe, 0xff5c5c, 0x20bc07, 0xec4899, 0x20bc07,
        ],
    ),
    palette(
        "bluesky-light",
        [
            0xffffff, 0x0b0f14, 0x6e7d8c, 0x1185fe, 0xe0245e, 0x13a60b, 0xec4899, 0x13a60b,
        ],
    ),
];

/// The built-in themes, in the order the picker lists them: Bluesky's own
/// looks first (`bluesky`, its "dim" look, is the default), then the
/// terminal's palette, then well-known editor and terminal themes by name,
/// and `monochrome` last.
pub(crate) static THEMES: [Theme; BLUESKY.len() + 1 + EDITOR_THEMES.len() + 1] = {
    let mut all = [MONOCHROME; BLUESKY.len() + 1 + EDITOR_THEMES.len() + 1];
    let mut i = 0;
    while i < BLUESKY.len() {
        all[i] = BLUESKY[i];
        i += 1;
    }
    all[BLUESKY.len()] = TERMINAL;
    let mut j = 0;
    while j < EDITOR_THEMES.len() {
        all[BLUESKY.len() + 1 + j] = EDITOR_THEMES[j];
        j += 1;
    }
    all
};

/// Names themes had before, still read from `settings.json`.
const OLD_NAMES: [(&str, &str); 3] = [
    ("default", "terminal"),
    ("light", "github-light"),
    ("catppuccin", "catppuccin-mocha"),
];

/// Index of the theme named `name`, ignoring case.
pub(crate) fn index_of(name: &str) -> Option<usize> {
    let name = name.trim();
    let name = OLD_NAMES
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(name))
        .map_or(name, |(_, new)| new);
    THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn theme_names_are_unique_and_found_ignoring_case() {
        for (i, t) in THEMES.iter().enumerate() {
            assert_eq!(index_of(t.name), Some(i));
            assert_eq!(index_of(&t.name.to_uppercase()), Some(i));
        }
        assert_eq!(index_of("no-such-theme"), None);
    }

    #[test]
    fn the_picker_lists_bluesky_then_terminal_then_the_editors_then_monochrome() {
        assert_eq!(THEMES.len(), 42);
        assert_eq!(THEMES[3], TERMINAL);
        assert_eq!(THEMES[4..41], EDITOR_THEMES);
        assert_eq!(THEMES[THEMES.len() - 1], MONOCHROME);
        assert_eq!(THEMES[THEMES.len() - 1].name, "monochrome");
    }

    #[test]
    fn a_256_color_terminal_gets_no_24_bit_colors() {
        for t in &THEMES {
            let t = t.for_depth(ColorDepth::Ansi256);
            for c in [t.bg, t.fg, t.dim, t.accent, t.error, t.ok, t.like, t.repost] {
                assert!(!matches!(c, Color::Rgb(..)), "{}: {c:?}", t.name);
            }
        }
    }

    #[test]
    fn no_color_means_monochrome_whatever_was_chosen() {
        let t = THEMES[index_of("dracula").unwrap()].for_depth(ColorDepth::None);
        assert!(t.mono);
        assert_eq!(t.name, "monochrome");
        let t = THEMES[0].for_depth(ColorDepth::None);
        assert_eq!(t, THEMES[THEMES.len() - 1]);
    }

    #[test]
    fn the_default_is_bluesky_and_terminal_follows_the_palette() {
        assert_eq!(THEMES[0].name, "bluesky");
        assert_eq!(
            THEMES[0].accent,
            Color::Rgb(0x11, 0x85, 0xfe),
            "Bluesky's blue"
        );
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
    fn bluesky_keeps_its_text_readable() {
        // Rough WCAG luminance contrast, as terminal-kit checks its own.
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
        for t in &BLUESKY {
            let bg = lum(t.bg).unwrap();
            assert!(contrast(bg, lum(t.fg).unwrap()) >= 4.5, "{}: text", t.name);
            for (role, c) in [("accent", t.accent), ("like", t.like), ("error", t.error)] {
                assert!(contrast(bg, lum(c).unwrap()) >= 2.0, "{}: {role}", t.name);
            }
        }
    }
}
