//! A note's MFM, as Misskey reads it.
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
//!   `<plain>`, in math, or in markup that never closes is not a link at all.
//!
//! This module reads a note with the grammar of mfm-js 0.28.0, the version
//! Misskey's web client uses, rule for rule and in the same order: the
//! same nesting limit (20) and the same budget of nested reads (twenty per
//! character of the text and a thousand), and markup that opens but never
//! closes (`**bold`, `$[x2 big`) is text as written, all of it, as mfm-js's
//! `seqOrText` makes it. Emoji are read as text, but for the keycaps that
//! start with an ASCII character (`#️⃣`), which mfm-js takes before a
//! hashtag.

use std::collections::{HashMap, HashSet};

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

/// How deep markup may nest, as mfm-js's default nesting limit.
const NEST_LIMIT: usize = 20;

// ---------------------------------------------------------------------
// The tree
// ---------------------------------------------------------------------

/// What draws its content in a style of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// `**...**`, `<b>...</b>`, `__...__`.
    Bold,
    /// `***...***`, which mfm-js reads as `$[tada ...]`.
    Big,
    Small,
    /// `<i>...</i>`, `*...*`, `_..._`.
    Italic,
    /// `<s>...</s>`, `~~...~~`.
    Strike,
}

/// A node of a note's MFM, with byte offsets into the text it was read
/// from (a quote's content for the nodes in a quote).
#[derive(Debug, Clone)]
enum Node {
    /// Text; `raw` when it is markup that never closed, as written.
    Text {
        s: usize,
        e: usize,
        raw: bool,
    },
    /// `<plain>...</plain>`: what it holds is `cs..ce`.
    Plain {
        cs: usize,
        ce: usize,
    },
    /// An address, `<...>` around it when `brackets`.
    Url {
        s: usize,
        e: usize,
        brackets: bool,
    },
    /// `[label](address)` or `?[label](address)`.
    Link {
        s: usize,
        e: usize,
        silent: bool,
        url: Box<Node>,
        label: Vec<Node>,
    },
    /// `$[name.args content]`: the name is `s + 2..name_end`, the arguments
    /// `name_end..head_end - 1`.
    Fn {
        s: usize,
        name_end: usize,
        head_end: usize,
        args: Vec<(String, Option<String>)>,
        children: Vec<Node>,
    },
    Styled {
        style: Style,
        children: Vec<Node>,
    },
    /// `<center>...</center>`, with the line breaks it takes before and
    /// after it.
    Center {
        lead: (usize, usize),
        trail: (usize, usize),
        children: Vec<Node>,
    },
    /// Code, math, a search box, a mention, a hashtag, a custom emoji or
    /// a keycap: shown as written.
    Kept {
        s: usize,
        e: usize,
    },
    /// `> ...`: its lines without the `>`, read again as a note.
    Quote {
        lead: (usize, usize),
        trail: (usize, usize),
        content: String,
        /// Where each line of `content` starts, and where in the text it
        /// came from.
        lines: Vec<(usize, usize)>,
        /// What each line starts with: `>` and the space after it.
        marks: Vec<String>,
        children: Vec<Node>,
    },
}

/// What a rule reads: a character of text, text as written, or a node.
enum Item {
    Text(usize, usize, bool),
    Node(Node),
}

/// The state mfm-js keeps while it reads: how deep it is, whether it is in
/// a link's label, the budget left, and the caches of `manyThen` (which
/// make a label or a center that failed fail again at once).
struct State {
    depth: usize,
    link_label: bool,
    budget: i64,
    inputs: Vec<String>,
    failures: HashMap<(u8, usize, usize), HashSet<usize>>,
}

/// The text being read, and its index among [`State::inputs`].
#[derive(Clone, Copy)]
struct Cx<'a> {
    s: &'a str,
    b: &'a [u8],
    id: usize,
}

type Res = Option<(usize, Item)>;

const RULE_LINK: u8 = 0;
const RULE_CENTER: u8 = 1;

fn starts(b: &[u8], i: usize, what: &[u8]) -> bool {
    b.get(i..).is_some_and(|r| r.starts_with(what))
}

/// The end of the character at `i`.
fn next_char(cx: Cx<'_>, i: usize) -> Option<usize> {
    if i >= cx.b.len() {
        return None;
    }
    let mut j = i + 1;
    while j < cx.b.len() && !cx.s.is_char_boundary(j) {
        j += 1;
    }
    Some(j)
}

fn newline(b: &[u8], i: usize) -> Option<usize> {
    if starts(b, i, b"\r\n") {
        Some(i + 2)
    } else if matches!(b.get(i), Some(b'\r' | b'\n')) {
        Some(i + 1)
    } else {
        None
    }
}

fn opt_newline(b: &[u8], i: usize) -> usize {
    newline(b, i).unwrap_or(i)
}

fn line_begin(b: &[u8], i: usize) -> bool {
    i == 0 || matches!(b[i - 1], b'\r' | b'\n')
}

fn line_end(b: &[u8], i: usize) -> bool {
    i == b.len() || matches!(b[i], b'\r' | b'\n')
}

/// mfm-js's `space`: a space, an ideographic space or a tab.
fn space(b: &[u8], i: usize) -> Option<usize> {
    match b.get(i) {
        Some(b' ' | b'\t') => Some(i + 1),
        _ if starts(b, i, "\u{3000}".as_bytes()) => Some(i + 3),
        _ => None,
    }
}

fn alnum(b: &[u8], i: usize) -> bool {
    b.get(i).is_some_and(u8::is_ascii_alphanumeric)
}

/// mfm-js's `alphaAndNumBefore`.
fn alnum_before(b: &[u8], i: usize) -> bool {
    i > 0 && b[i - 1].is_ascii_alphanumeric()
}

/// The end of a run of the bytes `ok` takes from `i`, if there is one.
fn run(b: &[u8], i: usize, ok: impl Fn(u8) -> bool) -> Option<usize> {
    let mut j = i;
    while j < b.len() && ok(b[j]) {
        j += 1;
    }
    (j > i).then_some(j)
}

fn scheme(b: &[u8], i: usize) -> Option<usize> {
    if starts(b, i, b"https://") {
        Some(i + 8)
    } else if starts(b, i, b"http://") {
        Some(i + 7)
    } else {
        None
    }
}

fn is_url_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b".,_/:%#@$&?!~=+-".contains(&c)
}

/// mfm-js's `until`: the end of the text from `i` up to where `stop`
/// matches (or the end), if it holds `min` characters or more.
fn until(cx: Cx<'_>, i: usize, min: usize, stop: impl Fn(&[u8], usize) -> bool) -> Option<usize> {
    let mut j = i;
    let mut n = 0;
    while j < cx.b.len() && !stop(cx.b, j) {
        j = next_char(cx, j)?;
        n += 1;
    }
    (n >= min).then_some(j)
}

impl State {
    fn new(text: &str) -> State {
        State {
            depth: 0,
            link_label: false,
            budget: text.encode_utf16().count() as i64 * 20 + 1000,
            inputs: vec![text.to_string()],
            failures: HashMap::new(),
        }
    }

    fn input_id(&mut self, s: &str) -> usize {
        if let Some(i) = self.inputs.iter().position(|t| t == s) {
            return i;
        }
        self.inputs.push(s.to_string());
        self.inputs.len() - 1
    }

    /// mfm-js's `nest`: one level deeper, `p` if the budget and the depth
    /// allow it and it reads something, `fallback` otherwise.
    fn nest(
        &mut self,
        p: impl FnOnce(&mut State) -> Res,
        fallback: impl FnOnce(&mut State) -> Res,
    ) -> Res {
        self.depth += 1;
        let nestable = if self.budget <= 0 {
            false
        } else {
            self.budget -= 1;
            self.depth < NEST_LIMIT
        };
        let r = if nestable { p(self) } else { None };
        let r = r.or_else(|| fallback(self));
        self.depth -= 1;
        r
    }

    fn state_key(&self) -> usize {
        self.depth * 2 + usize::from(self.link_label)
    }

    fn failed(&self, key: (u8, usize, usize), at: usize) -> bool {
        self.failures.get(&key).is_some_and(|f| f.contains(&at))
    }
}

fn text_char(cx: Cx<'_>, i: usize) -> Res {
    next_char(cx, i).map(|e| (e, Item::Text(i, e, false)))
}

