//! Terminal checks that run before the UI starts.
//!
//! bsky draws images with the terminal's own graphics protocol. It asks the
//! terminal which protocols it speaks (kitty graphics, sixel, or iTerm2
//! inline images); when the answer is none of them, the client runs as text
//! and says so, rather than drawing pictures the terminal cannot show. The
//! checks themselves are terminal-kit's; this module words their failures
//! for bsky and reads `BSKY_GRAPHICS`.

use ratatui_image::picker::{Picker, ProtocolType};

use crate::error::{Error, Kind, Result};

pub(crate) use terminal_kit::terminal::protocol_name;

/// Environment variable that forces a graphics protocol, for terminals that
/// support one but do not answer the detection query (some multiplexers).
pub(crate) const GRAPHICS_ENV: &str = "BSKY_GRAPHICS";

/// Fail unless stdin and stdout are both terminals.
pub(crate) fn ensure_interactive() -> Result<()> {
    if terminal_kit::terminal::is_interactive() {
        Ok(())
    } else {
        Err(
            Error::new(Kind::Terminal, "bsky needs an interactive terminal")
                .with_hint("run bsky directly in a terminal, not through a pipe or redirect"),
        )
    }
}

/// Parse a `BSKY_GRAPHICS` value.
fn parse_protocol(value: &str) -> Result<ProtocolType> {
    terminal_kit::terminal::parse_protocol(value).ok_or_else(|| {
        let other = value.trim().to_ascii_lowercase();
        Error::new(
            Kind::Usage,
            format!("{GRAPHICS_ENV}={other:?} is not a graphics protocol"),
        )
        .with_hint(format!(
            "set {GRAPHICS_ENV} to kitty, sixel, or iterm2, or unset it"
        ))
    })
}

/// Query the terminal for a picker for a real image protocol, or `None`
/// when it speaks none (or does not answer), and the client runs as text.
///
/// Must run after the alternate screen is entered and before any event is
/// read, because the terminal's answer arrives on stdin.
pub(crate) fn detect_graphics() -> Result<Option<Picker>> {
    let forced = match std::env::var(GRAPHICS_ENV) {
        Ok(v) if !v.trim().is_empty() => Some(parse_protocol(&v)?),
        _ => None,
    };
    Ok(terminal_kit::terminal::detect_graphics(forced))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("kitty", ProtocolType::Kitty)]
    #[case(" Sixel ", ProtocolType::Sixel)]
    #[case("ITERM2", ProtocolType::Iterm2)]
    fn protocol_names_parse_case_insensitively(#[case] v: &str, #[case] want: ProtocolType) {
        assert_eq!(parse_protocol(v).unwrap(), want);
    }

    #[rstest]
    #[case("halfblocks")]
    #[case("ascii")]
    fn non_image_protocols_are_rejected(#[case] v: &str) {
        let err = parse_protocol(v).unwrap_err();
        assert_eq!(err.kind(), Kind::Usage);
        assert!(err.message().contains(GRAPHICS_ENV));
    }

    #[test]
    fn the_rejection_names_the_value_as_it_was_read() {
        let err = parse_protocol(" ASCII ").unwrap_err();
        assert_eq!(
            err.to_string(),
            "error: BSKY_GRAPHICS=\"ascii\" is not a graphics protocol\nhint: set BSKY_GRAPHICS to kitty, sixel, or iterm2, or unset it"
        );
    }
}
