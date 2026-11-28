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
pub(crate) use terminal_kit::theme::{ColorDepth, THEMES, Theme, chosen, color_depth};

/// A content warning: the like color, bold, so it stands out in every
/// theme and without color, as msky draws one.
pub(crate) fn warn(t: &Theme) -> Style {
    t.like().add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_kit::theme::{MONOCHROME, TERMINAL};

    #[test]
    fn a_warning_is_the_like_color_in_bold() {
        for t in [THEMES[0], TERMINAL, MONOCHROME] {
            assert_eq!(warn(&t), t.like().add_modifier(Modifier::BOLD));
        }
    }
}
