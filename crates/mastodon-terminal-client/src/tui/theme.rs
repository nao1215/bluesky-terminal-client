//! Color themes.
//!
//! The themes and their styles are terminal-kit's, as bsky's and msky's
//! are; this module lists every theme the picker (`T`) offers, in their
//! order, and adds the style only mstdn draws.
//!
//! `terminal`, the default, uses the sixteen ANSI colors, so it follows the
//! terminal's own palette; `monochrome` uses no color at all and marks
//! emphasis with bold and reverse video. The others are 24-bit palettes. On
//! a terminal that does not announce 24-bit color they are mapped to the
//! nearest xterm-256 colors; with `NO_COLOR` set, mstdn uses `monochrome`
//! whatever is chosen.

use ratatui::style::{Modifier, Style};
use terminal_kit::theme::{EDITOR_THEMES, MONOCHROME, TERMINAL, palette};

pub(crate) use terminal_kit::theme::{ColorDepth, Theme, color_depth};

/// Bluesky's own looks, as bsky and msky offer them.
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

/// The built-in themes, in the order bsky's and msky's pickers list them:
/// Bluesky's looks, the terminal's palette (the default), the editor and
/// terminal themes by name, and `monochrome` last.
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

/// The theme mstdn starts in without a `theme` setting.
pub(crate) const DEFAULT: &str = "terminal";

/// The theme `name` (else the default) as a terminal of `depth` shows it,
/// and its place in [`THEMES`].
pub(crate) fn chosen(name: Option<&str>, depth: ColorDepth) -> (usize, Theme) {
    let i = name
        .and_then(index_of)
        .or_else(|| index_of(DEFAULT))
        .unwrap_or(0);
    (i, THEMES[i].for_depth(depth))
}

/// Index of the theme named `name`, ignoring case.
pub(crate) fn index_of(name: &str) -> Option<usize> {
    let name = name.trim();
    THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
}

/// A content warning: the like color, bold, so it stands out in every
/// theme and without color, as msky draws one.
pub(crate) fn warn(t: &Theme) -> Style {
    t.like().add_modifier(Modifier::BOLD)
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

    // The same 42 as bsky's and msky's, in their order.
    #[test]
    fn the_picker_lists_bluesky_then_terminal_then_the_editors_then_monochrome() {
        assert_eq!(THEMES.len(), 42);
        assert_eq!(THEMES[3], TERMINAL);
        assert_eq!(THEMES[4..41], EDITOR_THEMES);
        assert_eq!(THEMES[41], MONOCHROME);
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
        assert_eq!(chosen(Some("nord"), ColorDepth::None).1, MONOCHROME);
    }

    #[test]
    fn the_default_is_terminal_and_an_unknown_name_falls_back_to_it() {
        let (i, t) = chosen(None, ColorDepth::TrueColor);
        assert_eq!(THEMES[i].name, "terminal");
        assert_eq!(t, TERMINAL);
        assert_eq!(chosen(Some("no-such-theme"), ColorDepth::TrueColor).0, i);
        let (i, _) = chosen(Some("Dracula"), ColorDepth::TrueColor);
        assert_eq!(THEMES[i].name, "dracula");
    }

    #[test]
    fn a_warning_is_the_like_color_in_bold() {
        for t in [THEMES[0], TERMINAL, MONOCHROME] {
            assert_eq!(warn(&t), t.like().add_modifier(Modifier::BOLD));
        }
    }
}
