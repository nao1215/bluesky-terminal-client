//! Color themes.
//!
//! The themes and their styles are terminal-kit's, as bsky's are; this module
//! lists every theme the picker (`T`) offers, in bsky's order, and adds the
//! styles only msky draws.
//!
//! `terminal`, msky's default, uses the sixteen ANSI colors, so it follows
//! the terminal's own palette; `bluesky` draws in the colors of Bluesky's own
//! app; `monochrome` uses no color at all and marks emphasis with bold and
//! reverse video. The others are 24-bit palettes. On a terminal that does not
//! announce 24-bit color they are mapped to the nearest xterm-256 colors,
//! because sending 24-bit escapes there garbles the screen; with `NO_COLOR`
//! set, msky uses `monochrome` whatever is chosen.

use ratatui::style::{Modifier, Style};

pub(crate) use terminal_kit::theme::{ColorDepth, THEMES, Theme, color_depth};

/// Names themes had before, still read from `settings.json`.
const OLD_NAMES: [(&str, &str); 3] = [
    ("default", "terminal"),
    ("light", "github-light"),
    ("catppuccin", "catppuccin-mocha"),
];

/// The theme msky starts in without a `theme` setting.
pub(crate) const DEFAULT: &str = "terminal";

/// The theme `name` (else the default) as a terminal of `depth` shows it.
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
    let name = OLD_NAMES
        .iter()
        .find(|(old, _)| old.eq_ignore_ascii_case(name))
        .map_or(name, |(_, new)| new);
    THEMES
        .iter()
        .position(|t| t.name.eq_ignore_ascii_case(name))
}

/// Index of `monochrome`, which `NO_COLOR` forces: the last of [`THEMES`].
pub(crate) fn monochrome() -> usize {
    THEMES.len() - 1
}

/// The styles msky draws that bsky does not.
pub(crate) trait MisskeyStyles {
    /// A content warning: the like color, bold, so it stands out in every
    /// theme and without color.
    fn warn(&self) -> Style;

    /// A renote: the repost color.
    fn renote(&self) -> Style;
}

impl MisskeyStyles for Theme {
    fn warn(&self) -> Style {
        self.like().add_modifier(Modifier::BOLD)
    }

    fn renote(&self) -> Style {
        self.repost()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use terminal_kit::theme::{EDITOR_THEMES, MONOCHROME, TERMINAL};

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
        assert_eq!(THEMES[monochrome()], MONOCHROME);
        assert_eq!(THEMES[monochrome()].name, "monochrome");
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
    fn bluesky_comes_first_and_terminal_follows_the_palette() {
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
    fn a_warning_is_the_like_color_in_bold_and_a_renote_the_repost_color() {
        for t in [THEMES[0], TERMINAL, MONOCHROME] {
            assert_eq!(t.warn(), t.like().add_modifier(Modifier::BOLD));
            assert!(t.warn().add_modifier.contains(Modifier::BOLD));
            assert_eq!(t.renote(), t.repost());
        }
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
        for t in &THEMES[..3] {
            let bg = lum(t.bg).unwrap();
            assert!(contrast(bg, lum(t.fg).unwrap()) >= 4.5, "{}: text", t.name);
            for (role, c) in [("accent", t.accent), ("like", t.like), ("error", t.error)] {
                assert!(contrast(bg, lum(c).unwrap()) >= 2.0, "{}: {role}", t.name);
            }
        }
    }
}