/// Items read one level deeper from `i` with `item` until `stop` matches
/// where the next would start, as mfm-js's
/// `seq(notMatch(stop), nest(item)).many(0)`.
fn items(
    st: &mut State,
    cx: Cx<'_>,
    mut j: usize,
    stop: impl Fn(&[u8], usize) -> bool,
    item: fn(&mut State, Cx<'_>, usize) -> Res,
) -> (usize, Vec<Item>) {
    let mut got = Vec::new();
    while j < cx.b.len() {
        if stop(cx.b, j) {
            break;
        }
        match st.nest(|st| item(st, cx, j), |_| text_char(cx, j)) {
            Some((k, it)) => {
                got.push(it);
                j = k;
            }
            None => break,
        }
    }
    (j, got)
}

/// mfm-js's `mergeText`: the items as nodes, the characters next to each
/// other joined into one text.
fn merge(items: Vec<Item>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for it in items {
        match it {
            Item::Text(s, e, raw) => {
                if s == e {
                    continue;
                }
                if let Some(Node::Text {
                    e: last_e,
                    raw: last_raw,
                    ..
                }) = out.last_mut()
                    && *last_e == s
                    && !*last_raw
                    && !raw
                {
                    *last_e = e;
                    continue;
                }
                out.push(Node::Text { s, e, raw });
            }
            Item::Node(n) => out.push(n),
        }
    }
    out
}

// ---------------------------------------------------------------------
// The rules, as mfm-js's `language`
// ---------------------------------------------------------------------

/// mfm-js's `full`: what a note, or a quote's content, is made of.
fn full(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let c = *b.get(i)?;
    if let Some(r) = keycap(cx, i) {
        return Some(r);
    }
    if matches!(c, b'\r' | b'\n' | b'<')
        && let Some(r) = center(st, cx, i)
    {
        return Some(r);
    }
    if c == b'<'
        && let Some(r) = angle_markup(st, cx, i)
    {
        return Some(r);
    }
    if c == b'*'
        && let Some(r) = asterisk(st, cx, i)
    {
        return Some(r);
    }
    if c == b'_'
        && let Some(r) = underscore(cx, i)
    {
        return Some(r);
    }
    if matches!(c, b'\r' | b'\n' | b'`')
        && let Some(r) = code_block(cx, i)
    {
        return Some(r);
    }
    if c == b'`'
        && let Some(r) = inline_code(cx, i)
    {
        return Some(r);
    }
    if matches!(c, b'\r' | b'\n' | b'>')
        && let Some(r) = quote(st, cx, i)
    {
        return Some(r);
    }
    if matches!(c, b'\r' | b'\n' | b'\\')
        && let Some(r) = math_block(cx, i)
    {
        return Some(r);
    }
    if c == b'\\'
        && let Some(r) = math_inline(cx, i)
    {
        return Some(r);
    }
    if let Some(r) = shared(st, cx, i, c) {
        return Some(r);
    }
    if let Some(r) = search(cx, i) {
        return Some(r);
    }
    text_char(cx, i)
}

/// mfm-js's `inline`: what markup holds.
fn inline(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let c = *b.get(i)?;
    if let Some(r) = keycap(cx, i) {
        return Some(r);
    }
    if c == b'<'
        && let Some(r) = angle_markup(st, cx, i)
    {
        return Some(r);
    }
    if c == b'*'
        && let Some(r) = asterisk(st, cx, i)
    {
        return Some(r);
    }
    if c == b'_'
        && let Some(r) = underscore(cx, i)
    {
        return Some(r);
    }
    if c == b'`'
        && let Some(r) = inline_code(cx, i)
    {
        return Some(r);
    }
    if c == b'\\'
        && let Some(r) = math_inline(cx, i)
    {
        return Some(r);
    }
    if let Some(r) = shared(st, cx, i, c) {
        return Some(r);
    }
    text_char(cx, i)
}

/// A link's label: `inline`, with links, addresses, mentions and hashtags
/// read as text.
fn label_inline(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    st.link_label = true;
    let r = inline(st, cx, i);
    st.link_label = false;
    r
}

/// The rules `full` and `inline` try in the same order after the ones
/// above: strike, function, mention, hashtag, custom emoji, link, address.
fn shared(st: &mut State, cx: Cx<'_>, i: usize, c: u8) -> Res {
    match c {
        b'~' => strike_wave(st, cx, i),
        b'$' => function(st, cx, i),
        b'@' => mention(st, cx, i),
        b'#' => hashtag(st, cx, i),
        b':' => emoji_code(cx, i),
        b'?' | b'[' => link(st, cx, i),
        b'h' => url(st, cx, i),
        _ => None,
    }
}

/// A keycap emoji (`#️⃣`, `1⃣`), the only emoji that starts with what
/// starts other markup.
fn keycap(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !b
        .get(i)
        .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'#' | b'*'))
    {
        return None;
    }
    let mut j = i + 1;
    if starts(b, j, "\u{fe0f}".as_bytes()) {
        j += 3;
    }
    if !starts(b, j, "\u{20e3}".as_bytes()) {
        return None;
    }
    Some((j + 3, Item::Node(Node::Kept { s: i, e: j + 3 })))
}

/// The tags, in mfm-js's order, and `<https://...>`.
fn angle_markup(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    tag(st, cx, i, b"<small>", b"</small>", Style::Small)
        .or_else(|| plain_tag(cx, i))
        .or_else(|| tag(st, cx, i, b"<b>", b"</b>", Style::Bold))
        .or_else(|| tag(st, cx, i, b"<i>", b"</i>", Style::Italic))
        .or_else(|| tag(st, cx, i, b"<s>", b"</s>", Style::Strike))
        .or_else(|| url_alt(st, cx, i))
}

/// `***big***`, `**bold**`, `*italic*`.
fn asterisk(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    tag(st, cx, i, b"***", b"***", Style::Big)
        .or_else(|| tag(st, cx, i, b"**", b"**", Style::Bold))
        .or_else(|| simple_mark(cx, i, b'*', Style::Italic))
}

/// `__bold__`, `_italic_`.
fn underscore(cx: Cx<'_>, i: usize) -> Res {
    bold_under(cx, i).or_else(|| simple_mark(cx, i, b'_', Style::Italic))
}

/// `open`, the items up to `close`, and `close`, as mfm-js's `seqOrText`:
/// when `close` never comes, all of it is text.
fn tag(st: &mut State, cx: Cx<'_>, i: usize, open: &[u8], close: &[u8], style: Style) -> Res {
    if !starts(cx.b, i, open) {
        return None;
    }
    let from = i + open.len();
    let (j, got) = items(st, cx, from, |b, j| starts(b, j, close), inline);
    if got.is_empty() {
        return Some((from, Item::Text(i, from, true)));
    }
    if !starts(cx.b, j, close) {
        return Some((j, Item::Text(i, j, true)));
    }
    let children = merge(got);
    Some((
        j + close.len(),
        Item::Node(Node::Styled { style, children }),
    ))
}

/// `__...__` of letters, digits and spaces.
fn bold_under(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !starts(b, i, b"__") {
        return None;
    }
    let j = letters_and_spaces(b, i + 2)?;
    if !starts(b, j, b"__") {
        return None;
    }
    let children = vec![Node::Text {
        s: i + 2,
        e: j,
        raw: false,
    }];
    Some((
        j + 2,
        Item::Node(Node::Styled {
            style: Style::Bold,
            children,
        }),
    ))
}

fn letters_and_spaces(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    loop {
        if alnum(b, j) {
            j += 1;
        } else if let Some(k) = space(b, j) {
            j = k;
        } else {
            break;
        }
    }
    (j > i).then_some(j)
}

/// `*...*` or `_..._` of letters, digits and spaces, not right after a
/// letter or digit.
fn simple_mark(cx: Cx<'_>, i: usize, mark: u8, style: Style) -> Res {
    let b = cx.b;
    if b.get(i) != Some(&mark) {
        return None;
    }
    let j = letters_and_spaces(b, i + 1)?;
    if b.get(j) != Some(&mark) || alnum_before(b, i) {
        return None;
    }
    let children = vec![Node::Text {
        s: i + 1,
        e: j,
        raw: false,
    }];
    Some((j + 1, Item::Node(Node::Styled { style, children })))
}

/// `~~...~~` on one line.
fn strike_wave(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !starts(b, i, b"~~") {
        return None;
    }
    let from = i + 2;
    let (j, got) = items(
        st,
        cx,
        from,
        |b, j| starts(b, j, b"~~") || newline(b, j).is_some(),
        inline,
    );
    if got.is_empty() {
        return Some((from, Item::Text(i, from, true)));
    }
    if !starts(b, j, b"~~") {
        return Some((j, Item::Text(i, j, true)));
    }
    let children = merge(got);
    Some((
        j + 2,
        Item::Node(Node::Styled {
            style: Style::Strike,
            children,
        }),
    ))
}

/// `<plain>...</plain>`, a line break right inside either end left out.
fn plain_tag(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !starts(b, i, b"<plain>") {
        return None;
    }
    let cs = opt_newline(b, i + 7);
    let ce = until(cx, cs, 1, |b, j| starts(b, opt_newline(b, j), b"</plain>"))?;
    let k = opt_newline(b, ce);
    if !starts(b, k, b"</plain>") {
        return None;
    }
    Some((k + 8, Item::Node(Node::Plain { cs, ce })))
}

/// `$[name.k=v,k ...]`.
fn function(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !starts(b, i, b"$[") {
        return None;
    }
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let Some(name_end) = run(b, i + 2, word) else {
        return Some((i + 2, Item::Text(i, i + 2, true)));
    };
    let mut j = name_end;
    let mut args = Vec::new();
    if b.get(j) == Some(&b'.') {
        // `arg.sep(",", 1)`: a `,` with no argument after it is left.
        let arg = |at: usize| -> Option<(usize, (String, Option<String>))> {
            let k = run(b, at, word)?;
            let key = cx.s[at..k].to_string();
            if b.get(k) == Some(&b'=')
                && let Some(v) = run(b, k + 1, |c| word(c) || matches!(c, b'.' | b'-'))
            {
                return Some((v, (key, Some(cx.s[k + 1..v].to_string()))));
            }
            Some((k, (key, None)))
        };
        if let Some((k, a)) = arg(j + 1) {
            args.push(a);
            j = k;
            while b.get(j) == Some(&b',') {
                let Some((k, a)) = arg(j + 1) else { break };
                args.push(a);
                j = k;
            }
        }
    }
    if b.get(j) != Some(&b' ') {
        return Some((j, Item::Text(i, j, true)));
    }
    let head_end = j + 1;
    let (j, got) = items(st, cx, head_end, |b, j| b.get(j) == Some(&b']'), inline);
    if got.is_empty() {
        return Some((head_end, Item::Text(i, head_end, true)));
    }
    if b.get(j) != Some(&b']') {
        return Some((j, Item::Text(i, j, true)));
    }
    let children = merge(got);
    Some((
        j + 1,
        Item::Node(Node::Fn {
            s: i,
            name_end,
            head_end,
            args,
            children,
        }),
    ))
}

