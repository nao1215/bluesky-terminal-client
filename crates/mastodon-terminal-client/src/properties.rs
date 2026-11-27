//! Properties of the code that reads a status's HTML and counts a post's
//! length, checked on generated text: emoji with ZWJ, skin tones, flags and
//! VS16, CJK, half-width kana, combining marks, controls, the characters
//! HTML gives a meaning to, and Mastodon's links, mentions and hashtags. A
//! run takes a few seconds with the default 256 cases; `PROPTEST_CASES`
//! asks for more. A failure prints the smallest text it found, which
//! belongs in a unit test of its own once fixed.

use proptest::prelude::*;

use crate::cli::counted;
use crate::html::to_text;

/// Pieces of plain text a status is made of: each a grapheme or a short
/// run, the ones HTML escapes among them.
const PIECES: &[&str] = &[
    "a",
    "Z",
    "0",
    " ",
    "  ",
    "\n",
    "\n\n",
    "\n\n\n",
    "\t",
    "\u{7}",
    "\u{1b}[31m",
    "日",
    "本",
    "語",
    "ｶ",
    "ﾞ",
    "한",
    "é",
    "e\u{301}",
    "\u{200b}",
    "👍",
    "👍🏽",
    "🇯🇵",
    "❤️",
    "🏛️",
    "👨\u{200d}👩\u{200d}👧\u{200d}👦",
    "🐈\u{200d}⬛",
    "<",
    ">",
    "&",
    "&amp;",
    "\"",
    "'",
    "<p>",
    "</a>",
    "<br>",
    ":blobcat:",
    "-",
    ".",
    "/",
    "…",
];

