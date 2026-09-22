//! Terminal checks that run before the UI starts.
//!
//! bsky draws images with the terminal's own graphics protocol. It asks the
//! terminal which protocols it speaks (kitty graphics, sixel, or iTerm2
//! inline images) and refuses to start when the answer is none of them,
//! rather than degrading to a client that silently drops every picture.

use std::io::IsTerminal;

use ratatui_image::picker::{Picker, ProtocolType};

use crate::error::{Error, Kind, Result};

/// Environment variable that forces a graphics protocol, for terminals that
/// support one but do not answer the detection query (some multiplexers).
pub const GRAPHICS_ENV: &str = "BSKY_GRAPHICS";

/// Fail unless stdin and stdout are both terminals.
pub fn ensure_interactive() -> Result<()> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Ok(())
    } else {
        Err(
            Error::new(Kind::Terminal, "bsky needs an interactive terminal")
                .with_hint("run bsky directly in a terminal, not through a pipe or redirect"),
        )
    }
}

/// Parse a `BSKY_GRAPHICS` value.
pub fn parse_protocol(value: &str) -> Result<ProtocolType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kitty" => Ok(ProtocolType::Kitty),
        "sixel" => Ok(ProtocolType::Sixel),
        "iterm2" => Ok(ProtocolType::Iterm2),
        other => Err(Error::new(
            Kind::Usage,
            format!("{GRAPHICS_ENV}={other:?} is not a graphics protocol"),
        )
        .with_hint(format!(
            "set {GRAPHICS_ENV} to kitty, sixel, or iterm2, or unset it"
        ))),
    }
}

/// Human name of a protocol, for the status line.
pub fn protocol_name(p: ProtocolType) -> &'static str {
    match p {
        ProtocolType::Kitty => "kitty",
        ProtocolType::Sixel => "sixel",
        ProtocolType::Iterm2 => "iterm2",
        ProtocolType::Halfblocks => "halfblocks",
    }
}

/// Query the terminal and return a picker for a real image protocol.
///
/// Must run after the alternate screen is entered and before any event is
/// read, because the terminal's answer arrives on stdin.
pub fn detect_graphics() -> Result<Picker> {
    let forced = match std::env::var(GRAPHICS_ENV) {
        Ok(v) if !v.trim().is_empty() => Some(parse_protocol(&v)?),
        _ => None,
    };
    let mut picker = Picker::from_query_stdio().map_err(|e| {
        Error::new(
            Kind::Terminal,
            format!("cannot query the terminal for image support: {e}"),
        )
    })?;
    if let Some(p) = forced {
        picker.set_protocol_type(p);
    }
    if picker.protocol_type() == ProtocolType::Halfblocks {
        return Err(
            Error::new(Kind::Terminal, "this terminal cannot display images").with_hint(format!(
                "use a terminal with kitty graphics, sixel, or iTerm2 inline images \
                 (kitty, Ghostty, WezTerm, foot, iTerm2, ...), or set {GRAPHICS_ENV} \
                 when yours supports one but does not answer the query"
            )),
        );
    }
    Ok(picker)
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
}