/// `` `code` `` on one line.
fn inline_code(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if b.get(i) != Some(&b'`') {
        return None;
    }
    let e = until(cx, i + 1, 1, |b, j| {
        b[j] == b'`' || starts(b, j, "´".as_bytes()) || newline(b, j).is_some()
    })?;
    if b.get(e) != Some(&b'`') {
        return None;
    }
    Some((e + 1, Item::Node(Node::Kept { s: i, e: e + 1 })))
}

/// `\(math\)` on one line.
fn math_inline(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if !starts(b, i, b"\\(") {
        return None;
    }
    let e = until(cx, i + 2, 1, |b, j| {
        starts(b, j, b"\\)") || newline(b, j).is_some()
    })?;
    if !starts(b, e, b"\\)") {
        return None;
    }
    Some((e + 2, Item::Node(Node::Kept { s: i, e: e + 2 })))
}

/// `@name` or `@name@host`.
fn mention(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if st.link_label || b.get(i) != Some(&b'@') {
        return None;
    }
    let part = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-');
    let name_end = run(b, i + 1, part)?;
    let mut end = name_end;
    let mut host: Option<(usize, usize)> = None;
    if b.get(name_end) == Some(&b'@')
        && let Some(h) = run(b, name_end + 1, part)
    {
        host = Some((name_end + 1, h));
        end = h;
    }
    if alnum_before(b, i) {
        return None;
    }
    let trailing = |s: usize, e: usize| {
        b[s..e]
            .iter()
            .rev()
            .take_while(|c| matches!(c, b'.' | b'-'))
            .count()
    };
    let mut invalid = false;
    let mut host_len: Option<usize> = host.map(|(s, e)| e - s);
    if let Some((s, e)) = host {
        let t = trailing(s, e);
        if t > 0 {
            let len = e - s - t;
            if len == 0 {
                invalid = true;
                host_len = None;
            } else {
                host_len = Some(len);
            }
        }
    }
    let mut name_len = name_end - (i + 1);
    let t = trailing(i + 1, name_end);
    if t > 0 {
        if host_len.is_none() {
            name_len -= t;
        } else {
            invalid = true;
        }
    }
    let starts_badly = |s: usize| matches!(b[s], b'.' | b'-');
    if name_len == 0 || starts_badly(i + 1) {
        invalid = true;
    }
    if let (Some(_), Some((s, _))) = (host_len, host)
        && starts_badly(s)
    {
        invalid = true;
    }
    if invalid {
        return Some((end, Item::Text(i, end, true)));
    }
    let len = 1 + name_len + host_len.map_or(0, |h| 1 + h);
    Some((i + len, Item::Node(Node::Kept { s: i, e: i + len })))
}

/// mfm-js's `hashTagChar`.
fn hashtag_char(cx: Cx<'_>, j: usize) -> Option<usize> {
    let b = cx.b;
    let c = *b.get(j)?;
    if b" \t.,!?'\"#:/[]()<>".contains(&c) || newline(b, j).is_some() || space(b, j).is_some() {
        return None;
    }
    for s in ["【", "】", "「", "」", "（", "）"] {
        if starts(b, j, s.as_bytes()) {
            return None;
        }
    }
    next_char(cx, j)
}

fn hashtag_item(st: &mut State, cx: Cx<'_>, j: usize) -> Option<usize> {
    let b = cx.b;
    for (open, close) in [("(", ")"), ("[", "]"), ("「", "」"), ("（", "）")] {
        if !starts(b, j, open.as_bytes()) {
            continue;
        }
        let mut k = j + open.len();
        while k < b.len() {
            let r = st.nest(
                |st| hashtag_item(st, cx, k).map(|e| (e, Item::Text(k, e, false))),
                |_| hashtag_char(cx, k).map(|e| (e, Item::Text(k, e, false))),
            );
            match r {
                Some((e, _)) => k = e,
                None => break,
            }
        }
        if starts(b, k, close.as_bytes()) {
            return Some(k + close.len());
        }
    }
    hashtag_char(cx, j)
}

/// `#tag`.
fn hashtag(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if st.link_label || b.get(i) != Some(&b'#') {
        return None;
    }
    let mut j = i + 1;
    while j < b.len() {
        match hashtag_item(st, cx, j) {
            Some(k) => j = k,
            None => break,
        }
    }
    if j == i + 1 || alnum_before(b, i) || b[i + 1..j].iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some((j, Item::Node(Node::Kept { s: i, e: j })))
}

/// `:name:`.
fn emoji_code(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if b.get(i) != Some(&b':') {
        return None;
    }
    let e = run(b, i + 1, |c| {
        c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-')
    })?;
    if b.get(e) != Some(&b':') || !(line_end(b, e + 1) || !alnum(b, e + 1)) {
        return None;
    }
    Some((e + 1, Item::Node(Node::Kept { s: i, e: e + 1 })))
}

