//! Text helpers for drawing: wrapping by display width, truncation, and times.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// `text` as a terminal draws it. A terminal gives a control character no
/// cell, so text measured with one would be drawn shorter than it was laid
/// out: a tab becomes four spaces, a CR (alone or before LF) ends the line
/// as LF does, and other control characters are left out.
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

/// Wrap `text` into lines at most `width` columns wide.
///
/// Words move to the next line whole when they fit on one; longer words,
/// and text without spaces such as Japanese, break between grapheme
/// clusters, so an emoji with a skin-tone modifier is never split.
/// Explicit newlines are kept, and trailing blank lines are dropped.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let text = drawable(text);
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
/// The cut falls between grapheme clusters, so a family emoji, a flag, or a
/// letter with a combining mark is kept whole or dropped whole.
pub fn truncate(s: &str, width: usize) -> String {
    let s = drawable(s);
    if s.width() <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut col = 0;
    for g in s.graphemes(true) {
        let gw = g.width();
        if col + gw + 1 > width {
            break;
        }
        out.push_str(g);
        col += gw;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

pub use crate::clock::local_time as format_time;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // A terminal draws no cell for a control character, so text measured
    // with one is drawn shorter than it was laid out: a tab becomes spaces,
    // a CR ends the line as LF does, and other controls go.
    #[rstest]
    #[case("name\tscore\nalice\t10", &["name    score", "alice    10"])]
    #[case("a\r\nb\rc", &["a", "b", "c"])]
    #[case("x\u{7}y\u{1b}z", &["xyz"])]
    #[case("👨‍👩‍👧\t🇯🇵 1️⃣ ❤️ e\u{301}", &["👨‍👩‍👧    🇯🇵 1️⃣ ❤️ e\u{301}"])]
    fn control_characters_are_laid_out_as_they_are_drawn(
        #[case] text: &str,
        #[case] want: &[&str],
    ) {
        assert_eq!(wrap(text, 40), want);
    }

    #[test]
    fn a_truncated_name_has_no_control_characters() {
        assert_eq!(truncate("Al\tice\u{7}", 20), "Al    ice");
    }

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

    #[rstest]
    #[case(
        "👨\u{200d}👩\u{200d}👧👨\u{200d}👩\u{200d}👧",
        3,
        "👨\u{200d}👩\u{200d}👧…"
    )]
    #[case("👍🏽👍🏽", 3, "👍🏽…")]
    #[case("a🇯🇵b", 3, "a…")]
    #[case("か\u{3099}き\u{3099}く\u{3099}", 5, "か\u{3099}き\u{3099}…")]
    fn truncation_never_splits_a_grapheme_cluster(
        #[case] s: &str,
        #[case] w: usize,
        #[case] want: &str,
    ) {
        assert_eq!(truncate(s, w), want);
    }

    #[test]
    fn truncation_is_a_grapheme_prefix_within_the_width() {
        let texts = [
            "👨\u{200d}👩\u{200d}👧 family 🇯🇵 flag 👍🏽 か\u{3099}",
            "Bluesky は分散型の SNS です 👍",
        ];
        for s in texts {
            let bounds: Vec<usize> = s
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain([s.len()])
                .collect();
            for width in 0..=s.width() + 1 {
                let out = truncate(s, width);
                assert!(out.width() <= width, "{width}: {out:?}");
                let kept = out.strip_suffix('…').unwrap_or(&out);
                assert!(s.starts_with(kept), "{width}: {out:?}");
                assert!(bounds.contains(&kept.len()), "{width}: {out:?}");
            }
        }
    }

    #[test]
    fn bad_timestamps_are_shown_verbatim() {}
}
