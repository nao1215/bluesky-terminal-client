//! A status's HTML as the text it reads as, for the terminal and for
//! `--json`.
//!
//! Mastodon sends a status's text (and an account's description) as HTML:
//! paragraphs in `<p>`, line breaks as `<br>`, and links, mentions and
//! hashtags as `<a>`, a long link's address split into spans of which some
//! are hidden. This is read back into the text the author wrote:
//!
//! - a paragraph is followed by a blank line, a `<br>` by a line break;
//!   there is never more than one blank line in a row;
//! - a mention (`@alice`) and a hashtag (`#rust`) read as their text, a link
//!   as its address whole (the hidden spans included); a link whose text is
//!   not its address (one from another server's software, `<a
//!   href="https://example.com/a">the article</a>`) reads `the article
//!   (https://example.com/a)`, so the address is never lost;
//! - custom emoji are already `:shortcode:` in the text and stay so; a
//!   picture (`<img>`) reads as its alt text;
//! - a quote reads with `> ` before its lines, a list item with `- ` (or its
//!   number), and preformatted text keeps its lines and spaces;
//! - entities (`&amp;`, `&#128008;`) are decoded, and what is in `<script>`
//!   or `<style>` is not text.
//!
//! Spaces the author typed stay, as Mastodon's web client shows them; a
//! line break in the HTML source is only a space, and none is left at the
//! start or the end of a line. Nothing here makes the text safe for a
//! terminal: [`crate::text::drawable`] does that where it is printed.

/// The text `html` reads as.
pub fn to_text(html: &str) -> String {
    let mut w = Writer::default();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(at) = rest.find(['<', '&']) else {
            w.text(rest);
            break;
        };
        w.text(&rest[..at]);
        rest = &rest[at..];
        if rest.starts_with('&') {
            let (c, used) = entity(rest);
            w.decoded(&c);
            rest = &rest[used..];
            continue;
        }
        match tag(rest) {
            Some((t, used)) => {
                rest = &rest[used..];
                if let Some(skip) = w.tag(&t) {
                    // The content of a script or a style is not text: it
                    // goes up to its closing tag, or to the end.
                    rest = skip_to_close(rest, skip);
                }
            }
            None => {
                // A `<` that starts no tag is text.
                w.text("<");
                rest = &rest[1..];
            }
        }
    }
    w.finish()
}

/// A tag as read: its name in lower case, whether it closes, and its
/// attributes, their values decoded.
#[derive(Debug, Default)]
struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map_or("", |(_, v)| v.as_str())
    }
}

/// The tag `s` starts with (`s` starts with `<`), and how many bytes it
/// takes; `None` when it is no tag: `<` not followed by a name, `/` or `!`,
/// or one that is never closed. A comment or a declaration is a tag with
/// no name.
fn tag(s: &str) -> Option<(Tag, usize)> {
    let body = &s[1..];
    if let Some(after) = body.strip_prefix("!--") {
        let end = after.find("-->").map_or(s.len(), |i| 1 + 3 + i + 3);
        return Some((Tag::default(), end));
    }
    let first = body.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '/' || first == '!' || first == '?') {
        return None;
    }
    // The tag ends at the first `>` outside a quoted value.
    let mut quote: Option<char> = None;
    let mut end = None;
    for (i, c) in body.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => {
                end = Some(i);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    let inner = &body[..end];
    let used = 1 + end + 1;
    if first == '!' || first == '?' {
        return Some((Tag::default(), used));
    }
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let name_end = inner
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(inner.len());
    let name = inner[..name_end].to_ascii_lowercase();
    if name.is_empty() {
        return Some((Tag::default(), used));
    }
    Some((
        Tag {
            name,
            closing,
            attrs: attributes(&inner[name_end..]),
        },
        used,
    ))
}

/// The attributes of a tag, `name="value"`, `name='value'`, `name=value`
/// or `name` alone, names in lower case and values decoded.
fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() {
            return out;
        }
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let mut value = String::new();
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (raw, left) = match after.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let inner = &after[1..];
                    match inner.find(q) {
                        Some(i) => (&inner[..i], &inner[i + 1..]),
                        None => (inner, ""),
                    }
                }
                _ => {
                    let i = after
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(after.len());
                    (&after[..i], &after[i..])
                }
            };
            value = decode(raw);
            rest = left;
        }
        if name.is_empty() {
            // `=` with no name before it: skip the character.
            let mut chars = rest.chars();
            chars.next();
            rest = chars.as_str();
            continue;
        }
        out.push((name, value));
    }
}