/// mfm-js's `manyThen`: items up to `stop`, then `after`, with the item
/// boundaries that led to a failure remembered so that a read from one of
/// them fails at once.
fn many_then(
    st: &mut State,
    cx: Cx<'_>,
    rule: u8,
    from: usize,
    stop: impl Fn(&[u8], usize) -> bool,
    item: fn(&mut State, Cx<'_>, usize) -> Res,
    after: impl FnOnce(&mut State, usize) -> Option<(usize, Node)>,
) -> Option<(usize, Vec<Item>, Node)> {
    let key = (rule, cx.id, st.state_key());
    let mut boundaries = Vec::new();
    let mut got = Vec::new();
    let mut j = from;
    while !st.failed(key, j) {
        boundaries.push(j);
        if j >= cx.b.len() || stop(cx.b, j) {
            break;
        }
        match st.nest(|st| item(st, cx, j), |_| text_char(cx, j)) {
            Some((k, it)) => {
                got.push(it);
                j = k;
            }
            None => break,
        }
    }
    if got.is_empty() {
        return None;
    }
    let tail = if st.failed(key, j) {
        None
    } else {
        after(st, j)
    };
    match tail {
        Some((end, n)) => Some((end, got, n)),
        None => {
            st.failures.entry(key).or_default().extend(boundaries);
            None
        }
    }
}

/// `[label](address)` or `?[label](address)`; the label ends at `]` or a
/// line break, and the address is one as it is or in angle brackets.
fn link(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if st.link_label {
        return None;
    }
    let (silent, open) = if starts(b, i, b"?[") {
        (true, i + 2)
    } else if b.get(i) == Some(&b'[') {
        (false, i + 1)
    } else {
        return None;
    };
    let after = |st: &mut State, j: usize| -> Option<(usize, Node)> {
        if !starts(b, j, b"](") {
            return None;
        }
        let at = j + 2;
        let (k, it) = url_alt(st, cx, at).or_else(|| url(st, cx, at))?;
        let Item::Node(n) = it else { return None };
        (b.get(k) == Some(&b')')).then_some((k + 1, n))
    };
    let (end, got, url) = many_then(
        st,
        cx,
        RULE_LINK,
        open,
        |b, j| b[j] == b']' || newline(b, j).is_some(),
        label_inline,
        after,
    )?;
    Some((
        end,
        Item::Node(Node::Link {
            s: i,
            e: end,
            silent,
            url: Box::new(url),
            label: merge(got),
        }),
    ))
}

/// One item of an address: a URL character, or brackets that pair up with
/// items in them.
fn url_item(st: &mut State, cx: Cx<'_>, j: usize) -> Option<usize> {
    let b = cx.b;
    let c = *b.get(j)?;
    let close = match c {
        b'(' => b')',
        b'[' => b']',
        c if is_url_char(c) => return Some(j + 1),
        _ => return None,
    };
    let mut k = j + 1;
    while k < b.len() {
        let r = st.nest(
            |st| url_item(st, cx, k).map(|e| (e, Item::Text(k, e, false))),
            |_| is_url_char(b[k]).then(|| (k + 1, Item::Text(k, k + 1, false))),
        );
        match r {
            Some((e, _)) => k = e,
            None => break,
        }
    }
    (b.get(k) == Some(&close)).then_some(k + 1)
}

/// An address as it is, without a `.` or `,` at its end.
fn url(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if st.link_label {
        return None;
    }
    let from = scheme(b, i)?;
    let mut j = from;
    while j < b.len() {
        match url_item(st, cx, j) {
            Some(k) => j = k,
            None => break,
        }
    }
    if j == from {
        return None;
    }
    let mut end = j;
    while end > from && matches!(b[end - 1], b'.' | b',') {
        end -= 1;
    }
    if end == from {
        return Some((j, Item::Text(i, j, true)));
    }
    Some((
        end,
        Item::Node(Node::Url {
            s: i,
            e: end,
            brackets: false,
        }),
    ))
}

/// `<https://...>`, with no space in it.
fn url_alt(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    if st.link_label || b.get(i) != Some(&b'<') {
        return None;
    }
    let from = scheme(b, i + 1)?;
    let e = until(cx, from, 1, |b, j| b[j] == b'>' || space(b, j).is_some())?;
    if b.get(e) != Some(&b'>') {
        return None;
    }
    Some((
        e + 1,
        Item::Node(Node::Url {
            s: i,
            e: e + 1,
            brackets: true,
        }),
    ))
}

/// `<center>...</center>` on lines of its own.
fn center(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let at = opt_newline(b, i);
    if !line_begin(b, at) || !starts(b, at, b"<center>") {
        return None;
    }
    let from = opt_newline(b, at + 8);
    let close = |b: &[u8], j: usize| starts(b, opt_newline(b, j), b"</center>");
    let after = |_: &mut State, j: usize| -> Option<(usize, Node)> {
        let k = opt_newline(b, j) + 9;
        if !close(b, j) || !line_end(b, k) {
            return None;
        }
        let end = opt_newline(b, k);
        Some((end, Node::Kept { s: k, e: end }))
    };
    let (end, got, trail) = many_then(st, cx, RULE_CENTER, from, close, inline, after)?;
    let Node::Kept { s: ts, e: te } = trail else {
        return None;
    };
    Some((
        end,
        Item::Node(Node::Center {
            lead: (i, at),
            trail: (ts, te),
            children: merge(got),
        }),
    ))
}

/// A code block: ```` ``` ```` at the start of a line, to a line that is
/// ```` ``` ````.
fn code_block(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let at = opt_newline(b, i);
    if !line_begin(b, at) || !starts(b, at, b"```") {
        return None;
    }
    let mut j = at + 3;
    while j < b.len() && newline(b, j).is_none() {
        j = next_char(cx, j)?;
    }
    let body = newline(b, j)?;
    let fence = |b: &[u8], j: usize| {
        newline(b, j).is_some_and(|k| starts(b, k, b"```") && line_end(b, k + 3))
    };
    let e = until(cx, body, 1, fence)?;
    let k = newline(b, e)?;
    if !starts(b, k, b"```") || !line_end(b, k + 3) {
        return None;
    }
    let end = opt_newline(b, k + 3);
    Some((end, Item::Node(Node::Kept { s: i, e: end })))
}

/// `\[math\]` at the start of a line.
fn math_block(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let at = opt_newline(b, i);
    if !line_begin(b, at) || !starts(b, at, b"\\[") {
        return None;
    }
    let from = opt_newline(b, at + 2);
    let e = until(cx, from, 1, |b, j| starts(b, opt_newline(b, j), b"\\]"))?;
    let k = opt_newline(b, e);
    if !starts(b, k, b"\\]") || !line_end(b, k + 2) {
        return None;
    }
    let end = opt_newline(b, k + 2);
    Some((end, Item::Node(Node::Kept { s: i, e: end })))
}

/// A search box: a line that ends in ` 検索`, ` search` or ` [search]`.
fn search_button(b: &[u8], i: usize) -> Option<usize> {
    let word = |at: usize| -> Option<usize> {
        if starts(b, at, "検索".as_bytes()) {
            Some(at + "検索".len())
        } else if b
            .get(at..at + 6)
            .is_some_and(|w| w.eq_ignore_ascii_case(b"search"))
        {
            Some(at + 6)
        } else {
            None
        }
    };
    if b.get(i) == Some(&b'[')
        && let Some(k) = word(i + 1)
        && b.get(k) == Some(&b']')
    {
        return Some(k + 1);
    }
    word(i)
}

fn search(cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let at = opt_newline(b, i);
    if !line_begin(b, at) {
        return None;
    }
    let ends = |b: &[u8], j: usize| {
        newline(b, j).is_some()
            || space(b, j)
                .and_then(|k| search_button(b, k))
                .is_some_and(|k| line_end(b, k))
    };
    let e = until(cx, at, 1, ends)?;
    let k = search_button(b, space(b, e)?)?;
    if !line_end(b, k) {
        return None;
    }
    let end = opt_newline(b, k);
    Some((end, Item::Node(Node::Kept { s: i, e: end })))
}

/// Lines that start with `>`, their content read again as a note.
fn quote(st: &mut State, cx: Cx<'_>, i: usize) -> Res {
    let b = cx.b;
    let at = opt_newline(b, opt_newline(b, i));
    if !line_begin(b, at) {
        return None;
    }
    let line = |j: usize| -> Option<(usize, usize, usize)> {
        if b.get(j) != Some(&b'>') {
            return None;
        }
        let from = space(b, j + 1).unwrap_or(j + 1);
        let mut k = from;
        while k < b.len() && newline(b, k).is_none() {
            k = next_char(cx, k)?;
        }
        Some((j, from, k))
    };
    let mut parts = vec![line(at)?];
    let mut j = parts[0].2;
    while let Some(k) = newline(b, j) {
        let Some(p) = line(k) else { break };
        parts.push(p);
        j = p.2;
    }
    let end = opt_newline(b, opt_newline(b, j));
    if parts.len() == 1 && parts[0].1 == parts[0].2 {
        return None;
    }
    let mut content = String::new();
    let mut lines = Vec::new();
    let mut marks = Vec::new();
    for (n, (m, s, e)) in parts.iter().enumerate() {
        if n > 0 {
            content.push('\n');
        }
        lines.push((content.len(), *s));
        marks.push(cx.s[*m..*s].to_string());
        content.push_str(&cx.s[*s..*e]);
    }
    let id = st.input_id(&content);
    let inner = Cx {
        s: &content,
        b: content.as_bytes(),
        id,
    };
    // `nest(fullParser).many(0)`: the whole content one level deeper, or
    // a character of text where the depth or the budget does not allow it.
    let mut got = Vec::new();
    let mut k = 0;
    while k < inner.b.len() {
        let mut read = Vec::new();
        let r = st.nest(
            |st| {
                let mut m = k;
                while m < inner.b.len() {
                    let Some((e, it)) = full(st, inner, m) else {
                        break;
                    };
                    read.push(it);
                    m = e;
                }
                Some((m, Item::Text(k, k, false)))
            },
            |_| text_char(inner, k),
        );
        let Some((e, it)) = r else { break };
        got.append(&mut read);
        got.push(it);
        k = e;
    }
    let children = merge(got);
    Some((
        end,
        Item::Node(Node::Quote {
            lead: (i, at),
            trail: (j, end),
            content,
            lines,
            marks,
            children,
        }),
    ))
}

/// The nodes of `text`, as mfm-js's `parse`.
fn parse(text: &str) -> Vec<Node> {
    let mut st = State::new(text);
    let cx = Cx {
        s: text,
        b: text.as_bytes(),
        id: 0,
    };
    let mut got = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let Some((e, it)) = full(&mut st, cx, i) else {
            break;
        };
        got.push(it);
        i = e;
    }
    merge(got)
}

// ---------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------

/// Where a position in a quote's content is in the text around it.
fn map_pos(lines: &[(usize, usize)], p: usize) -> usize {
    let n = lines.partition_point(|(c, _)| *c <= p).saturating_sub(1);
    let (c, o) = lines[n];
    o + (p - c)
}

/// Every link in `nodes`, read from `src`, with `map` taking a position in
/// `src` to one in the note.
fn collect(nodes: &[Node], src: &str, map: &dyn Fn(usize) -> usize, out: &mut Vec<Link>) {
    for n in nodes {
        match n {
            Node::Url { s, e, brackets } => out.push(Link {
                start: map(*s),
                end: map(*e),
                url: url_of(src, *s, *e, *brackets).to_string(),
                form: if *brackets { Form::Angle } else { Form::Bare },
            }),
            Node::Link {
                s, e, silent, url, ..
            } => {
                let Node::Url {
                    s: us,
                    e: ue,
                    brackets,
                } = **url
                else {
                    continue;
                };
                out.push(Link {
                    start: map(*s),
                    end: map(*e),
                    url: url_of(src, us, ue, brackets).to_string(),
                    form: Form::Labeled { silent: *silent },
                });
            }
            Node::Fn { children, .. }
            | Node::Styled { children, .. }
            | Node::Center { children, .. } => collect(children, src, map, out),
            Node::Quote {
                content,
                lines,
                children,
                ..
            } => {
                let inner = |p: usize| map(map_pos(lines, p));
                collect(children, content, &inner, out);
            }
            _ => {}
        }
    }
}

fn url_of(src: &str, s: usize, e: usize, brackets: bool) -> &str {
    if brackets {
        &src[s + 1..e - 1]
    } else {
        &src[s..e]
    }
}

