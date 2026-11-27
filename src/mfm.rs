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
    scan(text.as_bytes()).0
}

/// Every link in `b`, and where each `[` that opens neither a link nor a
/// function is, and each `<plain>` that never closes (outside code,
/// `<plain>` and math, as the links are).
fn scan(b: &[u8]) -> (Vec<Link>, Vec<usize>) {
    let mut found = Vec::new();
    let mut strays = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let line_start = i == 0 || b[i - 1] == b'\n';
        if let Some(end) = skipped(b, i, line_start) {
            i = end;
            continue;
        }
        // A function's `[` opens no label: mfm-js reads the function first.
        // Its content is read on for links.
        if b[i..].starts_with(b"$[")
            && let Some((_, content)) = function(b, i)
            && closing_bracket(b, content).is_some()
        {
            i = content;
            continue;
        }
        if let Some(link) = labeled(b, i).or_else(|| angle(b, i)).or_else(|| bare(b, i)) {
            i = link.end;
            found.push(link);
            continue;
        }
        // `skipped` takes a `<plain>` that closes, and `angle` an address
        // whose `>` comes, so one here does not.
        if b[i] == b'['
            || b[i..].starts_with(b"<plain>")
            || (b[i] == b'<' && scheme_len(b, i + 1).is_some())
        {
            strays.push(i);
        }
        // One character on: a byte inside a multi-byte one never starts
        // any of the forms above, which are all ASCII.
        i += 1;
    }
    (found, strays)
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
///
/// A `[` before a link on its line that no `]` closes before the link
/// would take the `](` added after the address as the end of its own label
/// and make a link that is not silent, so it is written `<plain>[</plain>`,
/// which reads as the same `[`. A `<plain>` that never closes would be
/// closed by the `</plain>` added after it, so it is written
/// `<plain><</plain>plain>`, which reads as the same `<plain>`; so is the
/// `<` of a `<https://` with no `>` that the `>` of `<plain>` would close.
/// A link
/// right after `http://` gets an empty `<plain></plain>` before its `?`,
/// which would otherwise make `http://?` an address.
pub fn silenced(text: &str) -> String {
    let b = text.as_bytes();
    let (links, strays) = scan(b);
    let mut out = String::with_capacity(text.len() + 64);
    let mut last = 0;
    let mut strays = strays.into_iter().peekable();
    for l in links {
        while let Some(at) = strays.next_if(|at| *at < l.start) {
            if b[at] == b'<' {
                // A `<https://` with a space before the link cannot reach
                // the `>` added after it.
                let open = b[at..].starts_with(b"<plain>")
                    || !b[at..l.start].iter().any(u8::is_ascii_whitespace);
                if open {
                    out.push_str(&text[last..at]);
                    out.push_str("<plain><</plain>");
                    last = at + 1;
                }
                continue;
            }
            let open = label_end(b, at + 1);
            let line_ends = b[at..l.start].contains(&b'\n');
            if !line_ends && open.is_none_or(|close| close >= l.start) {
                out.push_str(&text[last..at]);
                out.push_str("<plain>[</plain>");
                last = at + 1;
            }
        }
        out.push_str(&text[last..l.start]);
        if l.form != (Form::Labeled { silent: true }) && joins_address(out.as_bytes()) {
            out.push_str("<plain></plain>");
        }
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

/// Whether the `?` that starts a silent link would be taken into an
/// address that `out` ends in (`http://` before `[label](...)` is no link,
/// but `http://?` is). Such a link is put after an empty `<plain>`.
fn joins_address(out: &[u8]) -> bool {
    let mut k = out.len();
    while k > 0 && (is_url_char(out[k - 1]) || b"()[]".contains(&out[k - 1])) {
        k -= 1;
    }
    let mut tail = out[k..].to_vec();
    let q = tail.len();
    tail.push(b'?');
    (0..q).any(|p| bare(&tail, p).is_some_and(|l| l.end > q))
}

/// A run of a note's text as the client draws it: its characters, and
/// whether MFM makes them bold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
}

/// `text` as plain text, the way Misskey's own client shows what MFM
/// draws, less the drawing: a function (`$[x2 ...]`, `$[tada ...]`,
/// `$[fg.color=f00 ...]`, nested ones too) is its content, `$[ruby base
/// reading]` is `base(reading)`, `<center>`, `<small>`, `<i>`, `<s>`,
/// `<sup>` and `<sub>` are what they hold, `<b>` and `**bold**` are bold,
/// a link with a label (`[label](address)`, `?[label](address)`) is its
/// label, `<https://...>` is the address without its brackets, and
/// `<plain>` is what it holds as it is. Code, math, custom
/// emoji (`:name:`) and anything that does not close are left as written.
/// The text never grows: markup is only taken away.
pub fn plain(text: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    render(text, false, 0, &mut runs);
    runs
}

/// [`plain`] without the bold: the text alone.
pub fn plain_text(text: &str) -> String {
    plain(text).into_iter().map(|r| r.text).collect()
}

/// Add `s` to `runs`, joined to the last run when it is of the same style.
fn push(runs: &mut Vec<Run>, s: &str, bold: bool) {
    if s.is_empty() {
        return;
    }
    match runs.last_mut() {
        Some(r) if r.bold == bold => r.text.push_str(s),
        _ => runs.push(Run {
            text: s.to_string(),
            bold,
        }),
    }
}

/// The tags whose content is drawn as it is, bold for `<b>`.
const TAGS: &[(&str, bool)] = &[
    ("center", false),
    ("small", false),
    ("i", false),
    ("s", false),
    ("sup", false),
    ("sub", false),
    ("b", true),
];

/// Draw `s`, the whole text at depth 0 and some markup's content below it.
/// Content always comes after the markup's opening on the same line, so
/// only the whole text starts a line, where a code block or math block
/// may open.
fn render(s: &str, bold: bool, depth: usize, runs: &mut Vec<Run>) {
    let b = s.as_bytes();
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        let line_start = if i == 0 {
            depth == 0
        } else {
            b[i - 1] == b'\n'
        };
        let found: Drawn<'_> = if b[i..].starts_with(b"<plain>") {
            find(b, i + 7, b"</plain>").map(|close| {
                let inner = &s[i + 7..close];
                (
                    close + 8,
                    Box::new(move |r: &mut Vec<Run>| push(r, inner, bold))
                        as Box<dyn FnOnce(&mut Vec<Run>)>,
                )
            })
        } else if let Some(end) = skipped(b, i, line_start) {
            let kept = &s[i..end];
            Some((end, Box::new(move |r: &mut Vec<Run>| push(r, kept, bold))))
        } else if depth < NEST_LIMIT {
            special(s, i, bold, depth)
        } else {
            None
        };
        match found {
            Some((end, draw)) => {
                push(runs, &s[last..i], bold);
                draw(runs);
                i = end;
                last = end;
            }
            None => {
                // One character on: every form starts with an ASCII byte.
                i += 1;
                while i < b.len() && !s.is_char_boundary(i) {
                    i += 1;
                }
            }
        }
    }
    push(runs, &s[last..], bold);
}

