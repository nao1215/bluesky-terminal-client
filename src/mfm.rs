//! The links in a note's text, as Misskey's MFM reads them.
//!
//! Misskey sends no card with a note. Each client that shows the note
//! finds the addresses in its text and asks its own server for a preview of
//! each (`GET /url?url=...`, which the server makes with summaly from the
//! page's Open Graph and other tags). Which addresses get one is decided by
//! the MFM parser (mfm-js) and `extractUrlFromMfm` in Misskey's web client:
//!
//! - an address written as it is (`https://example.com/a`), or in angle
//!   brackets (`<https://example.com/a>`), gets a preview;
//! - a link with a label (`[label](https://example.com/a)`) gets one too;
//! - a silent link (`?[label](https://example.com/a)`) does not;
//! - an address in code (`` `...` ``, a ```` ``` ```` block), in
//!   `<plain>`, or in math is not a link at all.
//!
//! This module follows mfm-js's rules for those forms: an address is
//! `http://` or `https://` and then the characters `[.,a-z0-9_/:%#@$&?!~=+-]`
//! (and brackets that pair up), without a `.` or `,` at its end.

/// How a link is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// `https://...` as it is.
    Bare,
    /// `<https://...>`.
    Angle,
    /// `[label](https://...)`, or `?[label](...)` when `silent`.
    Labeled { silent: bool },
}

/// A link in the text: where it is (byte offsets of the whole of it,
/// label and brackets included), its address, and how it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub start: usize,
    pub end: usize,
    pub url: String,
    pub form: Form,
}

/// How deep brackets in an address may nest, as mfm-js's default nesting
/// limit.
const NEST_LIMIT: usize = 20;

/// Every link in `text`, in order.
pub fn links(text: &str) -> Vec<Link> {
    let b = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let line_start = i == 0 || b[i - 1] == b'\n';
        if let Some(end) = skipped(b, i, line_start) {
            i = end;
            continue;
        }
        if let Some(link) = labeled(b, i).or_else(|| angle(b, i)).or_else(|| bare(b, i)) {
            i = link.end;
            found.push(link);
            continue;
        }
        // One character on: a byte inside a multi-byte one never starts
        // any of the forms above, which are all ASCII.
        i += 1;
    }
    found
}

/// The end of what starts at `i` and holds no link: code, `<plain>`, math.
fn skipped(b: &[u8], i: usize, line_start: bool) -> Option<usize> {
    let rest = &b[i..];
    if line_start && rest.starts_with(b"```") {
        // A block runs to a line that starts with ``` again.
        let body = i + 3;
        let close = find(b, body, b"\n```")?;
        return Some(close + 4);
    }
    if rest.starts_with(b"`") {
        let close = find_on_line(b, i + 1, b"`")?;
        return (close > i + 1).then_some(close + 1);
    }
    if rest.starts_with(b"<plain>") {
        let close = find(b, i + 7, b"</plain>")?;
        return Some(close + 8);
    }
    if rest.starts_with(b"\\(") {
        let close = find_on_line(b, i + 2, b"\\)")?;
        return Some(close + 2);
    }
    if line_start && rest.starts_with(b"\\[") {
        let close = find(b, i + 2, b"\\]")?;
        return Some(close + 2);
    }
    None
}

fn find(b: &[u8], from: usize, what: &[u8]) -> Option<usize> {
    b.get(from..)?
        .windows(what.len())
        .position(|w| w == what)
        .map(|p| from + p)
}

fn find_on_line(b: &[u8], from: usize, what: &[u8]) -> Option<usize> {
    let at = find(b, from, what)?;
    (!b[from..at].contains(&b'\n')).then_some(at)
}

fn is_url_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b".,_/:%#@$&?!~=+-".contains(&c)
}

fn scheme_len(b: &[u8], i: usize) -> Option<usize> {
    let rest = &b[i..];
    if rest.starts_with(b"https://") {
        Some(8)
    } else if rest.starts_with(b"http://") {
        Some(7)
    } else {
        None
    }
}

/// One item of an address at `i`: a URL character, or a bracket pair with
/// items in it. Its end.
fn item(b: &[u8], i: usize, depth: usize) -> Option<usize> {
    let c = *b.get(i)?;
    let close = match c {
        b'(' => b')',
        b'[' => b']',
        c if is_url_char(c) => return Some(i + 1),
        _ => return None,
    };
    if depth >= NEST_LIMIT {
        return None;
    }
    let mut j = i + 1;
    while let Some(k) = item(b, j, depth + 1) {
        j = k;
    }
    (b.get(j) == Some(&close)).then_some(j + 1)
}