/// A piece of a status as its author wrote it.
#[derive(Debug, Clone)]
enum Piece {
    Text(&'static str),
    /// An address, which Mastodon links.
    Url(&'static str),
    /// A mention of an account, `@name` or `@name@host`.
    Mention(&'static str),
    Tag(&'static str),
}

const URLS: &[&str] = &[
    "https://e.example/a",
    "https://e.example/a/very/long/path/that/is/cut?x=1&y=2#frag",
    "http://日本.example/パス",
    "https://e.example/",
];

fn piece() -> impl Strategy<Value = Piece> {
    prop_oneof![
        6 => prop::sample::select(PIECES).prop_map(Piece::Text),
        1 => prop::sample::select(URLS).prop_map(Piece::Url),
        1 => prop::sample::select(&["alice", "bob@other.example", "a_b"][..]).prop_map(Piece::Mention),
        1 => prop::sample::select(&["rust", "猫", "a_1"][..]).prop_map(Piece::Tag),
    ]
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// What the author typed, what it reads as (Mastodon shows a mention of
/// another server's account as `@name`, its address being on the link),
/// and the HTML Mastodon makes of it: the text
/// escaped, a paragraph at each blank line, `<br />` at each other line
/// break, and its links, mentions and hashtags as Mastodon writes them. A
/// link, a mention or a hashtag stands apart from what is around it, as it
/// must for Mastodon to see it.
fn status(pieces: &[Piece]) -> (String, String, String) {
    let mut typed = String::new();
    let mut reads = String::new();
    let mut html = String::new();
    for p in pieces {
        let (t, h) = match p {
            Piece::Text(s) => (s.to_string(), escape(s)),
            Piece::Mention(m) => {
                let name = m.split('@').next().unwrap_or(m);
                typed.push_str(&format!(" @{m} "));
                reads.push_str(&format!(" @{name} "));
                html.push_str(&format!(
                    " <span class=\"h-card\" translate=\"no\"><a href=\"https://m.example/@{m}\" class=\"u-url mention\">@<span>{name}</span></a></span> "
                ));
                continue;
            }
            Piece::Url(u) => {
                let (scheme, rest) = u.split_at(u.find("://").map_or(0, |i| i + 3));
                let cut = rest.char_indices().nth(10).map_or(rest.len(), |(i, _)| i);
                (
                    format!(" {u} "),
                    format!(
                        " <a href=\"{}\" target=\"_blank\" rel=\"nofollow noopener\" translate=\"no\"><span class=\"invisible\">{}</span><span class=\"ellipsis\">{}</span><span class=\"invisible\">{}</span></a> ",
                        escape(u),
                        escape(scheme),
                        escape(&rest[..cut]),
                        escape(&rest[cut..])
                    ),
                )
            }
            Piece::Tag(t) => (
                format!(" #{t} "),
                format!(
                    " <a href=\"https://m.example/tags/{t}\" class=\"mention hashtag\" rel=\"tag\">#<span>{t}</span></a> "
                ),
            ),
        };
        typed.push_str(&t);
        reads.push_str(&t);
        html.push_str(&h);
    }
    // Mastodon trims the text and writes the paragraphs; the HTML pieces
    // were escaped one by one, so the paragraphs are made on both at once.
    (typed, reads, paragraphs(&html))
}

/// `html` (escaped text with raw line breaks) as Mastodon's paragraphs.
fn paragraphs(html: &str) -> String {
    let trimmed = html.trim_matches(|c: char| c == ' ' || c == '\n' || c == '\t');
    let mut out = String::new();
    for para in split_blank(trimmed) {
        out.push_str("<p>");
        out.push_str(&para.replace('\n', "<br />"));
        out.push_str("</p>");
    }
    out
}

/// `s` split at each run of two line breaks or more, as Rails splits it.
fn split_blank(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut newlines = 0;
    for c in s.chars() {
        if c == '\n' {
            newlines += 1;
            continue;
        }
        match newlines {
            0 => {}
            1 => {
                if let Some(l) = out.last_mut() {
                    l.push('\n');
                }
            }
            _ => out.push(String::new()),
        }
        newlines = 0;
        if let Some(l) = out.last_mut() {
            l.push(c);
        }
    }
    out
}

/// The text as it reads after the round trip: what was typed, its lines
/// without the spaces at their ends, no more than one blank line in a row,
/// none at the start or the end.
fn normalized(typed: &str) -> String {
    let trimmed = typed.trim_matches(|c: char| c == ' ' || c == '\n' || c == '\t');
    let mut out = String::new();
    let mut blank = 0;
    for line in trimmed.split('\n') {
        let line = line.trim_end_matches([' ', '\u{a0}']);
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        if !out.is_empty() || !line.is_empty() {
            if !out.is_empty() || blank > 0 {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out.trim_end_matches('\n').to_string()
}

/// Bits of HTML, well formed or not.
const SOUP: &[&str] = &[
    "<p>",
    "</p>",
    "<br>",
    "<br />",
    "<a href=\"https://e.example/x\">",
    "<a href='x'>",
    "</a>",
    "<span class=\"invisible\">",
    "</span>",
    "<blockquote>",
    "</blockquote>",
    "<ul>",
    "<ol>",
    "<li>",
    "</li>",
    "</ul>",
    "</ol>",
    "<pre>",
    "</pre>",
    "<script>",
    "</script>",
    "<!--",
    "-->",
    "<img alt=\":x:\">",
    "&amp;",
    "&#128008;",
    "&#xZZ;",
    "&",
    "<",
    ">",
    "\"",
    "a",
    " ",
    "\n",
    "日本",
    "👨\u{200d}👩\u{200d}👧",
    "e\u{301}",
    "\u{1b}",
    "<b",
    "=\"",
    "'",
];

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(SOUP), 0..40).prop_map(|p| p.concat())
}

proptest! {
    // What an author typed, made HTML as Mastodon makes it, reads back as
    // what they typed: every character, emoji and CJK whole, the links
    // whole, the mentions and hashtags as written.
    #[test]
    fn a_status_reads_back_as_it_was_typed(pieces in prop::collection::vec(piece(), 0..30)) {
        let (_, reads, html) = status(&pieces);
        prop_assert_eq!(to_text(&html), normalized(&reads), "{}", html);
    }

    // Whatever the HTML, the text is read without a panic, has no space at
    // the end of a line, no more than one blank line in a row, and none at
    // its start or end.
    #[test]
    fn any_html_reads_as_tidy_text(html in soup()) {
        let text = to_text(&html);
        for line in text.split('\n') {
            prop_assert!(!line.ends_with(' '), "{:?}", text);
        }
        prop_assert!(!text.contains("\n\n\n"), "{:?}", text);
        prop_assert!(!text.starts_with('\n') && !text.ends_with('\n'), "{:?}", text);
    }

    // A link keeps its text, and its address whenever the text is not it.
    #[test]
    fn a_link_never_loses_its_address(label in "[a-zA-Z0-9 ぁ-ん]{0,12}", path in "[a-z0-9]{0,8}") {
        let href = format!("https://e.example/{path}");
        let text = to_text(&format!("<p>see <a href=\"{href}\">{label}</a></p>"));
        prop_assert!(text.contains(label.trim()), "{:?}", text);
        prop_assert!(text.contains(&href) || text.contains(&href.trim_start_matches("https://").to_string()), "{:?}", text);
    }

    // The length of a post never counts an address as more than the
    // server's reserved length, nor a remote mention's host; text with
    // neither counts as its graphemes.
    #[test]
    fn a_post_counts_as_mastodon_counts_it(pieces in prop::collection::vec(piece(), 0..30)) {
        let (typed, _, _) = status(&pieces);
        let per_url = 23;
        let mut plain = typed.clone();
        for p in &pieces {
            match p {
                Piece::Url(u) => plain = plain.replacen(u, &"x".repeat(per_url), 1),
                Piece::Mention(m) if m.contains('@') => {
                    plain = plain.replacen(&format!("@{m}"), &format!("@{}", m.split('@').next().unwrap_or("")), 1);
                }
                _ => {}
            }
        }
        use unicode_segmentation::UnicodeSegmentation;
        prop_assert_eq!(counted(&typed, per_url), plain.graphemes(true).count(), "{:?}", typed);
    }
}
