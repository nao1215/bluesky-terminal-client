//! Text others wrote, made safe to print.

/// `text` as text only: a control character in it would reach the terminal
/// as a command (an escape sequence can set the clipboard or the window
/// title, or clear the screen), so it is dropped. A line break stays, a
/// carriage return becomes one, and a tab becomes four spaces. The rule is
/// terminal-kit's, the one the full-screen client lays text out by.
pub(crate) use terminal_kit::text::drawable;

/// As [`drawable`], on one line: for what is printed as one line.
pub(crate) fn one_line(s: &str) -> String {
    drawable(s).replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn escape_sequences_lose_their_control_characters_and_text_stays() {
        assert_eq!(
            drawable("a\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J\tb\r\nc\rd\u{9b}"),
            "a]52;c;cHduZWQ=[2J    b\nc\nd"
        );
        assert_eq!(drawable("今日は👍🏽 🇯🇵\n2行目"), "今日は👍🏽 🇯🇵\n2行目");
        assert!(matches!(drawable("plain"), Cow::Borrowed(_)));
    }

    #[test]
    fn one_line_joins_the_lines() {
        assert_eq!(one_line("Al\nice\r\n👨‍👩‍👧"), "Al ice 👨‍👩‍👧");
    }
}