/// An address as it is at `i`.
fn bare(b: &[u8], i: usize) -> Option<Link> {
    let scheme = scheme_len(b, i)?;
    let start = i + scheme;
    let mut end = start;
    while let Some(k) = item(b, end, 0) {
        end = k;
    }
    while end > start && matches!(b[end - 1], b'.' | b',') {
        end -= 1;
    }
    if end == start {
        return None;
    }
    Some(Link {
        start: i,
        end,
        url: String::from_utf8_lossy(&b[i..end]).into_owned(),
        form: Form::Bare,
    })
}

/// `<https://...>` at `i`: an address up to `>`, with no space in it.
fn angle(b: &[u8], i: usize) -> Option<Link> {
    if b.get(i) != Some(&b'<') {
        return None;
    }
    scheme_len(b, i + 1)?;
    let mut j = i + 1;
    while let Some(&c) = b.get(j) {
        if c == b'>' {
            break;
        }
        if c.is_ascii_whitespace() {
            return None;
        }
        j += 1;
    }
    if b.get(j) != Some(&b'>') || j == i + 1 + scheme_len(b, i + 1)? {
        return None;
    }
    Some(Link {
        start: i,
        end: j + 1,
        url: String::from_utf8_lossy(&b[i + 1..j]).into_owned(),
        form: Form::Angle,
    })
}

/// `[label](address)` or `?[label](address)` at `i`. The label ends at
/// the first `]` on its line; the address is one as it is or in angle
/// brackets, followed by `)`.
fn labeled(b: &[u8], i: usize) -> Option<Link> {
    let (silent, open) = if b[i..].starts_with(b"?[") {
        (true, i + 2)
    } else if b[i] == b'[' {
        (false, i + 1)
    } else {
        return None;
    };
    let close = label_end(b, open)?;
    if close == open || b.get(close + 1) != Some(&b'(') {
        return None;
    }
    let at = close + 2;
    let target = angle(b, at).or_else(|| bare(b, at))?;
    if b.get(target.end) != Some(&b')') {
        return None;
    }
    Some(Link {
        start: i,
        end: target.end + 1,
        url: target.url,
        form: Form::Labeled { silent },
    })
}

/// The `]` that ends a label starting at `from`, on the same line; a
/// `<plain>` in the label is taken whole, `]` and all.
fn label_end(b: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    loop {
        match *b.get(j)? {
            b']' => return Some(j),
            b'\n' => return None,
            b'<' if b[j..].starts_with(b"<plain>") => {
                let close = find_on_line(b, j + 7, b"</plain>")?;
                j = close + 8;
            }
            _ => j += 1,
        }
    }
}

/// The addresses Misskey's web client shows a preview under, in order:
/// every link that is not silent, each once, two that differ only after
/// `#` counted as one, and not `skip` (the address of the note quoted,
/// which the client leaves out).
pub fn previewed(text: &str, skip: &[&str]) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for l in links(text) {
        if matches!(l.form, Form::Labeled { silent: true }) || skip.contains(&l.url.as_str()) {
            continue;
        }
        let key = without_hash(&l.url);
        if !urls.iter().any(|u| without_hash(u) == key) {
            urls.push(l.url);
        }
    }
    urls
}

/// The address without what follows its last `#`, as the client compares
/// them.
pub fn without_hash(url: &str) -> &str {
    url.rfind('#').map_or(url, |i| &url[..i])
}