/// Every link in `text`, in order.
pub fn links(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    collect(&parse(text), text, &|p| p, &mut out);
    out
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

// ---------------------------------------------------------------------
// Silencing
// ---------------------------------------------------------------------

/// `text` with every link made silent, so Misskey's web client shows no
/// preview under it, and the text reads as before: an address becomes
/// `?[<plain>address</plain>](address)` (the `<plain>` keeps the address
/// in the label from being read as MFM), and `[label](address)` becomes
/// `?[label](address)`.
///
/// What was text and would be read with what silencing adds is written so
/// that it stays text: a `[` that opened no link would take the `](` added
/// after an address as the end of its label, a `<plain>` or a `<https://`
/// that never closed would be closed by the `</plain>` added, and text that
/// ends like an address (`http://`) would take the `?` in. Such a `[` or
/// `<` is written `<plain>[</plain>`, and the character before the `?`
/// `<plain>/</plain>`, which read as the same. Each way is read again
/// before it is used; when none of them keeps the text, the note is written
/// again from what it shows, every link in it silent.
pub fn silenced(text: &str) -> String {
    let tree = parse(text);
    let want = Expect::of(text, &tree);
    if want
        .links
        .iter()
        .all(|l| l.form == (Form::Labeled { silent: true }))
    {
        return text.to_string();
    }
    let mut edits = Vec::new();
    edit_links(&tree, text, &|p| p, &mut edits);
    let plain = apply(text, edits.clone());
    if want.kept_by(&plain) {
        return plain;
    }
    if let Some(escaped) = escaped(text, &tree, &edits, &want) {
        return escaped;
    }
    let mut w = Writer::default();
    w.nodes(&tree, text, Ctx::Full);
    let written = w.finish();
    if want.kept_by(&written) {
        return written;
    }
    // Only what the note shows, in `<plain>`: no link, but no preview and
    // the same text.
    let mut w = Writer::default();
    w.text(&plain_text(text));
    w.finish()
}

/// `text` with the links silenced as `edits` do and with as few of the
/// `[` and `<` of its text before the last link, and of the characters
/// right before a link, written in `<plain>` as keep what it shows, if
/// that does.
fn escaped(text: &str, tree: &[Node], edits: &[Edit], want: &Expect) -> Option<String> {
    let last = edits.iter().map(|e| e.at + e.del).max()?;
    let mut strays = Vec::new();
    escape_strays(tree, text, &|p| p, &mut strays);
    strays.retain(|e| e.at < last);
    for e in edits {
        escape_before(text, e.at, &mut strays);
    }
    let with = |kept: &[bool]| {
        let mut all = edits.to_vec();
        all.extend(
            strays
                .iter()
                .zip(kept)
                .filter(|(_, k)| **k)
                .map(|(e, _)| e.clone()),
        );
        apply(text, all)
    };
    let mut kept = vec![true; strays.len()];
    if !want.kept_by(&with(&kept)) {
        return None;
    }
    // Each escape that is not needed is left out; past a few dozen the
    // note is left with all of them rather than read that many times.
    if strays.len() <= 64 {
        for k in 0..kept.len() {
            kept[k] = false;
            if !want.kept_by(&with(&kept)) {
                kept[k] = true;
            }
        }
    }
    Some(with(&kept))
}

/// What silencing must keep: the runs the note shows and its addresses.
struct Expect {
    runs: Vec<Run>,
    links: Vec<Link>,
}

impl Expect {
    fn of(text: &str, tree: &[Node]) -> Expect {
        let mut runs = Vec::new();
        render(tree, text, false, &mut runs);
        let mut links = Vec::new();
        collect(tree, text, &|p| p, &mut links);
        Expect { runs, links }
    }

    fn kept_by(&self, quiet: &str) -> bool {
        let got = links(quiet);
        plain(quiet) == self.runs
            && got.len() == self.links.len()
            && got
                .iter()
                .zip(&self.links)
                .all(|(g, w)| g.url == w.url && g.form == (Form::Labeled { silent: true }))
    }
}

/// A change to the text: `del` bytes at `at` replaced with `ins`.
#[derive(Debug, Clone)]
struct Edit {
    at: usize,
    del: usize,
    ins: String,
}

fn apply(text: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| (e.at, e.del));
    let mut out = String::with_capacity(text.len() + 64);
    let mut last = 0;
    for e in edits {
        if e.at < last {
            continue;
        }
        out.push_str(&text[last..e.at]);
        out.push_str(&e.ins);
        last = e.at + e.del;
    }
    out.push_str(&text[last..]);
    out
}

/// The edits that make each link silent.
fn edit_links(nodes: &[Node], src: &str, map: &dyn Fn(usize) -> usize, out: &mut Vec<Edit>) {
    for n in nodes {
        match n {
            Node::Url { s, e, brackets } => {
                let url = url_of(src, *s, *e, *brackets);
                let target = if *brackets {
                    format!("<{url}>")
                } else {
                    url.to_string()
                };
                out.push(Edit {
                    at: map(*s),
                    del: map(*e) - map(*s),
                    ins: format!("?[<plain>{url}</plain>]({target})"),
                });
            }
            Node::Link {
                s, silent: false, ..
            } => out.push(Edit {
                at: map(*s),
                del: 0,
                ins: "?".to_string(),
            }),
            Node::Fn { children, .. }
            | Node::Styled { children, .. }
            | Node::Center { children, .. } => edit_links(children, src, map, out),
            Node::Quote {
                content,
                lines,
                children,
                ..
            } => {
                let inner = |p: usize| map(map_pos(lines, p));
                edit_links(children, content, &inner, out);
            }
            _ => {}
        }
    }
}

/// Each `[` and `<` of text, written in `<plain>`.
fn escape_strays(nodes: &[Node], src: &str, map: &dyn Fn(usize) -> usize, out: &mut Vec<Edit>) {
    for n in nodes {
        match n {
            Node::Text { s, e, raw: false } => {
                for (k, c) in src[*s..*e].char_indices() {
                    if c == '[' || c == '<' {
                        out.push(Edit {
                            at: map(s + k),
                            del: 1,
                            ins: format!("<plain>{c}</plain>"),
                        });
                    }
                }
            }
            Node::Link { label, .. } => escape_strays(label, src, map, out),
            Node::Fn { children, .. }
            | Node::Styled { children, .. }
            | Node::Center { children, .. } => escape_strays(children, src, map, out),
            Node::Quote {
                content,
                lines,
                children,
                ..
            } => {
                let inner = |p: usize| map(map_pos(lines, p));
                escape_strays(children, content, &inner, out);
            }
            _ => {}
        }
    }
}

/// The character before `at`, written in `<plain>` when it is one an
/// address takes.
fn escape_before(text: &str, at: usize, out: &mut Vec<Edit>) {
    let Some(c) = text[..at].chars().next_back() else {
        return;
    };
    if c.is_ascii() && (is_url_char(c as u8) || matches!(c, '(' | ')' | '[' | ']')) {
        out.push(Edit {
            at: at - 1,
            del: 1,
            ins: format!("<plain>{c}</plain>"),
        });
    }
}

/// Where a note is written again: in a note or a quote, which may hold
/// blocks, in markup, or in a link's label.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Full,
    Inline,
}

/// A note written again from its nodes, text in `<plain>` and every link
/// silent.
#[derive(Default)]
struct Writer {
    out: String,
    /// Text not written yet, so that text next to text is one `<plain>`.
    pending: String,
    /// Whether a line break that starts the text waiting is written as it
    /// is, as a center that took none after it needs one.
    break_next: bool,
}

impl Writer {
    fn text(&mut self, s: &str) {
        self.pending.push_str(s);
    }

    /// Write the text waiting, in `<plain>`: a line break at either end is
    /// written twice, as `<plain>` leaves one out, and a `</plain>` in it
    /// is cut after its `<`.
    fn flush(&mut self) {
        let text = std::mem::take(&mut self.pending);
        let mut rest = text.as_str();
        if std::mem::take(&mut self.break_next) {
            let brk = newline(rest.as_bytes(), 0).unwrap_or(0);
            self.out.push_str(&rest[..brk]);
            rest = &rest[brk..];
        }
        while !rest.is_empty() {
            let cut = rest.find("</plain>").map_or(rest.len(), |k| k + 1);
            let (chunk, after) = rest.split_at(cut);
            self.out.push_str("<plain>");
            if chunk.starts_with(['\r', '\n']) {
                self.out.push('\n');
            }
            self.out.push_str(chunk);
            if chunk.ends_with('\n') {
                self.out.push('\n');
            } else if chunk.ends_with('\r') {
                self.out.push('\r');
            }
            self.out.push_str("</plain>");
            rest = after;
        }
    }

    /// Write the text waiting, but a line break at its end as it is, so
    /// that a block can start after it.
    fn flush_to_line_start(&mut self) {
        let brk = if self.pending.ends_with("\r\n") {
            "\r\n"
        } else if self.pending.ends_with('\n') {
            "\n"
        } else if self.pending.ends_with('\r') {
            "\r"
        } else {
            ""
        };
        self.pending.truncate(self.pending.len() - brk.len());
        self.flush();
        self.out.push_str(brk);
    }

    fn raw(&mut self, s: &str) {
        self.flush();
        self.out.push_str(s);
    }

    fn finish(mut self) -> String {
        self.flush();
        self.out
    }

    fn nodes(&mut self, nodes: &[Node], src: &str, ctx: Ctx) {
        for n in nodes {
            self.node(n, src, ctx);
        }
    }

    fn rendered(&mut self, nodes: &[Node], src: &str) {
        let mut runs = Vec::new();
        render(nodes, src, false, &mut runs);
        for r in runs {
            self.text(&r.text);
        }
    }

