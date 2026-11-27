//! Terminal checks that run before the UI starts.
//!
//! Pictures are drawn with the terminal's own graphics protocol. The
//! terminal is asked which protocols it speaks (kitty graphics, sixel, or
//! iTerm2 inline images); when the answer is none of them, the client runs
//! as text rather than drawing pictures the terminal cannot show.

use std::io::IsTerminal;

use ratatui_image::picker::{Picker, ProtocolType};

/// Whether stdin and stdout are both terminals.
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// The protocol a user named (`kitty`, `sixel` or `iterm2`, in any case and
/// with spaces around it), or `None` for anything else: halfblocks draws no
/// picture, so it is not one to ask for.
pub fn parse_protocol(value: &str) -> Option<ProtocolType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "kitty" => Some(ProtocolType::Kitty),
        "sixel" => Some(ProtocolType::Sixel),
        "iterm2" => Some(ProtocolType::Iterm2),
        _ => None,
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

/// Query the terminal for a picker for a real image protocol, or `None`
/// when it speaks none (or does not answer), and the client runs as text.
/// `forced` is the protocol to use whatever the terminal answers, for
/// terminals that support one but do not answer the query (some
/// multiplexers).
///
/// Must run after the alternate screen is entered and before any event is
/// read, because the terminal's answer arrives on stdin.
pub fn detect_graphics(forced: Option<ProtocolType>) -> Option<Picker> {
    let mut picker = Picker::from_query_stdio().ok()?;
    if let Some(p) = forced {
        picker.set_protocol_type(p);
    }
    (picker.protocol_type() != ProtocolType::Halfblocks).then_some(picker)
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
        assert_eq!(parse_protocol(v), Some(want));
    }

    #[rstest]
    #[case("halfblocks")]
    #[case("ascii")]
    #[case("")]
    fn non_image_protocols_are_rejected(#[case] v: &str) {
        assert_eq!(parse_protocol(v), None);
    }

    #[rstest]
    #[case(ProtocolType::Kitty, "kitty")]
    #[case(ProtocolType::Sixel, "sixel")]
    #[case(ProtocolType::Iterm2, "iterm2")]
    #[case(ProtocolType::Halfblocks, "halfblocks")]
    fn a_protocol_name_parses_back_to_the_protocol(#[case] p: ProtocolType, #[case] name: &str) {
        assert_eq!(protocol_name(p), name);
        if p != ProtocolType::Halfblocks {
            assert_eq!(parse_protocol(name), Some(p));
        }
    }
}