/// `s` with its entities decoded.
fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let (c, used) = entity(&rest[at..]);
        out.push_str(&c);
        rest = &rest[at + used..];
    }
    out.push_str(rest);
    out
}

/// The named entities decoded, beside the numeric ones: those HTML of a
/// status is written with.
const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("hellip", "…"),
    ("mdash", "—"),
    ("ndash", "–"),
    ("lsquo", "‘"),
    ("rsquo", "’"),
    ("ldquo", "“"),
    ("rdquo", "”"),
    ("laquo", "«"),
    ("raquo", "»"),
    ("middot", "·"),
    ("bull", "•"),
    ("copy", "©"),
    ("reg", "®"),
    ("trade", "™"),
    ("deg", "°"),
    ("times", "×"),
    ("divide", "÷"),
    ("euro", "€"),
    ("yen", "¥"),
    ("pound", "£"),
    ("shy", "\u{ad}"),
    ("zwj", "\u{200d}"),
    ("zwnj", "\u{200c}"),
];

/// The entity `s` starts with (`s` starts with `&`), decoded, and how many
/// bytes it takes. One that is not an entity is the `&` itself. A number
/// that names no character (0, a surrogate, past U+10FFFF) is U+FFFD.
fn entity(s: &str) -> (String, usize) {
    let body = &s[1..];
    let Some(semi) = body.find(';').filter(|i| *i <= 32) else {
        return ("&".into(), 1);
    };
    let name = &body[..semi];
    let used = 1 + semi + 1;
    if let Some(num) = name.strip_prefix('#') {
        let n = match num.strip_prefix(['x', 'X']) {
            Some(hex) if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                u32::from_str_radix(hex, 16).ok()
            }
            None if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) => {
                num.parse::<u32>().ok()
            }
            _ => return ("&".into(), 1),
        };
        let c = n
            .filter(|n| *n != 0)
            .and_then(char::from_u32)
            .unwrap_or('\u{fffd}');
        return (c.to_string(), used);
    }
    match ENTITIES.iter().find(|(n, _)| *n == name) {
        Some((_, c)) => ((*c).to_string(), used),
        None => ("&".into(), 1),
    }
}

/// `rest` after the closing tag of `name`, or nothing when it is not closed.
fn skip_to_close<'a>(rest: &'a str, name: &str) -> &'a str {
    let lower = rest.to_ascii_lowercase();
    let close = format!("</{name}");
    match lower.find(&close) {
        Some(i) => {
            let after = &rest[i..];
            after.find('>').map_or("", |j| &after[j + 1..])
        }
        None => "",
    }
}

/// An `<a>` being read: where it points, what it is, and its text so far.
#[derive(Debug, Default)]
struct Anchor {
    href: String,
    /// A mention or a hashtag: it reads as its text.
    named: bool,
    text: String,
    /// Whether a space was due before the link.
    space_before: bool,
}

/// A list being read: numbered from its next item's number, or not.
#[derive(Debug, Clone, Copy)]
enum List {
    Bullets,
    Numbers(u64),
}

/// The text as it is written: lines, the breaks between them, and what
/// starts each line inside a quote or a list.
#[derive(Debug, Default)]
struct Writer {
    out: String,
    /// Line breaks to write before the next text: 1 for a line, 2 for a
    /// paragraph.
    pending: usize,
    /// Whether the next text starts a line.
    line_start: bool,
    /// A space the HTML source has (a line break in it, say) that shows
    /// only between two words on one line.
    soft_space: bool,
    /// How deep in quotes the text is.
    quote: usize,
    /// How deep in quotes the text was where the breaks due were asked
    /// for, the least of them: a blank line before a quote is not quoted.
    pending_quote: usize,
    lists: Vec<List>,
    /// What the next text on a new line in a list item starts with.
    item_mark: Option<String>,
    pre: usize,
    anchor: Option<Anchor>,
}