    fn node(&mut self, n: &Node, src: &str, ctx: Ctx) {
        match n {
            Node::Url { s, e, brackets } => {
                let url = url_of(src, *s, *e, *brackets);
                self.raw("?[");
                self.text(url);
                self.flush();
                if *brackets {
                    self.out.push_str(&format!("](<{url}>)"));
                } else {
                    self.out.push_str(&format!("]({url})"));
                }
            }
            Node::Link { url, label, .. } => {
                let Node::Url { s, e, brackets } = **url else {
                    return;
                };
                let target = url_of(src, s, e, brackets);
                self.raw("?[");
                self.nodes(label, src, Ctx::Inline);
                self.flush();
                if brackets {
                    self.out.push_str(&format!("](<{target}>)"));
                } else {
                    self.out.push_str(&format!("]({target})"));
                }
            }
            Node::Fn {
                s,
                name_end,
                head_end,
                children,
                ..
            } if &src[s + 2..*name_end] != "ruby" => {
                self.raw(&src[*s..*head_end]);
                self.nodes(children, src, Ctx::Inline);
                self.raw("]");
            }
            Node::Styled { style, children } => {
                let (open, close) = match style {
                    Style::Bold => ("<b>", "</b>"),
                    Style::Big => ("$[tada ", "]"),
                    Style::Small => ("<small>", "</small>"),
                    Style::Italic => ("<i>", "</i>"),
                    Style::Strike => ("<s>", "</s>"),
                };
                self.raw(open);
                self.nodes(children, src, Ctx::Inline);
                self.raw(close);
            }
            Node::Center {
                lead,
                trail,
                children,
            } if ctx == Ctx::Full => {
                if lead.0 == lead.1 {
                    self.flush_to_line_start();
                } else {
                    self.raw(&src[lead.0..lead.1]);
                }
                self.raw("<center>");
                self.nodes(children, src, Ctx::Inline);
                self.raw("</center>");
                self.raw(&src[trail.0..trail.1]);
                self.break_next = trail.0 == trail.1;
            }
            Node::Quote {
                lead,
                trail,
                content,
                marks,
                children,
                ..
            } if ctx == Ctx::Full => {
                let mut inner = Writer::default();
                inner.nodes(children, content, Ctx::Full);
                let body = inner.finish();
                if lead.0 == lead.1 {
                    self.flush_to_line_start();
                } else {
                    self.raw(&src[lead.0..lead.1]);
                }
                let quoted: Vec<String> = body
                    .split('\n')
                    .enumerate()
                    .map(|(n, l)| format!("{}{l}", marks.get(n).map_or("> ", String::as_str)))
                    .collect();
                self.raw(&quoted.join("\n"));
                self.raw(&src[trail.0..trail.1]);
            }
            n => self.rendered(std::slice::from_ref(n), src),
        }
    }
}

// ---------------------------------------------------------------------
// Plain text
// ---------------------------------------------------------------------

/// A run of a note's text as the client draws it: its characters, and
/// whether MFM makes them bold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
}

/// `text` as plain text, the way Misskey's own client shows what MFM
/// draws, less the drawing: a function Misskey knows (`$[x2 ...]`,
/// `$[tada ...]`, `$[fg.color=f00 ...]`, nested ones too) is its content,
/// one it does not know is `$[name content]`, `$[ruby base reading]` is
/// `base(reading)`, `<center>`, `<small>`, `<i>`, `<s>`, `*italic*` and
/// `~~strike~~` are what they hold, `<b>` and `**bold**` are bold, a link
/// with a label (`[label](address)`, `?[label](address)`) is its label,
/// `<https://...>` is the address without its brackets, `<plain>` is what
/// it holds as it is, and a quote is its lines after the `>` they start
/// with. Code, math,
/// mentions, hashtags, custom emoji (`:name:`) and anything that does not
/// close are left as written. The text never grows: markup is only taken
/// away.
pub fn plain(text: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    render(&parse(text), text, false, &mut runs);
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

/// The functions Misskey's web client draws; any other is drawn as
/// `$[name content]`.
const FUNCTIONS: &[&str] = &[
    "tada",
    "jelly",
    "twitch",
    "shake",
    "spin",
    "jump",
    "bounce",
    "flip",
    "x2",
    "x3",
    "x4",
    "font",
    "blur",
    "rainbow",
    "sparkle",
    "rotate",
    "position",
    "scale",
    "fg",
    "bg",
    "border",
    "ruby",
    "unixtime",
    "clickable",
];

/// The font families `$[font.name ...]` takes; without one the client
/// draws it as written.
const FONTS: &[&str] = &["serif", "monospace", "cursive", "fantasy", "emoji", "math"];

/// The text of `nodes` when they are text alone, as mfm-js joins them.
fn only_text<'a>(nodes: &[Node], src: &'a str) -> Option<&'a str> {
    let first = nodes.first()?;
    let last = nodes.last()?;
    let (Node::Text { s, .. }, Node::Text { e, .. }) = (first, last) else {
        return None;
    };
    nodes
        .iter()
        .all(|n| matches!(n, Node::Text { .. }))
        .then(|| &src[*s..*e])
}

/// The nodes of `nodes` after mfm-js joins text: the last one, and how
/// many there are.
fn last_child(nodes: &[Node]) -> (usize, usize) {
    let mut count = 0;
    let mut last = 0;
    for (k, n) in nodes.iter().enumerate() {
        let joins =
            matches!(n, Node::Text { .. }) && k > 0 && matches!(nodes[k - 1], Node::Text { .. });
        if !joins {
            count += 1;
            last = k;
        }
    }
    (count, last)
}

fn render(nodes: &[Node], src: &str, bold: bool, runs: &mut Vec<Run>) {
    for n in nodes {
        match n {
            Node::Text { s, e, .. } | Node::Kept { s, e } => push(runs, &src[*s..*e], bold),
            Node::Plain { cs, ce } => push(runs, &src[*cs..*ce], bold),
            Node::Url { s, e, brackets } => push(runs, url_of(src, *s, *e, *brackets), bold),
            Node::Link { label, .. } => render(label, src, bold, runs),
            Node::Fn {
                s,
                name_end,
                args,
                children,
                ..
            } => {
                let name = &src[s + 2..*name_end];
                let known = FUNCTIONS.contains(&name)
                    && (name != "font" || args.iter().any(|(k, _)| FONTS.contains(&k.as_str())));
                if !known {
                    push(runs, &format!("$[{name} "), bold);
                    render(children, src, bold, runs);
                    push(runs, "]", bold);
                } else if name == "ruby" {
                    ruby(children, src, bold, runs);
                } else {
                    render(children, src, bold, runs);
                }
            }
            Node::Styled { style, children } => {
                render(children, src, bold || *style == Style::Bold, runs);
            }
            Node::Center {
                lead,
                trail,
                children,
            } => {
                push(runs, &src[lead.0..lead.1], bold);
                render(children, src, bold, runs);
                push(runs, &src[trail.0..trail.1], bold);
            }
            Node::Quote {
                lead,
                trail,
                content,
                marks,
                children,
                ..
            } => {
                // Each line drawn after the mark its line was written with
                // (drawn, a quote has as many lines as it was written with
                // or fewer), so that the text does not grow.
                push(runs, &src[lead.0..lead.1], bold);
                let mut inner = Vec::new();
                render(children, content, bold, &mut inner);
                let mark = |n: usize| marks.get(n).map_or(">", String::as_str);
                let mut n = 0;
                push(runs, mark(n), bold);
                for r in inner {
                    let mut lines = r.text.split('\n');
                    if let Some(first) = lines.next() {
                        push(runs, first, r.bold);
                    }
                    for l in lines {
                        n += 1;
                        push(runs, "\n", r.bold);
                        push(runs, mark(n), bold);
                        push(runs, l, r.bold);
                    }
                }
                push(runs, &src[trail.0..trail.1], bold);
            }
        }
    }
}

