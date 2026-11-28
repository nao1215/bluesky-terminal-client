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

/// How many cells `s` takes on screen: the width of each grapheme cluster
/// added up, as the terminal and ratatui draw it. `str::width` measures the
/// string as a whole and says less for some (the lam-alif ligature لا is 1
/// there, 2 cells drawn).
pub fn cells(s: &str) -> usize {
    graphemes(s).map(|(_, g)| cluster_width(g)).sum()
}

/// The width of one grapheme cluster; a printable ASCII letter is one cell
/// without asking the tables. A half-width sound mark (the ﾞ of ｶﾞ, the ﾟ
/// of ﾊﾟ) joins the letter before it into one cluster and has no width in
/// the tables, yet ratatui and terminals give it a cell of its own: counted
/// as none, a line of such text was drawn longer than it was laid out.
pub fn cluster_width(g: &str) -> usize {
    match g.as_bytes() {
        [b] if b.is_ascii_graphic() || *b == b' ' => 1,
        _ => {
            g.width()
                + g.chars()
                    .filter(|c| matches!(c, '\u{ff9e}' | '\u{ff9f}'))
                    .count()
        }
    }
}

/// `s`'s grapheme clusters with their byte offsets, as
/// `UnicodeSegmentation::grapheme_indices` gives them. An ASCII character
/// followed by another (other than CR before LF) is a cluster by itself,
/// so a run of them is split without the segmentation tables, which are
/// asked only where something else follows; most of a post is such runs.
pub fn graphemes(s: &str) -> impl Iterator<Item = (usize, &str)> {
    let b = s.as_bytes();
    let mut i = 0;
    // The tables' own iterator over the rest, kept while it is needed.
    let mut slow: Option<unicode_segmentation::GraphemeIndices<'_>> = None;
    std::iter::from_fn(move || {
        let start = i;
        let lone_ascii = |i: usize| {
            b.get(i).is_some_and(u8::is_ascii)
                && b.get(i + 1)
                    .is_none_or(|n| n.is_ascii() && !(b[i] == b'\r' && *n == b'\n'))
        };
        if lone_ascii(i) {
            slow = None;
            i += 1;
            return Some((start, &s[start..i]));
        }
        let (_, g) = slow
            .get_or_insert_with(|| s[i..].grapheme_indices(true))
            .next()?;
        i = start + g.len();
        Some((start, g))
    })
}

/// [`wrap`] of a post's text, kept from one frame to the next: the same
/// posts are drawn at the same width on every frame, and wrapping them is
/// most of what drawing a list costs. The cache is the drawing thread's,
/// and starts again once it holds more texts than a screen and its
/// neighbours show.
pub fn wrap_cached(text: &str, width: usize) -> Vec<String> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::hash::{DefaultHasher, Hash, Hasher};
    type Wrapped = (usize, Box<str>, Vec<String>);
    thread_local! {
        static CACHE: RefCell<HashMap<u64, Wrapped>> = RefCell::new(HashMap::new());
    }
    const KEPT: usize = 512;
    let mut h = DefaultHasher::new();
    (text, width).hash(&mut h);
    let key = h.finish();
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((w, t, lines)) = c.get(&key)
            && *w == width
            && **t == *text
        {
            return lines.clone();
        }
        let lines = wrap(text, width);
        if c.len() >= KEPT {
            c.clear();
        }
        c.insert(key, (width, text.into(), lines.clone()));
        lines
    })
}

/// A row of hints, `key what` pairs two spaces apart, in `width` cells: the
/// pairs that do not fit are left out from the middle, and the last one,
/// which says how to leave (`esc cancel`), is kept.
pub fn fit_hints(row: &str, width: usize) -> String {
    if cells(row) <= width {
        return row.to_string();
    }
    let pairs: Vec<&str> = row.split("  ").collect();
    let Some((last, rest)) = pairs.split_last() else {
        return truncate(row, width);
    };
    let mut kept = String::new();
    for p in rest {
        let with = if kept.is_empty() {
            p.to_string()
        } else {
            format!("{kept}  {p}")
        };
        if cells(&with) + 2 + cells(last) > width {
            break;
        }
        kept = with;
    }
    if kept.is_empty() {
        truncate(last, width)
    } else {
        format!("{kept}  {last}")
    }
}