impl Writer {
    /// Text of the HTML source (not decoded yet).
    fn text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if self.pre > 0 {
            self.put(s, true);
            return;
        }
        // A run of whitespace with a line break in it is the source's
        // layout: one soft space. Spaces alone are the author's.
        let mut run = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if matches!(c, '\n' | '\r' | '\x0c') {
                // Spaces before the break go with it.
                let kept = run.trim_end_matches(' ').len();
                run.truncate(kept);
                self.flush_run(&mut run);
                while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
                    chars.next();
                }
                self.soft_space = true;
                continue;
            }
            run.push(c);
        }
        self.flush_run(&mut run);
    }

    fn flush_run(&mut self, run: &mut String) {
        if !run.is_empty() {
            let s = std::mem::take(run);
            self.put(&s, false);
        }
    }

    /// Text already decoded (an entity's character): a line break written
    /// as one is the source's layout too, but in preformatted text.
    fn decoded(&mut self, s: &str) {
        if self.pre > 0 || self.anchor.is_some() {
            self.put(s, self.pre > 0);
        } else {
            self.text(s);
        }
    }

    /// Write `s`, starting a line first when one is due.
    fn put(&mut self, s: &str, literal: bool) {
        if let Some(a) = &mut self.anchor {
            if self.soft_space && !a.text.is_empty() {
                a.text.push(' ');
            }
            self.soft_space = false;
            a.text.push_str(s);
            return;
        }
        if literal && s.contains('\n') {
            // Each line break of preformatted text is one, blank lines
            // included.
            for (i, line) in s.split('\n').enumerate() {
                if i > 0 && !self.out.is_empty() {
                    self.pending += 1;
                }
                self.put_inline(line.trim_end_matches('\r'));
            }
            return;
        }
        self.put_inline(s);
    }

    fn put_inline(&mut self, s: &str) {
        // The mark of a quote's blank line is not text; it would show as
        // nothing on a terminal anyway.
        let owned;
        let s = if s.contains(QUOTED_BLANK) {
            owned = s.replace(QUOTED_BLANK, "");
            owned.as_str()
        } else {
            s
        };
        if s.is_empty() {
            return;
        }
        // A new line starts without the space due (start_line drops it).
        self.flush_breaks();
        if self.soft_space {
            self.out.push(' ');
        }
        self.soft_space = false;
        self.out.push_str(s);
        self.line_start = false;
    }

    /// Write the line breaks due, and what starts the new line.
    fn flush_breaks(&mut self) {
        if self.out.is_empty() {
            self.pending = 0;
            self.start_line();
            return;
        }
        if self.pending > 0 {
            for i in 0..self.pending {
                trim_line_end(&mut self.out);
                self.out.push('\n');
                // A blank line inside a quote still says it is quoted.
                let depth = self.pending_quote.min(self.quote);
                if i + 1 < self.pending && depth > 0 {
                    self.out.push(QUOTED_BLANK);
                    self.out.push_str(&">".repeat(depth));
                }
            }
            self.pending = 0;
            self.start_line();
        } else if self.line_start {
            self.start_line();
        }
    }

    fn start_line(&mut self) {
        if self.quote > 0 {
            self.out.push_str(&">".repeat(self.quote));
            self.out.push(' ');
        }
        if let Some(mark) = self.item_mark.take() {
            self.out.push_str(&mark);
        } else if !self.lists.is_empty() {
            self.out.push_str(&"  ".repeat(self.lists.len()));
        }
        self.line_start = false;
        self.soft_space = false;
    }

    /// Ask for `n` line breaks before the next text: 1 ends a line, 2 a
    /// paragraph. Asked twice, the larger stands.
    fn newline(&mut self, n: usize) {
        if !self.out.is_empty() {
            self.pending_quote = if self.pending == 0 {
                self.quote
            } else {
                self.pending_quote.min(self.quote)
            };
            self.pending = self.pending.max(n);
        }
        self.soft_space = false;
    }

    /// Act on tag `t`; the name of an element whose content is skipped.
    fn tag(&mut self, t: &Tag) -> Option<&'static str> {
        match (t.name.as_str(), t.closing) {
            ("script", false) => return Some("script"),
            ("style", false) => return Some("style"),
            ("template", false) => return Some("template"),
            ("br", _) => {
                if let Some(a) = &mut self.anchor {
                    a.text.push(' ');
                } else if self.out.is_empty() {
                    // A break before any text breaks nothing.
                } else if self.pending == 0 {
                    // A line break, as many as there are, up to a blank
                    // line: the next text is on a new line.
                    self.pending = 1;
                } else {
                    self.pending = (self.pending + 1).min(2);
                }
                self.soft_space = false;
            }
            ("a", false) => {
                self.close_anchor();
                let class = t.attr("class");
                let rel = t.attr("rel");
                let named = class
                    .split_whitespace()
                    .any(|c| c == "mention" || c == "hashtag")
                    || rel.split_whitespace().any(|r| r == "tag");
                self.anchor = Some(Anchor {
                    href: t.attr("href").trim().to_string(),
                    named,
                    text: String::new(),
                    space_before: std::mem::take(&mut self.soft_space),
                });
            }
            ("a", true) => self.close_anchor(),
            ("img", _) => {
                let alt = t.attr("alt").to_string();
                if !alt.is_empty() {
                    self.put(&alt, false);
                }
            }
            (
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "pre" | "ul" | "ol"
                | "blockquote" | "figure" | "hr" | "table",
                closing,
            ) => {
                self.close_anchor();
                // A list inside a list item is on the next line, not after a
                // blank one.
                let nested = matches!(t.name.as_str(), "ul" | "ol")
                    && self.lists.len() > usize::from(closing);
                self.newline(if nested { 1 } else { 2 });
                match (t.name.as_str(), closing) {
                    ("blockquote", false) => self.quote += 1,
                    ("blockquote", true) => self.quote = self.quote.saturating_sub(1),
                    ("pre", false) => self.pre += 1,
                    ("pre", true) => self.pre = self.pre.saturating_sub(1),
                    ("ul", false) => self.lists.push(List::Bullets),
                    ("ol", false) => {
                        let start = t.attr("start").trim().parse().unwrap_or(1);
                        self.lists.push(List::Numbers(start));
                    }
                    ("ul" | "ol", true) => {
                        self.lists.pop();
                        self.item_mark = None;
                    }
                    _ => {}
                }
            }
            ("li", false) => {
                self.close_anchor();
                self.newline(1);
                let mark = match self.lists.last_mut() {
                    Some(List::Numbers(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "- ".to_string(),
                };
                let indent = "  ".repeat(self.lists.len().saturating_sub(1));
                self.item_mark = Some(format!("{indent}{mark}"));
                self.line_start = true;
            }
            ("li" | "tr", true) | ("tr", false) => {
                self.close_anchor();
                self.newline(1);
            }
            _ => {}
        }
        None
    }

    /// Write the `<a>` being read, as a mention, a hashtag or a link reads.
    fn close_anchor(&mut self) {
        let Some(a) = self.anchor.take() else {
            return;
        };
        let text = a.text.trim().to_string();
        let shown = if a.named || !is_web(&a.href) || same_address(&text, &a.href) {
            if text.is_empty() && is_web(&a.href) {
                a.href
            } else if same_address(&text, &a.href) && !a.named && text.ends_with('…') {
                // A cut address reads whole.
                a.href
            } else {
                text
            }
        } else if text.is_empty() {
            a.href
        } else {
            format!("{text} ({})", a.href)
        };
        // The space due before the link goes before it, and one due at its
        // end after it.
        let after = self.soft_space;
        self.soft_space = a.space_before;
        self.put(&shown, false);
        self.soft_space = after;
    }

    fn finish(mut self) -> String {
        self.close_anchor();
        // Lines end without spaces, and no more than one blank line is in a
        // row: a line of a quote's `>` alone is a blank one too.
        let mut lines: Vec<(bool, &str)> = Vec::new();
        let mut blank = 0;
        for line in self.out.split('\n') {
            let line = line.trim_end_matches([' ', '\u{a0}']);
            let (empty, line) = match line.strip_prefix(QUOTED_BLANK) {
                Some(marks) => (true, marks),
                None => (line.is_empty(), line),
            };
            if empty {
                blank += 1;
                if blank > 1 || lines.is_empty() {
                    continue;
                }
            } else {
                blank = 0;
            }
            lines.push((empty, line));
        }
        while lines.last().is_some_and(|(empty, _)| *empty) {
            lines.pop();
        }
        lines.iter().map(|(_, l)| *l).collect::<Vec<_>>().join("\n")
    }
}