/// `text` with every link made silent, so Misskey's web client shows no
/// preview under it, and the text reads as before: an address becomes
/// `?[<plain>address</plain>](address)` (the `<plain>` keeps the address
/// in the label from being read as MFM), and `[label](address)` becomes
/// `?[label](address)`.
pub fn silenced(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut last = 0;
    for l in links(text) {
        out.push_str(&text[last..l.start]);
        match l.form {
            Form::Bare => out.push_str(&format!("?[<plain>{0}</plain>]({0})", l.url)),
            Form::Angle => out.push_str(&format!("?[<plain>{0}</plain>](<{0}>)", l.url)),
            Form::Labeled { silent: false } => {
                out.push('?');
                out.push_str(&text[l.start..l.end]);
            }
            Form::Labeled { silent: true } => out.push_str(&text[l.start..l.end]),
        }
        last = l.end;
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn urls(text: &str) -> Vec<String> {
        links(text).into_iter().map(|l| l.url).collect()
    }

    // The address ends where mfm-js ends it: at a character it does not
    // take (a space, CJK, an emoji, a quote), at a bracket that does not
    // pair up, and without a `.` or `,` at its end.
    #[rstest]
    #[case("見て https://debimate.jp/post/a/ です", "https://debimate.jp/post/a/")]
    #[case("https://example.com/a.", "https://example.com/a")]
    #[case("https://example.com/a,b,", "https://example.com/a,b")]
    #[case("(https://example.com/a)", "https://example.com/a")]
    #[case(
        "https://ja.wikipedia.org/wiki/A_(B)",
        "https://ja.wikipedia.org/wiki/A_(B)"
    )]
    #[case("https://example.com/日本", "https://example.com/")]
    #[case("https://example.com/🐈‍⬛", "https://example.com/")]
    #[case(
        "http://127.0.0.1:8080/x?y=1&z=2#t",
        "http://127.0.0.1:8080/x?y=1&z=2#t"
    )]
    #[case("\"https://example.com/q\"", "https://example.com/q")]
    fn an_address_ends_where_mfm_ends_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(urls(text), [want]);
    }

    #[rstest]
    #[case("no link here 🇯🇵")]
    #[case("https://")]
    #[case("https://.")]
    #[case("HTTPS://example.com")]
    #[case("ftp://example.com")]
    #[case("`https://example.com`")]
    #[case("```\nhttps://example.com\n```")]
    #[case("<plain>https://example.com</plain>")]
    #[case("\\(https://example.com\\)")]
    fn what_mfm_does_not_read_as_a_link_is_none(#[case] text: &str) {
        assert_eq!(urls(text), Vec::<String>::new(), "{text}");
    }

    #[test]
    fn each_form_is_told() {
        let got: Vec<(String, Form)> = links(
            "a https://a.test b <https://b.test/x(y> c [ラベル](https://c.test) d ?[s](<https://d.test>)",
        )
        .into_iter()
        .map(|l| (l.url, l.form))
        .collect();
        assert_eq!(
            got,
            [
                ("https://a.test".to_string(), Form::Bare),
                ("https://b.test/x(y".to_string(), Form::Angle),
                (
                    "https://c.test".to_string(),
                    Form::Labeled { silent: false }
                ),
                ("https://d.test".to_string(), Form::Labeled { silent: true }),
            ]
        );
    }

    // A bracket that opens no link is text, and an address in it is still
    // found.
    #[test]
    fn a_bracket_that_is_no_link_is_text() {
        assert_eq!(urls("[メモ] https://a.test"), ["https://a.test"]);
        assert_eq!(urls("[x](not a url) https://a.test"), ["https://a.test"]);
        assert_eq!(urls("[x\n](https://a.test)"), ["https://a.test"]);
    }

    // Code that never closes is not code: the address after it is a link.
    #[test]
    fn code_that_never_closes_is_text() {
        assert_eq!(urls("`a https://a.test"), ["https://a.test"]);
        assert_eq!(urls("```\nhttps://a.test"), ["https://a.test"]);
    }

    // Deeply nested brackets stop the address without overflowing.
    #[test]
    fn deep_brackets_end_the_address() {
        let text = format!("https://a.test/{}", "(".repeat(10_000));
        assert_eq!(urls(&text), ["https://a.test/"]);
    }

    #[test]
    fn previews_skip_silent_links_repeats_and_the_quoted_note() {
        let text = "https://a.test/#1 ?[s](https://s.test) https://a.test/#2 [l](https://b.test) https://q.test/notes/x https://b.test";
        assert_eq!(
            previewed(text, &["https://q.test/notes/x"]),
            ["https://a.test/#1", "https://b.test"]
        );
    }

    // Silenced, the text shows no preview, reads as it did, and has the
    // same addresses in the same order.
    #[rstest]
    #[case(
        "新しい記事 https://debimate.jp/post/a/ です",
        "新しい記事 ?[<plain>https://debimate.jp/post/a/</plain>](https://debimate.jp/post/a/) です"
    )]
    #[case(
        "<https://a.test/x>",
        "?[<plain>https://a.test/x</plain>](<https://a.test/x>)"
    )]
    #[case("[記事](https://a.test)。", "?[記事](https://a.test)。")]
    #[case("?[記事](https://a.test)", "?[記事](https://a.test)")]
    #[case("`https://a.test` 👍🏽", "`https://a.test` 👍🏽")]
    #[case(
        "https://a.test/[1]",
        "?[<plain>https://a.test/[1]</plain>](https://a.test/[1])"
    )]
    fn silenced_links_show_no_preview(#[case] text: &str, #[case] want: &str) {
        let s = silenced(text);
        assert_eq!(s, want);
        assert!(previewed(&s, &[]).is_empty(), "{s}");
        assert_eq!(urls(&s), urls(text));
    }
}