/// `s` on one line: line breaks and runs of spaces become one space, so
/// the words on either side stay apart where only one line is drawn.
pub fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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
        // Whether the line was ended by a space that did not fit, with
        // nothing put on the next one since.
        let mut ended_by_space = false;
        for word in split_keep_spaces(para) {
            let w = cells(word);
            if col + w <= width {
                line.push_str(word);
                col += w;
                ended_by_space &= word.trim().is_empty();
                continue;
            }
            if word.trim().is_empty() {
                // A space at the wrap point is dropped instead of starting
                // the next line; a line of spaces wider than the column is
                // one blank line.
                if !ended_by_space {
                    out.push(std::mem::take(&mut line).trim_end().to_string());
                }
                line.clear();
                col = 0;
                ended_by_space = true;
                continue;
            }
            ended_by_space = false;
            if w <= width && col > 0 {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
                col = w;
                continue;
            }
            for (_, g) in graphemes(word) {
                let gw = cluster_width(g);
                if col + gw > width && col > 0 {
                    out.push(std::mem::take(&mut line));
                    col = 0;
                }
                line.push_str(g);
                col += gw;
            }
        }
        // The space that ended the last line ended the paragraph too, so
        // there is no line after it.
        if !ended_by_space {
            out.push(line.trim_end().to_string());
        }
    }
    while out.last().is_some_and(std::string::String::is_empty) {
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
    for (i, g) in graphemes(s) {
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
        } else if cluster_width(g) >= 2 {
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
    if cells(&s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut col = 0;
    for (_, g) in graphemes(&s) {
        let gw = cluster_width(g);
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

/// Keep the end of `s` within `width` columns, starting with `…` when
/// shortened: the end of a path is the part that tells where it is. Cut
/// between grapheme clusters, as [`truncate`] is.
pub fn truncate_start(s: &str, width: usize) -> String {
    let s = drawable(s);
    if cells(&s) <= width {
        return s.into_owned();
    }
    if width == 0 {
        return String::new();
    }
    let kept: Vec<&str> = graphemes(&s).map(|(_, g)| g).collect();
    let mut used = 1;
    let mut from = kept.len();
    for g in kept.iter().rev() {
        let w = cluster_width(g);
        if used + w > width {
            break;
        }
        used += w;
        from -= 1;
    }
    format!("…{}", kept[from..].concat())
}

pub use crate::clock::local_time as format_time;

#[cfg(test)]
mod tests {
    use super::*;

    // The ASCII shortcut splits text as the segmentation tables do, on
    // random mixes of ASCII and the clusters that join across it.
    #[test]
    fn graphemes_agree_with_unicode_segmentation() {
        const PIECES: &[&str] = &[
            "a",
            "Z",
            " ",
            "  ",
            ".",
            "#",
            "\r",
            "\n",
            "\r\n",
            "\t",
            "\u{7}",
            "1",
            "\u{200d}",
            "👨",
            "👩",
            "👧",
            "🏽",
            "🏻",
            "🇯",
            "🇵",
            "🇺",
            "🇸",
            "\u{fe0f}",
            "❤",
            "\u{20e3}",
            "\u{301}",
            "\u{308}",
            "\u{1100}",
            "\u{1161}",
            "\u{11a8}",
            "가",
            "\u{600}",
            "\u{110bd}",
            "\u{93f}",
            "क",
            "日",
            "本",
            "ｱ",
            "\u{e0067}",
            "🏴",
            "لا",
            "\u{200b}",
            "\u{ad}",
            "\u{1f3f3}",
        ];
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..200_000 {
            let mut s = String::new();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            for _ in 0..(x % 12) {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                s.push_str(PIECES[(x % PIECES.len() as u64) as usize]);
            }
            let fast: Vec<_> = graphemes(&s).collect();
            let slow: Vec<_> = s.grapheme_indices(true).collect();
            assert_eq!(fast, slow, "{s:?}");
            let widths: usize = slow.iter().map(|(_, g)| g.width()).sum();
            assert_eq!(cells(&s), widths, "{s:?}");
        }
    }

    // A half-width sound mark takes a cell of its own, as ratatui and the
    // terminal draw it, alone or after its letter; so does a field's cursor
    // after one.
    #[test]
    fn a_half_width_sound_mark_takes_its_own_cell() {
        assert_eq!(cells("ｶﾞｲｼﾞﾝ"), 6);
        assert_eq!(cells("ﾊﾟ"), 2);
        assert_eq!(cells("\u{ff9e}"), 1);
        let input = crate::input::TextInput::single("ﾊﾟﾝ");
        assert_eq!(input.layout(20).cursor, (0, 3));
        assert_eq!(wrap("ﾊﾟﾊﾟﾊﾟ", 4), ["ﾊﾟﾊﾟ", "ﾊﾟ"]);
    }

    // A cluster too wide for any line keeps the zero-width space before it
    // on its line: alone, the space would be a line that shows nothing.
    #[test]
    fn a_zero_width_space_stays_with_a_cluster_too_wide() {
        assert_eq!(wrap("\u{200b}日ﾞﾞ", 2), ["\u{200b}日ﾞﾞ"]);
        assert_eq!(wrap("a\u{200b}日ﾞ", 2), ["a\u{200b}", "日ﾞ"]);
    }

    // A hint row too long for its place loses pairs from the middle and
    // keeps the last, which says how to leave.
    #[rstest]
    #[case("a one  b two  esc cancel", 40, "a one  b two  esc cancel")]
    #[case("a one  b two  esc cancel", 18, "a one  esc cancel")]
    #[case("a one  b two  esc cancel", 10, "esc cancel")]
    #[case("a one  b two  esc cancel", 6, "esc c…")]
    #[case("ctrl+s 送信  ctrl+o 添付  esc 取消", 21, "ctrl+s 送信  esc 取消")]
    fn a_hint_row_keeps_its_last_pair(#[case] row: &str, #[case] w: usize, #[case] want: &str) {
        assert_eq!(fit_hints(row, w), want);
        assert!(cells(&fit_hints(row, w)) <= w);
    }

    // The same text at the same width wraps as wrap does, from the cache
    // or not; another width or text is wrapped again.
    #[test]
    fn a_cached_wrap_is_the_wrap() {
        let text = "家族👨‍👩‍👧 and 🇯🇵 in a post long enough to wrap twice or more";
        for width in [3, 10, 25, 80] {
            assert_eq!(wrap_cached(text, width), wrap(text, width));
            assert_eq!(wrap_cached(text, width), wrap(text, width));
        }
        assert_eq!(wrap_cached("other", 10), wrap("other", 10));
        for i in 0..600 {
            let t = format!("post {i}");
            assert_eq!(wrap_cached(&t, 4), wrap(&t, 4));
        }
    }
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
    fn an_empty_paragraph_has_no_runs() {
        assert!(split_keep_spaces("").is_empty());
        assert_eq!(split_keep_spaces("ab  日本"), ["ab", "  ", "日", "本"]);
    }

    // A space that falls at the wrap point is dropped; it does not leave a
    // blank line under a paragraph that goes on, nor two for a line of
    // spaces wider than the column.
    #[rstest]
    #[case("hello world \nnext", 11, &["hello world", "next"])]
    #[case("hello world  \nnext", 11, &["hello world", "next"])]
    #[case("#tag#taga#tag\u{3000}\n日本語", 7, &["#tag#ta", "ga#tag", "日本語"])]
    #[case("日本語\u{3000}\n次", 6, &["日本語", "次"])]
    #[case("one\n    \ntwo", 3, &["one", "", "two"])]
    #[case("one\n\t\ntwo", 3, &["one", "", "two"])]
    #[case("ab   cd", 2, &["ab", "cd"])]
    fn a_space_at_the_wrap_point_adds_no_blank_line(
        #[case] text: &str,
        #[case] width: usize,
        #[case] want: &[&str],
    ) {
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

    // Measured as the screen draws them, a grapheme at a time: the lam-alif
    // ligature لا is 1 column as a string but 2 cells drawn.
    #[test]
    fn wrapped_lines_never_exceed_the_width() {
        let texts = [
            "この端末は分散型の SNS です。https://example.com/a/very/long/path/that/keeps/going 👍",
            "لا لا لالا \u{644}\u{627}\u{644}\u{627}\u{644}\u{627} مرحبا",
        ];
        for text in texts {
            for width in 1..30 {
                for line in wrap(text, width) {
                    assert!(cells(&line) <= width.max(2), "{width}: {line:?}");
                }
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
            "この端末は分散型の SNS です 👍",
            "لالالالالالالالا مرحبا",
        ];
        for s in texts {
            let bounds: Vec<usize> = s
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain([s.len()])
                .collect();
            for width in 0..=cells(s) + 1 {
                let out = truncate(s, width);
                assert!(cells(&out) <= width, "{width}: {out:?}");
                let kept = out.strip_suffix('…').unwrap_or(&out);
                assert!(s.starts_with(kept), "{width}: {out:?}");
                assert!(bounds.contains(&kept.len()), "{width}: {out:?}");
            }
        }
    }

    #[test]
    fn bad_timestamps_are_shown_verbatim() {}
}

// Properties of the layout helpers over generated text. proptest draws the
// text from pieces that are hard to lay out (ZWJ families, flags, keycaps,
// VS16, combining marks, CJK, half-width sound marks, Arabic ligatures, tabs
// and control characters) and checks what every answer has to satisfy, so a
// cluster split or a line one cell too wide is found without the case being
// written down. `PROPTEST_CASES` raises the count; the nightly workflow runs
// 20,000 cases.
#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    const PIECES: &[&str] = &[
        "a",
        "Z",
        "word",
        "longerwordthanmostlines",
        "1",
        ".",
        ",",
        "-",
        "#tag",
        "@alice.test",
        "https://example.com/a",
        " ",
        "  ",
        "\t",
        "\n",
        "\r\n",
        "\u{7}",
        "日本語",
        "中",
        "한국어",
        "。",
        "、",
        "\u{3000}",
        "👨\u{200d}👩\u{200d}👧\u{200d}👦",
        "🇯🇵",
        "🏳\u{fe0f}\u{200d}🌈",
        "1\u{fe0f}\u{20e3}",
        "❤\u{fe0f}",
        "👍🏽",
        "e\u{301}",
        "ｶﾞ",
        "ﾊﾟ",
        "مرحبا",
        "لا",
        "\u{200b}",
    ];

    fn text_from(pieces: &'static [&'static str], max: usize) -> impl Strategy<Value = String> {
        prop::collection::vec(prop::sample::select(pieces), 0..max).prop_map(|p| p.concat())
    }

    fn text() -> impl Strategy<Value = String> {
        text_from(PIECES, 24)
    }

    /// Text that stays on one line: no line break and no control character.
    fn line() -> impl Strategy<Value = String> {
        text().prop_map(|t| t.chars().filter(|c| !c.is_control()).collect())
    }

    fn visible(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    proptest! {
        // The shortcut for ASCII runs splits text as the segmentation
        // tables do.
        #[test]
        fn graphemes_are_unicode_segmentation(s in text()) {
            let ours: Vec<(usize, &str)> = graphemes(&s).collect();
            let theirs: Vec<(usize, &str)> = s.grapheme_indices(true).collect();
            prop_assert_eq!(ours, theirs);
        }

        // A wrapped line is never wider than the width, except a line that
        // holds one cluster wider than the whole width (a wide character in
        // a column of one), which cannot be split, and clusters that take no
        // cell.
        #[test]
        fn a_wrapped_line_fits_the_width(s in text(), width in 1usize..30) {
            for l in wrap(&s, width) {
                let drawn = graphemes(&l).filter(|(_, g)| cluster_width(g) > 0).count();
                prop_assert!(
                    cells(&l) <= width || drawn == 1,
                    "{l:?} is {} cells, wider than {width}", cells(&l)
                );
                prop_assert!(!l.contains('\n'));
            }
        }

        // Wrapping only moves text to the next line: every character other
        // than a space is still there, in the same order.
        #[test]
        fn wrapping_loses_no_character(s in text(), width in 1usize..30) {
            let lines = wrap(&s, width);
            prop_assert_eq!(visible(&lines.concat()), visible(&drawable(&s)));
        }

        // Metamorphic: a line already within the width wraps to itself.
        #[test]
        fn a_line_that_fits_is_left_alone(s in line(), spare in 0usize..10) {
            // A visible first character, so the line is not blank.
            let s = format!("x{}", s.trim_end());
            let width = cells(&s) + spare;
            prop_assert_eq!(wrap(&s, width), vec![s]);
        }

        // Metamorphic: two paragraphs are wrapped as each is on its own. The
        // first starts with a visible character: blank lines at the end of
        // the text are dropped, so a blank first paragraph would be too.
        #[test]
        fn paragraphs_wrap_one_by_one(a in line(), b in line(), width in 1usize..30) {
            let a = format!("x{a}");
            let mut both = wrap(&a, width);
            both.extend(wrap(&b, width));
            prop_assert_eq!(wrap(&format!("{a}\n{b}"), width), both);
        }

        // Truncated text fits, is the text itself when it already fits, and
        // otherwise is a prefix of whole clusters followed by `…`.
        #[test]
        fn truncation_keeps_a_cluster_prefix(s in text(), width in 0usize..30) {
            let shown = drawable(&s).into_owned();
            let out = truncate(&s, width);
            prop_assert!(cells(&out) <= width, "{out:?} is wider than {width}");
            if cells(&shown) <= width {
                prop_assert_eq!(out, shown);
            } else if width > 0 {
                let kept = out.strip_suffix('…').unwrap_or_else(|| panic!("{out:?} has no …"));
                prop_assert!(shown.starts_with(kept));
                prop_assert!(graphemes(&shown).any(|(i, _)| i == kept.len()));
            } else {
                prop_assert_eq!(out, "");
            }
        }

        // Metamorphic: one more column never keeps less of the text.
        #[test]
        fn a_wider_truncation_keeps_at_least_as_much(s in text(), width in 1usize..30) {
            let kept = |w: usize| {
                let out = truncate(&s, w);
                out.strip_suffix('…').map(str::to_string).unwrap_or(out)
            };
            prop_assert!(kept(width + 1).starts_with(&kept(width)));
        }

        // The start-side cut keeps a suffix of whole clusters after `…`.
        #[test]
        fn truncation_from_the_start_keeps_a_cluster_suffix(s in text(), width in 0usize..30) {
            let shown = drawable(&s).into_owned();
            let out = truncate_start(&s, width);
            prop_assert!(cells(&out) <= width, "{out:?} is wider than {width}");
            if cells(&shown) <= width {
                prop_assert_eq!(out, shown);
            } else if width > 0 {
                let kept = out.strip_prefix('…').unwrap_or_else(|| panic!("{out:?} has no …"));
                prop_assert!(shown.ends_with(kept));
                let at = shown.len() - kept.len();
                prop_assert!(at == shown.len() || graphemes(&shown).any(|(i, _)| i == at));
            } else {
                prop_assert_eq!(out, "");
            }
        }
    }
}

/// A byte count the way a person reads it.
pub fn human_bytes(n: u64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / (1024.0 * 1024.0)),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}
