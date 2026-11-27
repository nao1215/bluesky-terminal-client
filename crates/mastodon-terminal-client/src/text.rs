//! Text others wrote, made safe to print.

use std::borrow::Cow;

/// `text` as text only: a control character in it would reach the terminal
/// as a command (an escape sequence can set the clipboard or the window
/// title, or clear the screen), so it is dropped. A line break stays, a
/// carriage return becomes one, and a tab becomes four spaces.
pub fn drawable(text: &str) -> Cow<'_, str> {
    if !text.chars().any(|c| c.is_control() && c != '\n') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' | '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// As [`drawable`], on one line: for what is printed as one line.
pub fn one_line(s: &str) -> String {
    drawable(s).replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

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