type Drawn<'a> = Option<(usize, Box<dyn FnOnce(&mut Vec<Run>) + 'a>)>;

/// The MFM at `i` that draws its content otherwise than as written: where
/// it ends, and how to draw it.
fn special<'a>(s: &'a str, i: usize, bold: bool, depth: usize) -> Drawn<'a> {
    let b = s.as_bytes();
    if b[i..].starts_with(b"$[") {
        let (name_end, content) = function(b, i)?;
        let close = closing_bracket(b, content)?;
        let name = &s[i + 2..name_end];
        let inner = &s[content..close];
        let ruby = name == "ruby";
        return Some((
            close + 1,
            Box::new(
                move |r: &mut Vec<Run>| match inner.split_once(' ').filter(|_| ruby) {
                    Some((base, reading)) => {
                        render(base, bold, depth + 1, r);
                        push(r, "(", bold);
                        render(reading, bold, depth + 1, r);
                        push(r, ")", bold);
                    }
                    None => render(inner, bold, depth + 1, r),
                },
            ),
        ));
    }
    if b[i..].starts_with(b"**") {
        let close = closing(b, i + 2, b"**").filter(|c| *c > i + 2)?;
        let inner = &s[i + 2..close];
        return Some((
            close + 2,
            Box::new(move |r: &mut Vec<Run>| render(inner, true, depth + 1, r)),
        ));
    }
    if b[i] == b'<' {
        // `<https://...>` is the address alone, as Misskey's web client
        // shows it and as its silenced form reads.
        if let Some(link) = angle(b, i) {
            let url = &s[i + 1..link.end - 1];
            return Some((
                link.end,
                Box::new(move |r: &mut Vec<Run>| push(r, url, bold)),
            ));
        }
        for (tag, makes_bold) in TAGS {
            let open = format!("<{tag}>");
            if !b[i..].starts_with(open.as_bytes()) {
                continue;
            }
            let close_tag = format!("</{tag}>");
            let close = closing_tag(b, i + open.len(), open.as_bytes(), close_tag.as_bytes())?;
            let inner = &s[i + open.len()..close];
            let style = bold || *makes_bold;
            return Some((
                close + close_tag.len(),
                Box::new(move |r: &mut Vec<Run>| render(inner, style, depth + 1, r)),
            ));
        }
        return None;
    }
    if b[i] == b'[' || b[i..].starts_with(b"?[") {
        let link = labeled(b, i)?;
        let open = if b[i] == b'?' { i + 2 } else { i + 1 };
        let label_close = label_end(b, open)?;
        let label = &s[open..label_close];
        return Some((
            link.end,
            Box::new(move |r: &mut Vec<Run>| render(label, bold, depth + 1, r)),
        ));
    }
    // An address is drawn as written: the brackets it takes in may hold
    // what reads as MFM elsewhere (`https://a.test/([x](https://b.test))`),
    // as in mfm-js, and as its silenced form, in `<plain>`, reads.
    if let Some(link) = bare(b, i) {
        let url = &s[i..link.end];
        return Some((
            link.end,
            Box::new(move |r: &mut Vec<Run>| push(r, url, bold)),
        ));
    }
    None
}

