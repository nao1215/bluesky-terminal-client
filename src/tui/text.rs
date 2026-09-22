//! Text helpers for drawing: wrapping by display width, truncation, and times.

use chrono::{DateTime, Local};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Wrap `text` into lines at most `width` columns wide.
///
/// Words move to the next line whole when they fit on one; longer words,
/// and text without spaces such as Japanese, break between grapheme
/// clusters, so an emoji with a skin-tone modifier is never split.
/// Explicit newlines are kept, and trailing blank lines are dropped.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.lines() {
        let mut line = String::new();
        let mut col = 0;
        for word in split_keep_spaces(para) {
            let w = word.width();
            if col + w <= width {
                line.push_str(word);
                col += w;
                continue;
            }
            if word.trim().is_empty() {
                // A space at the wrap point is dropped instead of starting the next line.
                out.push(std::mem::take(&mut line));
                col = 0;
                continue;
            }
            if w <= width && col > 0 {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
                col = w;
                continue;
            }
            for g in word.graphemes(true) {
                let gw = g.width();
                if col + gw > width && col > 0 {
                    out.push(std::mem::take(&mut line));
                    col = 0;
                }
                line.push_str(g);
                col += gw;
            }
        }
        out.push(line.trim_end().to_string());
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// Split into alternating runs of non-space and space grapheme clusters.
/// Wide clusters are their own runs so CJK text can break between them.
fn split_keep_spaces(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut prev: Option<Class> = None;
    for (i, g) in s.grapheme_indices(true) {
        let class = Class::of(g);
        if let Some(p) = prev
            && (p != class || class == Class::Wide)
        {
            parts.push(&s[start..i]);
            start = i;
        }
        prev = Some(class);
    }
    if start < s.len() {
        parts.push(&s[start..]);
    }
    parts
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Wide,
    Narrow,
}

impl Class {
    fn of(g: &str) -> Self {
        if g.chars().all(char::is_whitespace) {
            Class::Space
        } else if g.width() >= 2 {
            Class::Wide
        } else {
            Class::Narrow
        }
    }
}

/// Cut `s` to at most `width` columns, ending with `…` when shortened.
pub fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut col = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if col + cw + 1 > width {
            break;
        }
        out.push(c);
        col += cw;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

/// Format an RFC 3339 timestamp as local `YYYY-MM-DD HH:MM`; unparsable
/// input is returned as-is.
pub fn format_time(ts: &str) -> String {
    match DateTime::parse_from_rfc3339(ts) {
        Ok(t) => t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => ts.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("hello world", 5, &["hello", "world"])]
    #[case("hello world", 11, &["hello world"])]
    #[case("a b c", 3, &["a b", "c"])]
    #[case("abcdefgh", 3, &["abc", "def", "gh"])]
    #[case("こんにちは世界", 6, &["こんに", "ちは世", "界"])]
    #[case("one\n\ntwo\n\n", 10, &["one", "", "two"])]
    #[case("hi 日本語", 5, &["hi 日", "本語"])]
    fn wraps_by_display_width(#[case] text: &str, #[case] width: usize, #[case] want: &[&str]) {
        assert_eq!(wrap(text, width), want);
    }

    #[test]
    fn an_emoji_with_a_modifier_is_never_split() {
        for width in 1..6 {
            for line in wrap("👍🏽👍🏽👍🏽", width) {
                assert!(!line.starts_with('\u{1F3FD}'), "{width}: {line:?}");
                assert!(line.chars().filter(|c| *c == '👍').count() * 2 == line.chars().count());
            }
        }
    }

    #[test]
    fn wrapped_lines_never_exceed_the_width() {
        let text =
            "Bluesky は分散型の SNS です。https://example.com/a/very/long/path/that/keeps/going 👍";
        for width in 1..30 {
            for line in wrap(text, width) {
                assert!(line.width() <= width.max(2), "{width}: {line:?}");
            }
        }
    }

    #[rstest]
    #[case("short", 10, "short")]
    #[case("abcdefghij", 5, "abcd…")]
    #[case("日本語テキスト", 7, "日本語…")]
    fn truncates_with_an_ellipsis(#[case] s: &str, #[case] w: usize, #[case] want: &str) {
        assert_eq!(truncate(s, w), want);
    }

    #[test]
    fn bad_timestamps_are_shown_verbatim() {
        assert_eq!(format_time("yesterday"), "yesterday");
        assert_eq!(format_time("2026-09-22T01:02:03.000Z").len(), 16);
    }
}