/// What starts a blank line inside a quote while it is written: a control
/// character, which no text keeps (see [`Writer::put_inline`]).
const QUOTED_BLANK: char = '\u{1}';

/// Remove the spaces at the end of the last line of `out`.
fn trim_line_end(out: &mut String) {
    let kept = out.trim_end_matches(' ').len();
    out.truncate(kept);
}

fn is_web(href: &str) -> bool {
    let l = href.to_ascii_lowercase();
    l.starts_with("https://") || l.starts_with("http://")
}

/// Whether the text of a link is its address: the same, the same without
/// the scheme or the trailing slash, or the start of it cut with `…`.
fn same_address(text: &str, href: &str) -> bool {
    let bare = |s: &str| {
        let l = s.trim();
        let l = l
            .strip_prefix("https://")
            .or_else(|| l.strip_prefix("http://"))
            .unwrap_or(l);
        l.trim_end_matches('/').to_string()
    };
    let (t, h) = (bare(text), bare(href));
    if t.is_empty() {
        return false;
    }
    if t.eq_ignore_ascii_case(&h) {
        return true;
    }
    match t.strip_suffix('…') {
        Some(cut) => !cut.is_empty() && h.starts_with(cut),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::paragraphs_and_breaks("<p>one<br />two</p><p>three</p>", "one\ntwo\n\nthree")]
    #[case::entities(
        "<p>a &amp; b &lt;tag&gt; &quot;q&quot; &#39;s&#39; &#128008; &#x1F408;&#xfe0f; &nbsp;x</p>",
        "a & b <tag> \"q\" 's' 🐈 🐈\u{fe0f} \u{a0}x"
    )]
    #[case::unknown_entity(
        "<p>&bogus; &amp &#; &#xZZ; &#0;</p>",
        "&bogus; &amp &#; &#xZZ; \u{fffd}"
    )]
    #[case::cjk_and_emoji(
        "<p>今日は👍🏽 1️⃣ ❤️ 🇯🇵 家族👨\u{200d}👩\u{200d}👧 e\u{301}</p>",
        "今日は👍🏽 1️⃣ ❤️ 🇯🇵 家族👨\u{200d}👩\u{200d}👧 e\u{301}"
    )]
    #[case::custom_emoji_stay("<p>hi :blobcat: :ablobcatwave:</p>", "hi :blobcat: :ablobcatwave:")]
    #[case::spaces_typed_stay("<p>a  b   c</p>", "a  b   c")]
    #[case::source_layout_is_one_space("<p>\n  a\n  b\n</p>\n<p>c</p>\n", "a b\n\nc")]
    #[case::many_breaks_one_blank_line("<p>a<br><br><br><br>b</p>", "a\n\nb")]
    #[case::stray_less_than("<p>1 < 2 and 3 > 1, a <= b</p>", "1 < 2 and 3 > 1, a <= b")]
    #[case::script_is_not_text("<p>a<script>alert('x')</script>b<style>p{}</style>c</p>", "abc")]
    #[case::comment("<p>a<!-- hidden -->b</p>", "ab")]
    #[case::img_alt(
        "<p>hey <img src=\"x.png\" alt=\":blob:\"> there</p>",
        "hey :blob: there"
    )]
    #[case::empty("", "")]
    #[case::only_tags("<p></p><p><br></p>", "")]
    #[case::plain("no html at all", "no html at all")]
    fn html_reads_as_its_text(#[case] html: &str, #[case] want: &str) {
        assert_eq!(to_text(html), want);
    }

    // What Mastodon makes of a mention, a hashtag and a long link: the
    // mention and the hashtag as typed, the link whole, its hidden spans
    // and all.
    #[test]
    fn mastodon_links_read_as_typed() {
        let html = concat!(
            "<p><span class=\"h-card\" translate=\"no\"><a href=\"https://mastodon.social/@Gargron\" class=\"u-url mention\">@<span>Gargron</span></a></span> ",
            "<a href=\"https://mastodon.social/tags/rust\" class=\"mention hashtag\" rel=\"tag\">#<span>rust</span></a> ",
            "<a href=\"https://example.com/very/long/path?a=1&amp;b=2\" target=\"_blank\" rel=\"nofollow noopener noreferrer\" translate=\"no\">",
            "<span class=\"invisible\">https://</span><span class=\"ellipsis\">example.com/very/lo</span><span class=\"invisible\">ng/path?a=1&amp;b=2</span></a></p>",
        );
        assert_eq!(
            to_text(html),
            "@Gargron #rust https://example.com/very/long/path?a=1&b=2"
        );
    }

    // A link whose text is not its address keeps the address after it; one
    // whose text is its address cut with … reads whole; one that is not a
    // web address reads as its text.
    #[rstest]
    #[case(
        "<a href=\"https://e.test/a\">the article</a>",
        "the article (https://e.test/a)"
    )]
    #[case("<a href=\"https://e.test/a\">e.test/a</a>", "e.test/a")]
    #[case(
        "<a href=\"https://e.test/a/\">https://e.test/a</a>",
        "https://e.test/a"
    )]
    #[case(
        "<a href=\"https://e.test/abcdef\">e.test/abc…</a>",
        "https://e.test/abcdef"
    )]
    #[case("<a href=\"https://e.test/a\"></a>", "https://e.test/a")]
    #[case("<a href=\"mailto:x@e.test\">write</a>", "write")]
    #[case("<a>bare</a>", "bare")]
    #[case("<a href=https://e.test/u>unquoted</a>", "unquoted (https://e.test/u)")]
    #[case(
        "<a href=\"https://misskey.test/@bob\" class=\"u-url mention\">@bob@misskey.test</a>",
        "@bob@misskey.test"
    )]
    #[case(
        "see <a href=\"https://e.test/\">here</a>.",
        "see here (https://e.test/)."
    )]
    fn a_link_keeps_its_address_when_its_text_is_not_it(#[case] html: &str, #[case] want: &str) {
        assert_eq!(to_text(html), want);
    }

    #[test]
    fn quotes_lists_and_preformatted_text() {
        assert_eq!(
            to_text("<p>before</p><blockquote><p>q1<br>q2</p><p>q3</p></blockquote><p>after</p>"),
            "before\n\n> q1\n> q2\n>\n> q3\n\nafter"
        );
        assert_eq!(
            to_text(
                "<ul><li>one</li><li>two</li></ul><ol start=\"3\"><li>three</li><li>four</li></ol>"
            ),
            "- one\n- two\n\n3. three\n4. four"
        );
        assert_eq!(
            to_text("<pre><code>fn main() {\n    let x = 1;\n}</code></pre><p>end</p>"),
            "fn main() {\n    let x = 1;\n}\n\nend"
        );
        assert_eq!(to_text("<h2>Title</h2><p>body</p>"), "Title\n\nbody");
    }

    // What the rarer tags and layouts do, one by one.
    #[rstest]
    #[case::declarations("<!DOCTYPE html><?xml version=\"1.0\"?><p>a</p>", "a")]
    #[case::template("<template><p>x</p></template><p>y</p>", "y")]
    #[case::table("<table><tr><td>a</td></tr><tr><td>b</td></tr></table>", "a\nb")]
    #[case::nested_lists("<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>", "- a\n  - b\n- c")]
    #[case::item_on_two_lines("<ul><li>a<br>b</li></ul>", "- a\n  b")]
    #[case::text_after_pre("<pre>x  y</pre>a\nb", "x  y\n\na b")]
    #[case::blank_line_in_pre("<pre>a\n\nb\n</pre>", "a\n\nb")]
    #[case::newline_entity("<p>a&#10;b</p><pre>c&#10;d</pre>", "a b\n\nc\nd")]
    #[case::space_before_a_link(
        "<p>see\n<a href=\"https://e.test/\">e.test</a>\nnow</p>",
        "see e.test now"
    )]
    #[case::tab_typed("<p>a\tb</p>", "a\tb")]
    #[case::formfeed("<p>a\u{c}b</p>", "a b")]
    fn rarer_html_reads_as_its_text(#[case] html: &str, #[case] want: &str) {
        assert_eq!(to_text(html), want);
    }

    // A mention or a hashtag is told by its class or by rel="tag" alone,
    // and reads as its text; a quoted `>` in an attribute does not end the
    // tag, and the attributes after a quoted one are read.
    #[rstest]
    #[case("<a href=\"https://m.test/tags/x\" rel=\"tag\">#x</a>", "#x")]
    #[case("<a href=\"https://m.test/tags/x\" class=\"hashtag\">#x</a>", "#x")]
    #[case("<a href=\"https://m.test/@a\" class=\"u-url mention\">@a</a>", "@a")]
    #[case("<a class=\"mention\" href=\"https://m.test/@a\">@a</a>", "@a")]
    #[case(
        "<a title='x' href=\"https://e.test/?a>b\">t</a>",
        "t (https://e.test/?a>b)"
    )]
    #[case("<a href=\"https://e.test\">…</a>", "… (https://e.test)")]
    fn links_are_told_apart_by_what_they_say_they_are(#[case] html: &str, #[case] want: &str) {
        assert_eq!(to_text(html), want);
    }

    // A tag that is not closed, a quote that never ends, and the like are
    // text or nothing, never a panic.
    #[rstest]
    #[case("<p>a<", "a<")]
    #[case("<p>a <b", "a <b")]
    #[case("<a href=\"x>y", "<a href=\"x>y")]
    #[case("<p>a</p", "a</p")]
    #[case("&", "&")]
    #[case("&#99999999999;", "\u{fffd}")]
    #[case("<!-- unclosed", "")]
    #[case("<script>never closed", "")]
    fn broken_html_is_read_as_far_as_it_goes(#[case] html: &str, #[case] want: &str) {
        assert_eq!(to_text(html), want);
    }
}