/// For `$[name.args ` at `i`: where the name ends, and where the content
/// starts, after the space. A name is letters, digits and `_`; the
/// arguments after a `.` are anything up to the space but a bracket.
fn function(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut j = i + 2;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    let name_end = j;
    if name_end == i + 2 {
        return None;
    }
    if b.get(j) == Some(&b'.') {
        while j < b.len() && !matches!(b[j], b' ' | b'\n' | b'[' | b']') {
            j += 1;
        }
    }
    (b.get(j) == Some(&b' ')).then_some((name_end, j + 1))
}

/// The `]` that closes a function whose content starts at `from`: a
/// function in it is taken whole, its own `]` with it, and a `]` in a link
/// or in `<plain>` is theirs ([`atom_end`]). A `$[` that starts no function
/// (`$[100]`) is text, as in mfm-js, so its `]` can close the one outside.
fn closing_bracket(b: &[u8], from: usize) -> Option<usize> {
    closing_bracket_at(b, from, 0)
}

fn closing_bracket_at(b: &[u8], from: usize, depth: usize) -> Option<usize> {
    let mut j = from;
    while j < b.len() {
        if depth < NEST_LIMIT
            && b[j..].starts_with(b"$[")
            && let Some((_, content)) = function(b, j)
        {
            // A function in it that never closes leaves no `]` after it
            // for this one either: stopping here keeps unclosed nesting
            // from being scanned again at every level.
            j = closing_bracket_at(b, content, depth + 1)? + 1;
            continue;
        }
        if let Some(end) = atom_end(b, j) {
            j = end;
            continue;
        }
        if b[j] == b']' {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// The end of what at `j` is read whole inside markup, so that nothing in
/// it closes the markup: a link of any form, or what holds no link (code,
/// `<plain>`, math), as [`links`] reads past them. Silencing turns
/// one form of link into another, and a `]`, `**` or `</tag>` read inside
/// one would close the markup somewhere else after it.
fn atom_end(b: &[u8], j: usize) -> Option<usize> {
    let line_start = j == 0 || b[j - 1] == b'\n';
    if let Some(end) = skipped(b, j, line_start) {
        return Some(end);
    }
    labeled(b, j)
        .or_else(|| angle(b, j))
        .or_else(|| bare(b, j))
        .map(|l| l.end)
}

/// The first `what` from `from` outside what [`atom_end`] reads whole.
fn closing(b: &[u8], from: usize, what: &[u8]) -> Option<usize> {
    let mut j = from;
    while j < b.len() {
        if let Some(end) = atom_end(b, j) {
            j = end;
        } else if b[j..].starts_with(what) {
            return Some(j);
        } else {
            j += 1;
        }
    }
    None
}

/// The `</tag>` that closes a `<tag>` whose content starts at `from`, the
/// same tag opened inside closed first.
fn closing_tag(b: &[u8], from: usize, open: &[u8], close: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    let mut j = from;
    while j < b.len() {
        if let Some(end) = atom_end(b, j) {
            j = end;
        } else if b[j..].starts_with(open) {
            depth += 1;
            j += open.len();
        } else if b[j..].starts_with(close) {
            if depth == 0 {
                return Some(j);
            }
            depth -= 1;
            j += close.len();
        } else {
            j += 1;
        }
    }
    None
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
        assert_eq!(plain_text(&s), plain_text(text), "{s}");
    }

    // A `[` before a link on its line that closes nothing, or a function's
    // `$[`, does not take the `](` silencing adds as the end of a label of
    // its own, which made a link that is not silent and kept the preview.
    #[rstest]
    #[case("https://example.com/wiki/Foo_(bar)_[baz. https://example.com/((((x))))")]
    #[case("[baz. https://a.test")]
    #[case("[メモ 🇯🇵 https://a.test と https://b.test")]
    #[case("$[x2 https://a.test]")]
    #[case("$[x2 https://a.test")]
    #[case("$[x2 [baz https://a.test]")]
    #[case("$[x2 $[tada 👨\u{200d}👩\u{200d}👧 https://a.test]] [x](https://b.test)")]
    #[case("`[` [ https://a.test")]
    #[case(
        "$[x2  http$[x3 $[x4 \0\0\0\0\0\0\0\0\0\0\0$[flip $[spin deep]]]]] ?[<plain>]</plai]>n(https://example.com/p)"
    )]
    #[case("<plain>閉じない 👍🏽\nhttps://a.test")]
    #[case("http://[x3 flip $[spin deplain>](https://example.com/p)")]
    #[case("見てhttps://<https://a.test>")]
    #[case(
        "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0<https://\0\0\0\0\0\0\0\0\0\0\0\0\0<https://expsb np\n["
    )]
    #[case("<https://🇯🇵https://a.test 後")]
    #[case("$[x ```\n[a https://in.block/\n```]")]
    fn a_bracket_before_a_silenced_link_does_not_bring_its_preview_back(#[case] text: &str) {
        silencing_keeps_the_text(text);
    }

    // Markup closes where it did before silencing: a `]`, `**` or `</tag>`
    // in an address, which silencing writes in a label and `<plain>`, is
    // the address's.
    #[rstest]
    #[case(
        "$[x2  http$https://example.com/wiki/Foo_(bar)_[baz] https://example.com/((((x))))]]]$[flip $[spin deep]]]]]example.com/p)"
    )]
    #[case("$[x2 https://a.test/[1]] 後")]
    #[case("**太字 <https://a.test/**> まだ**")]
    #[case("<small>小 <https://a.test/</small>> 🇯🇵</small>")]
    #[case(
        "https://example.com/wiki/Foo_(bar)_[baz] https://examp\u{1}\u{5}le.com/(https://e[label](https://example(((x))))"
    )]
    #[case("見て https://a.test/([x](https://b.test)) 👨\u{200d}👩\u{200d}👧")]
    #[case("$[f $$[shttp://lnm]nom]-")]
    #[case("`[iht`$[flip $` <pl<plain>```\nhttps://in.")]
    #[case("$[x2 `]` 漢字] https://a.test")]
    #[case("$[x2 $[price https://a.test] 🇯🇵]")]
    fn markup_closes_where_it_did_before_silencing(#[case] text: &str) {
        silencing_keeps_the_text(text);
    }

    fn silencing_keeps_the_text(text: &str) {
        let s = silenced(text);
        assert_eq!(previewed(&s, &[]), Vec::<String>::new(), "{s}");
        assert_eq!(urls(&s), urls(text), "{s}");
        assert_eq!(plain_text(&s), plain_text(text), "{s}");
    }

    // The `[` of a function is not a label's: the link in the function
    // starts at its own bracket.
    #[test]
    fn a_function_bracket_opens_no_link() {
        let got = links("$[x2 [a](https://a.test)]");
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].start, got[0].end), (5, 24));
    }

    fn runs(text: &str) -> Vec<(String, bool)> {
        plain(text).into_iter().map(|r| (r.text, r.bold)).collect()
    }

    #[rstest]
    #[case("$[x2 大きい]", "大きい")]
    #[case("$[tada.speed=2s :blobcat:]", ":blobcat:")]
    #[case("$[fg.color=f00 赤い$[bg.color=0f0 緑の上]文字]", "赤い緑の上文字")]
    #[case("$[spin $[x3 $[tada 🎉]]]!", "🎉!")]
    #[case("$[ruby 漢字 かんじ]を読む", "漢字(かんじ)を読む")]
    #[case("<center>まんなか</center>", "まんなか")]
    #[case("<small>小さく<i>斜め</i></small>", "小さく斜め")]
    #[case("<center><center>二重</center></center>", "二重")]
    #[case(
        "?[ラベル](https://example.com/a) と [見出し](https://example.com/b)",
        "ラベル と 見出し"
    )]
    #[case(
        "<plain>$[x2 そのまま] **太くない**</plain>",
        "$[x2 そのまま] **太くない**"
    )]
    #[case("`$[x2 code]` は $[x2 そのまま]", "`$[x2 code]` は そのまま")]
    #[case("$[x2 [label](https://example.com/a)]", "label")]
    #[case(":blobcat: :custom_emoji_name:", ":blobcat: :custom_emoji_name:")]
    #[case("見て <https://a.test/x(y> 👍🏽", "見て https://a.test/x(y 👍🏽")]
    #[case("$[x2 $[ab:c] 後]", "$[ab:c 後]")]
    #[case("$[x2 `]` 後]", "`]` 後")]
    #[case("$[x2 ```\n**太字**\n```]", "```\n太字\n```")]
    #[case("<https://a.test/x >", "<https://a.test/x >")]
    #[case("`<https://a.test>`", "`<https://a.test>`")]
    #[case(
        "家族👨\u{200d}👩\u{200d}👧$[x2 🇯🇵👍🏽❤\u{fe0f}]",
        "家族👨\u{200d}👩\u{200d}👧🇯🇵👍🏽❤\u{fe0f}"
    )]
    fn mfm_is_drawn_as_its_text(#[case] text: &str, #[case] want: &str) {
        assert_eq!(plain_text(text), want);
    }

    // What does not close, or is not a function, is left as written.
    #[rstest]
    #[case("$[x2 閉じない")]
    #[case("$[x2]")]
    #[case("$[ 名前なし]")]
    #[case("$[x2\n改行]")]
    #[case("<center>閉じない")]
    #[case("**片方だけ")]
    #[case("****")]
    #[case("price is $[100]")]
    #[case("[label](not an address)")]
    #[case("$[")]
    fn mfm_that_does_not_close_is_left_as_written(#[case] text: &str) {
        assert_eq!(plain_text(text), text);
    }

    #[test]
    fn bold_is_marked_and_the_rest_is_not() {
        assert_eq!(
            runs("前**太い$[x2 中も]**後<b>タグ</b>"),
            [
                ("前".to_string(), false),
                ("太い中も".to_string(), true),
                ("後".to_string(), false),
                ("タグ".to_string(), true),
            ]
        );
    }

    // Notes of misskey.io's local timeline, as they were written there.
    #[test]
    fn real_notes_are_drawn_as_their_text() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("testdata/mfm_samples.json")).unwrap();
        let got: Vec<String> = v["texts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| plain_text(t.as_str().unwrap()))
            .collect();
        assert_eq!(
            got,
            [
                "アイドルとして見せられない(バレたらおわる)もの:blobcatphoto_frustration:",
                "misskey.ioに新しい絵文字 :anagi_cheer_dance: (`anagi_cheer_dance`)が追加されたかも。\n:anagi_cheer_dance:",
                ":anagi_cheer_dance::blobcat_yay:",
                "じゃあ強化ペネ持続重ねするね…:blobcat_finger_beam:",
                "セブンの春巻きう、うましぎ～～！🫨\n───常設化しろ。",
            ]
        );
    }

    // On random mixes of MFM's pieces, emoji and CJK: nothing panics, the
    // text never grows, every character drawn is one of the text's, in
    // order, and markup nested past mfm-js's limit is left as written.
    #[test]
    fn plain_never_panics_and_never_grows() {
        const PIECES: &[&str] = &[
            "$[",
            "x2 ",
            "tada.speed=1s ",
            "ruby ",
            "]",
            "[",
            "](https://e.test/a)",
            "?[",
            "**",
            "<center>",
            "</center>",
            "<small>",
            "</small>",
            "<b>",
            "</b>",
            "<plain>",
            "</plain>",
            "`",
            "\n",
            " ",
            "a",
            "漢字",
            "👨\u{200d}👩\u{200d}👧",
            "🇯🇵",
            "👍🏽",
            ":blobcat:",
            "(",
            ")",
            "\\(",
            "\\)",
            "```\n",
        ];
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        for _ in 0..50_000 {
            let mut text = String::new();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            for _ in 0..(x % 16) {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                text.push_str(PIECES[(x % PIECES.len() as u64) as usize]);
            }
            let out = plain_text(&text);
            assert!(
                out.chars().count() <= text.chars().count(),
                "{text:?} -> {out:?}"
            );
            // Every character but the parentheses a ruby adds is one of
            // the text's, in order.
            let mut rest = text.chars();
            for c in out.chars().filter(|c| !matches!(c, '(' | ')')) {
                assert!(rest.any(|t| t == c), "{text:?} -> {out:?}: {c:?}");
            }
        }
        let deep = format!("{}x{}", "$[x2 ".repeat(40), "]".repeat(40));
        assert!(plain_text(&deep).chars().count() <= deep.chars().count());
    }
}