/// `$[ruby ...]` as Misskey's web client draws it: one text is split at
/// its spaces into the base and the reading (what follows a second space
/// is not drawn); otherwise the last node is the reading, if it is text,
/// and the rest the base.
fn ruby(children: &[Node], src: &str, bold: bool, runs: &mut Vec<Run>) {
    let (count, last) = last_child(children);
    let (base_nodes, reading) = if count == 1 {
        let text = only_text(children, src).unwrap_or("");
        let mut parts = text.split(' ');
        push(runs, parts.next().unwrap_or(""), bold);
        (None, parts.next().unwrap_or("").to_string())
    } else {
        let reading = only_text(&children[last..], src).unwrap_or("").trim();
        (Some(&children[..last]), reading.to_string())
    };
    if let Some(base) = base_nodes {
        render(base, src, bold, runs);
    }
    if !reading.is_empty() {
        push(runs, "(", bold);
        push(runs, &reading, bold);
        push(runs, ")", bold);
    }
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
    #[case("<https://x$[x2 漢字 http://p")]
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
    #[case("x**$[x **http://t] ")]
    #[case("<small>$[x2 </small>https://a.test] 🇯🇵</small>")]
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

    // Markup that never closes is text as written, all of it and the links
    // in it too, as mfm-js's `seqOrText` reads it: silencing has nothing to
    // change, and the text reads the same.
    #[rstest]
    #[case("$[x2 \n$[x2 $[x2 [label](https://e.example/c)]")]
    #[case("$[x2 https://a.test")]
    #[case("**太字 [記事](https://a.test)")]
    #[case("<small>小さく <https://a.test>")]
    fn markup_that_never_closes_holds_no_link(#[case] text: &str) {
        assert_eq!(urls(text), Vec::<String>::new());
        assert_eq!(silenced(text), text);
        assert_eq!(plain_text(text), text);
    }

    // A strike ends at the line, and what it took is text.
    #[test]
    fn a_strike_that_does_not_close_on_its_line_is_text() {
        let text = "~~消し https://a.test\nhttps://b.test";
        assert_eq!(urls(text), ["https://b.test"]);
        silencing_keeps_the_text(text);
    }

    // A ruby's reading is its last node when it holds more than one: an
    // address there is no reading, and silenced it is none either.
    #[test]
    fn a_ruby_whose_last_node_is_a_link_has_no_reading() {
        let text = "$[ruby <plain> \u{1b}[31m</plain>https://e.example/a]";
        assert_eq!(plain_text(text), " \u{1b}[31m");
        silencing_keeps_the_text(text);
    }

    // The links in a quote are found where they are in the note, and
    // silenced there.
    #[test]
    fn links_in_a_quote_are_silenced_in_place() {
        let text = "> 見て https://a.test\n> [記事](https://b.test)\n本文 https://c.test";
        let got: Vec<(usize, usize)> = links(text).iter().map(|l| (l.start, l.end)).collect();
        assert_eq!(got, [(9, 23), (26, 50), (58, 72)]);
        assert_eq!(
            silenced(text),
            "> 見て ?[<plain>https://a.test</plain>](https://a.test)\n> ?[記事](https://b.test)\n本文 ?[<plain>https://c.test</plain>](https://c.test)"
        );
        silencing_keeps_the_text(text);
    }

    // A `<plain>` that never closes, before a link, is written so that the
    // `</plain>` silencing adds does not close it.
    #[test]
    fn an_unclosed_plain_before_a_link_stays_text() {
        let text = "<plain>閉じない https://a.test";
        assert_eq!(
            silenced(text),
            "<plain><</plain>plain>閉じない ?[<plain>https://a.test</plain>](https://a.test)"
        );
        silencing_keeps_the_text(text);
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
    // Text and a link: the text is the base and the reading, the last
    // node, is not text, so Misskey's web client shows none.
    #[case("$[ruby x[31m [label](https://e.example/c)]", "x")]
    #[case("$[ruby **漢字** かんじ]", "漢字(かんじ)")]
    #[case("$[ruby 漢字 かんじ 余り]", "漢字(かんじ)")]
    #[case("<center>まんなか</center>", "まんなか")]
    #[case("<small>小さく<i>斜め</i></small>", "小さく斜め")]
    #[case("<center><small>二重</small></center>", "二重")]
    #[case("前\n<center>\n中\n</center>\n後", "前\n中\n後")]
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
    #[case("<b>太字 $[x2 </b>大] 後</b>", "太字 </b>大 後")]
    // A quote's lines keep the `>` and the space they were written with,
    // so that the text does not grow (`>]` was drawn `> ]`).
    #[case(
        "> 引用 https://a.test\n>続き\n本文",
        "> 引用 https://a.test\n>続き\n本文"
    )]
    #[case("前\n>]</plai]>n\n> > 二重", "前\n>]</plai]>n\n> > 二重")]
    #[case("*italic* と ~~消し~~", "italic と 消し")]
    #[case(
        "$[font.serif 明朝] $[font 中身] $[unknown 中身]",
        "明朝 $[font 中身] $[unknown 中身]"
    )]
    #[case("***大きく***", "大きく")]
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
    // The `**` inside takes the `]`, and closes nowhere.
    #[case("x**$[x **太字] 後")]
    // A center opens only at the start of a line and holds none.
    #[case("<center><center>二重</center></center>")]
    #[case("**太字 $[x2 大**]**")]
    #[case("<sup>上</sup><sub>下</sub>")]
    #[case("~~行をまたぐ\n打ち消し~~")]
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

    /// The tree of `text` in a short form: text as it is, and each node in
    /// braces (`{kept:@a}`, `{italic|a}`, `{link:address|label}`,
    /// `{$name.args|content}`), so that it can be checked against what
    /// mfm-js reads.
    fn tree(text: &str) -> String {
        fn of(nodes: &[Node], src: &str) -> String {
            let mut o = String::new();
            for n in nodes {
                match n {
                    Node::Text { s, e, .. } => o.push_str(&src[*s..*e]),
                    Node::Kept { s, e } => {
                        o.push_str(&format!(
                            "{{kept:{}}}",
                            src[*s..*e].trim_matches(['\r', '\n'])
                        ));
                    }
                    Node::Plain { cs, ce } => o.push_str(&format!("{{plain|{}}}", &src[*cs..*ce])),
                    Node::Url { s, e, brackets } => {
                        let form = if *brackets { "url<>" } else { "url" };
                        o.push_str(&format!("{{{form}:{}}}", url_of(src, *s, *e, *brackets)));
                    }
                    Node::Link {
                        silent, url, label, ..
                    } => {
                        let Node::Url { s, e, brackets } = **url else {
                            unreachable!()
                        };
                        let form = if *silent { "?link" } else { "link" };
                        let url = url_of(src, s, e, brackets);
                        o.push_str(&format!("{{{form}:{url}|{}}}", of(label, src)));
                    }
                    Node::Fn {
                        s,
                        name_end,
                        args,
                        children,
                        ..
                    } => {
                        let args: Vec<String> = args
                            .iter()
                            .map(|(k, v)| v.as_ref().map_or(k.clone(), |v| format!("{k}={v}")))
                            .collect();
                        let args = if args.is_empty() {
                            String::new()
                        } else {
                            format!(".{}", args.join(","))
                        };
                        let name = &src[s + 2..*name_end];
                        o.push_str(&format!("{{${name}{args}|{}}}", of(children, src)));
                    }
                    Node::Styled { style, children } => {
                        let style = match style {
                            Style::Bold => "bold",
                            Style::Big => "$tada",
                            Style::Small => "small",
                            Style::Italic => "italic",
                            Style::Strike => "strike",
                        };
                        o.push_str(&format!("{{{style}|{}}}", of(children, src)));
                    }
                    Node::Center { children, .. } => {
                        o.push_str(&format!("{{center|{}}}", of(children, src)));
                    }
                    Node::Quote {
                        content, children, ..
                    } => o.push_str(&format!("{{quote|{}}}", of(children, content))),
                }
            }
            o
        }
        of(&parse(text), text)
    }

    // Each case is read as mfm-js 0.28.0 reads it.
    #[rstest]
    // Italic and bold of letters, digits and spaces (an ideographic space
    // too), not right after a letter or digit.
    #[case("_a_", "{italic|a}")]
    #[case("*ab\u{3000}c*", "{italic|ab\u{3000}c}")]
    #[case("a*b*", "a*b*")]
    #[case(" *b*", " {italic|b}")]
    #[case("__a__ b", "{bold|a} b")]
    #[case("__", "__")]
    #[case("____", "____")]
    #[case("**_a_**", "{bold|{italic|a}}")]
    // Math in markup, and math that holds nothing.
    #[case("**\\(x\\)**", "{bold|{kept:\\(x\\)}}")]
    #[case("\\(\\)", "\\(\\)")]
    // A keycap is an emoji, with the variation selector or without.
    #[case("1\u{20e3}", "{kept:1\u{20e3}}")]
    #[case("1\u{fe0f}\u{20e3}", "{kept:1\u{fe0f}\u{20e3}}")]
    // A function needs a name, and takes the arguments after a `.`.
    #[case("$[ a]", "$[ a]")]
    #[case("$[x2.a=1 c]", "{$x2.a=1|c}")]
    #[case("$[x2.a=1-,b=2 c]", "{$x2.a=1-,b=2|c}")]
    // An address in brackets is no address in a label.
    #[case(
        "[<https://a.test>](https://b.test)",
        "{link:https://b.test|<https://a.test>}"
    )]
    fn inline_markup_is_read_as_mfm_js_reads_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(tree(text), want, "{text:?}");
    }

    // A mention is a name and a host of letters, digits, `_`, `.` and `-`,
    // without the `.` and `-` at their ends; one that starts with them, or
    // is right after a letter or digit, is text.
    #[rstest]
    #[case("@a", "{kept:@a}")]
    #[case("@alice@host.test", "{kept:@alice@host.test}")]
    #[case("@a@bc", "{kept:@a@bc}")]
    #[case("@alice@", "{kept:@alice}@")]
    #[case("@alice x", "{kept:@alice} x")]
    #[case("@a.", "{kept:@a}.")]
    #[case("@a@b.test.", "{kept:@a@b.test}.")]
    #[case("@a@..", "@a@..")]
    #[case("@a.@b", "@a.@b")]
    #[case("@.a", "@.a")]
    #[case("a@b", "a@b")]
    #[case("[@a](https://x.test)", "{link:https://x.test|@a}")]
    fn a_mention_is_read_as_mfm_js_reads_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(tree(text), want, "{text:?}");
    }

    // A hashtag ends at a space, a line break or a mark, takes brackets
    // that pair up, and is not digits alone or right after a letter.
    #[rstest]
    #[case("#_b_", "{kept:#_b_}")]
    #[case("#a.b", "{kept:#a}.b")]
    #[case("#a\nb", "{kept:#a}\nb")]
    #[case("#a\u{3000}b", "{kept:#a}\u{3000}b")]
    #[case("#(a)b", "{kept:#(a)b}")]
    #[case("#a(b)c", "{kept:#a(b)c}")]
    #[case("#\u{300c}a\u{300d}", "{kept:#\u{300c}a\u{300d}}")]
    #[case("#123", "#123")]
    #[case("#1a", "{kept:#1a}")]
    #[case("a#b", "a#b")]
    #[case("[#a](https://x.test)", "{link:https://x.test|#a}")]
    fn a_hashtag_is_read_as_mfm_js_reads_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(tree(text), want, "{text:?}");
    }

    // A custom emoji is not followed by a letter or digit.
    #[rstest]
    #[case(":a:", "{kept::a:}")]
    #[case(":a: ", "{kept::a:} ")]
    #[case(":a:b", ":a:b")]
    fn a_custom_emoji_is_read_as_mfm_js_reads_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(tree(text), want, "{text:?}");
    }

    // Code, math, search boxes and quotes start at the start of a line.
    #[rstest]
    #[case("a> b", "a> b")]
    #[case("a```\nb\n```", "a```\nb\n```")]
    #[case("a\n```\nb\n```", "a{kept:```\nb\n```}")]
    #[case("```js\nb\n```", "{kept:```js\nb\n```}")]
    #[case("\\[x\\]", "{kept:\\[x\\]}")]
    #[case("a\\[x\\]", "a\\[x\\]")]
    #[case("ab\n\\[x\\]", "ab{kept:\\[x\\]}")]
    #[case("\\[x\\]y", "\\[x\\]y")]
    #[case("\\[x\\]\na", "{kept:\\[x\\]}a")]
    #[case("a 検索", "{kept:a 検索}")]
    #[case("a search", "{kept:a search}")]
    #[case("a [search]", "{kept:a [search]}")]
    #[case("> a", "{quote|a}")]
    #[case(">a", "{quote|a}")]
    #[case(">", ">")]
    #[case(">\n>", "{quote|\n}")]
    fn a_block_is_read_as_mfm_js_reads_it(#[case] text: &str, #[case] want: &str) {
        assert_eq!(tree(text), want, "{text:?}");
    }

    // Markup nests twenty deep, as in mfm-js: past that, what would nest
    // is text. The depth is that of the markup around, not of what was
    // read before it.
    #[test]
    fn markup_nests_as_deep_as_mfm_js_lets_it() {
        let nested = |n: usize| format!("{}**a**{}", "<small>".repeat(n), "</small>".repeat(n));
        let inside = |text: &str| tree(text).replace("{small|", "").replace('}', "");
        assert_eq!(inside(&nested(19)), "{bold|a");
        assert_eq!(inside(&nested(20)), "**a**");
        let long = format!("<small>{}**b**</small>", "a".repeat(25));
        assert_eq!(
            tree(&long),
            format!("{{small|{}{{bold|b}}}}", "a".repeat(25))
        );
        let brackets = format!("https://a.test/{}b{}", "(".repeat(20), ")".repeat(20));
        assert_eq!(urls(&brackets), [brackets]);
    }

    // A link that fails is remembered as failed for the text it was read
    // in and the depth it was read at, and for nothing else: not for the
    // content of another quote, and not for the same link read at another
    // depth (here a center that never closes reads it one deeper, where
    // the code in its label is past the limit).
    #[test]
    fn a_failed_link_fails_again_only_where_it_failed() {
        let quotes = "> [a\nx\n> [c\nx\n> [b](https://x.test)\nx\n> [b](https://x.test)";
        let forms: Vec<Form> = links(quotes).into_iter().map(|l| l.form).collect();
        assert_eq!(forms, [Form::Labeled { silent: false }; 2]);
        let deep = format!("{} <center>[`]`](https://x.test)", ">".repeat(18));
        let forms: Vec<Form> = links(&deep).into_iter().map(|l| l.form).collect();
        assert_eq!(forms, [Form::Labeled { silent: false }]);
    }

    // A link that failed fails again at once, which keeps the reads of a
    // run of `[` within mfm-js's budget: the markup after it is read.
    #[test]
    fn a_run_of_brackets_leaves_the_budget_for_what_follows() {
        let text = format!("{}\n<small>**b**</small>", "[".repeat(200));
        let got = tree(&text);
        assert!(got.ends_with("\n{small|{bold|b}}"), "{got}");
    }

    // mfm-js lets a note nest twenty reads per character and a thousand.
    // A center that never closes reads its addresses twice, once in it and
    // once after it fails, and with fifteen characters before it the last
    // read the budget allows is the bold's; with fourteen it runs out just
    // before, and the bold is text.
    #[test]
    fn the_budget_runs_out_at_the_read_mfm_js_stops_at() {
        let note = |pad: usize| {
            let (pad, addresses) = ("y".repeat(pad), "https://a(".repeat(31));
            format!("{pad}\n<center>{addresses}\nx<small>**b**</small>**#(")
        };
        assert!(tree(&note(15)).ends_with("x{small|{bold|b}}**#("));
        assert!(tree(&note(14)).ends_with("x{small|**b**}**#("));
    }

    #[test]
    fn a_character_ends_after_its_last_byte() {
        let s = "é日";
        let cx = Cx {
            s,
            b: s.as_bytes(),
            id: 0,
        };
        assert_eq!(next_char(cx, 0), Some(2));
        assert_eq!(next_char(cx, 2), Some(5));
        assert_eq!(next_char(cx, 5), None);
    }

    // Text next to text is one node, but text as written of markup that
    // never closed stays apart.
    #[test]
    fn merge_joins_text_that_touches() {
        let spans = |items: Vec<Item>| -> Vec<(usize, usize, bool)> {
            merge(items)
                .into_iter()
                .map(|n| match n {
                    Node::Text { s, e, raw } => (s, e, raw),
                    _ => unreachable!(),
                })
                .collect()
        };
        let t = |s, e, raw| Item::Text(s, e, raw);
        assert_eq!(spans(vec![t(0, 1, false), t(1, 2, false)]), [(0, 2, false)]);
        assert_eq!(
            spans(vec![t(0, 1, false), t(2, 3, false)]),
            [(0, 1, false), (2, 3, false)]
        );
        assert_eq!(
            spans(vec![t(0, 1, true), t(1, 2, false)]),
            [(0, 1, true), (1, 2, false)]
        );
        assert_eq!(
            spans(vec![t(0, 1, false), t(1, 2, true)]),
            [(0, 1, false), (1, 2, true)]
        );
    }

    // Silenced, a `[` or `<` is written in `<plain>` where it is needed and
    // only there: in a function, a quote or markup, and the `/` of text
    // that ends like an address before a link.
    #[rstest]
    #[case(
        "$[x2 [baz https://a.test]",
        "$[x2 <plain>[</plain>baz ?[<plain>https://a.test</plain>](https://a.test)]"
    )]
    #[case(
        "> [baz https://a.test",
        "> <plain>[</plain>baz ?[<plain>https://a.test</plain>](https://a.test)"
    )]
    #[case(
        "http://<https://a.test>",
        "http:/<plain>/</plain>?[<plain>https://a.test</plain>](<https://a.test>)"
    )]
    #[case(
        "[x] [baz. https://a.test",
        "[x] <plain>[</plain>baz. ?[<plain>https://a.test</plain>](https://a.test)"
    )]
    // The `</plain>` that the `/` is written with, or that an address
    // after it is, would close a `<plain>` before a link or in its label.
    #[case(
        "<plain>https://[[l](https://a.test)",
        "<plain><</plain>plain>https:/<plain>/</plain>?[[l](https://a.test)"
    )]
    #[case(
        "[<plain>x](https://b.test) https://a.test",
        "?[<plain><</plain>plain>x](https://b.test) ?[<plain>https://a.test</plain>](https://a.test)"
    )]
    fn silencing_escapes_only_what_it_must(#[case] text: &str, #[case] want: &str) {
        assert_eq!(silenced(text), want);
        silencing_keeps_the_text(text);
    }

    fn written(text: &str) -> String {
        let mut w = Writer::default();
        w.nodes(&parse(text), text, Ctx::Full);
        w.finish()
    }

    // Written again, text is in `<plain>`, markup and blocks are kept, and
    // a block starts its line as it did.
    #[rstest]
    #[case("$[x2 a]", "$[x2 <plain>a</plain>]")]
    #[case("$[ruby 漢字 かんじ]", "<plain>漢字(かんじ)</plain>")]
    #[case("**a** `c`", "<b><plain>a</plain></b><plain> `c`</plain>")]
    #[case("[l](<https://a.test>)", "?[<plain>l</plain>](<https://a.test>)")]
    #[case(
        "a\n<center>b</center>",
        "<plain>a</plain>\n<center><plain>b</plain></center>"
    )]
    #[case(
        "<center>a</center>\n\nb",
        "<center><plain>a</plain></center>\n<plain>\n\nb</plain>"
    )]
    #[case("a\n> b", "<plain>a</plain>\n> <plain>b</plain>")]
    #[case("```\na\n```\n> b", "<plain>```\na\n```</plain>\n> <plain>b</plain>")]
    fn a_note_is_written_again_as_it_reads(#[case] text: &str, #[case] want: &str) {
        assert_eq!(written(text), want);
        assert_eq!(plain_text(want), plain_text(text));
    }

    // A ruby of one text, which mfm-js joins from what never closed and
    // the text around it, is split at its spaces.
    #[test]
    fn a_ruby_of_joined_text_is_split_at_its_spaces() {
        assert_eq!(plain_text("$[ruby a $[ b]"), "a($[)");
    }
}
