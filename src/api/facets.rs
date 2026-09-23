//! Rich-text facets for new posts: links, mentions, and hashtags.
//!
//! Bluesky does not parse post text on the server; a link is only clickable
//! when the client sends an `app.bsky.richtext.facet` with the UTF-8 byte
//! range it covers. [`detect`] finds the ranges, and the caller turns
//! mentions into facets once their handles resolve to DIDs.

use serde_json::{Value, json};

/// What a detected range points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An `http://` or `https://` URL. A bare domain in the text
    /// (`example.com/a`) links to it with `https://` put in front.
    Link(String),
    /// A handle, without the leading `@`.
    Mention(String),
    /// A hashtag, without the leading `#`.
    Tag(String),
}

/// A detected facet: the UTF-8 byte range `[start, end)` and its target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub target: Target,
}

/// Characters that end a sentence rather than a URL, including the
/// full-width ones Japanese and Chinese text closes with.
const TRAILING: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '"', '\'', '」', '。', '、', '！', '？', '）',
    '』', '】', '〉', '》', '〕', '］', '｝', '，', '．', '：', '；', '…', '”', '’',
];

/// Full-width punctuation that ends a link even with no space before the
/// text that follows, as in `https://example.com、あと`. The full-width
/// parentheses are not here: an address may hold ones it opened itself,
/// which [`trim_url`] keeps.
const URL_BREAK: &[char] = &[
    '、', '。', '，', '．', '！', '？', '：', '；', '「', '」', '『', '』', '【', '】', '〈', '〉',
    '《', '》', '〔', '〕', '［', '］', '｛', '｝', '　', '…', '〜', '“', '”', '‘', '’',
];

/// Brackets and quotes that open before a URL, handle, or tag.
const LEADING: &[char] = &[
    '(', '[', '「', '（', '『', '【', '〈', '《', '〔', '［', '｛', '“', '‘', '"', '\'',
];

/// Find every link, mention, and hashtag in `text`.
pub fn detect(text: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for (start, token) in tokens(text) {
        let first = spans.len();
        spans.extend(span_at(start, token));
        // Text without spaces glues a parenthesized link or mention to what
        // comes before it: each opening parenthesis in the token may start
        // one, unless it sits inside a span already found (a link's own
        // parentheses).
        for (i, open) in token.char_indices() {
            let close = match open {
                '(' => ')',
                '（' => '）',
                _ => continue,
            };
            let at = start + i;
            if spans[first..]
                .iter()
                .any(|s| (s.start..s.end).contains(&at))
            {
                continue;
            }
            let rest = &token[i + open.len_utf8()..];
            let rest = until_unmatched(rest, open, close);
            let inner = rest.trim_start_matches(LEADING);
            let inner_start = at + open.len_utf8() + (rest.len() - inner.len());
            if let Some(span) = span_at(inner_start, inner)
                && !spans[first..].iter().any(|s| s.start == span.start)
            {
                spans.push(span);
            }
        }
        // A tag runs to the next space, so a mention or a link written
        // straight after one is inside the token: `#Rust【@alice.test】`.
        // The tag stops there (see `tag_of`) and that span is taken too, so
        // the person is mentioned rather than spelled into the tag.
        if spans[first..]
            .iter()
            .any(|s| matches!(s.target, Target::Tag(_)))
            && let Some(i) = glued(token)
            && let Some(span) = span_at(start + i, &token[i..])
            && !spans[first..]
                .iter()
                .any(|s| (s.start..s.end).contains(&span.start))
        {
            spans.push(span);
        }
    }
    spans
}

/// Where a mention or an http(s) link begins inside `s`, past its first
/// character. A tag ends there, and the span itself is taken as well.
fn glued(s: &str) -> Option<usize> {
    s.char_indices()
        .skip(1)
        .find(|&(i, c)| {
            let rest = &s[i..];
            match c {
                '@' => matches!(
                    span_at(0, rest),
                    Some(Span {
                        target: Target::Mention(_),
                        ..
                    })
                ),
                _ => rest.starts_with("https://") || rest.starts_with("http://"),
            }
        })
        .map(|(i, _)| i)
}

/// `s` up to the `close` that has no `open` before it in `s`.
fn until_unmatched(s: &str, open: char, close: char) -> &str {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            if depth == 0 {
                return &s[..i];
            }
            depth -= 1;
        }
    }
    s
}

/// The link, mention, or tag `token` (at byte `start` of the text) begins
/// with, if any.
fn span_at(start: usize, token: &str) -> Option<Span> {
    if token.starts_with("https://") || token.starts_with("http://") {
        let token = &token[..token.find(URL_BREAK).unwrap_or(token.len())];
        let url = trim_url(token);
        (url.len() > url.find("://")? + 3).then(|| Span {
            start,
            end: start + url.len(),
            target: Target::Link(url.to_string()),
        })
    } else if let Some(rest) = token.strip_prefix('@') {
        // A handle is ASCII, so it ends at the first character that
        // cannot be in one: "@alice.test、こんにちは" mentions alice.test.
        let len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
            .unwrap_or(rest.len());
        let handle = rest[..len].trim_end_matches('.');
        is_handle(handle).then(|| Span {
            start,
            end: start + 1 + handle.len(),
            target: Target::Mention(handle.to_string()),
        })
    } else if let Some(domain) = bare_domain(token) {
        // The rest of the token belongs to the link too (`example.com/a`),
        // cut and trimmed as a URL written out in full is.
        let token = &token[..token[domain..]
            .find(URL_BREAK)
            .map_or(token.len(), |i| domain + i)];
        let url = trim_url(token);
        Some(Span {
            start,
            end: start + url.len(),
            target: Target::Link(format!("https://{url}")),
        })
    } else {
        let (hash, tag) = tag_of(token)?;
        Some(Span {
            start,
            end: start + hash + tag.len(),
            target: Target::Tag(tag.to_string()),
        })
    }
}

/// The top-level domains a bare domain may end with: Bluesky's app's list.
const TLDS: &str = include_str!("tlds.txt");

/// The byte length of the bare domain `token` starts with (`example.com`
/// in `example.com/a`), read the way Bluesky's app reads one: an ASCII
/// letter, then letters and digits, then one or more dot-separated labels of
/// letters and digits, the last of them a real top-level domain in lower
/// case. So `file.txt`, `v1.2`, and `my-site.com` stay text there and here.
fn bare_domain(token: &str) -> Option<usize> {
    if !token.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut end = token
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(token.len());
    let mut last = None;
    while token[end..].starts_with('.') {
        let label = &token[end + 1..];
        let len = label
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(label.len());
        if len == 0 {
            break;
        }
        last = Some(&label[..len]);
        end += 1 + len;
    }
    let tld = last?;
    TLDS.lines()
        .filter(|l| !l.starts_with('#'))
        .any(|l| l == tld)
        .then_some(end)
}

/// Strip sentence punctuation from the end of a URL, except a closing
/// parenthesis, ASCII or full-width, the URL itself opened
/// (`https://en.wikipedia.org/wiki/Rust_(programming_language)`).
fn trim_url(token: &str) -> &str {
    let mut url = token;
    while let Some(c) = url.chars().next_back() {
        if !TRAILING.contains(&c) {
            break;
        }
        let opener = match c {
            ')' => Some('('),
            '）' => Some('（'),
            _ => None,
        };
        if let Some(open) = opener
            && url.matches(open).count() >= url.matches(c).count()
        {
            break;
        }
        url = &url[..url.len() - c.len_utf8()];
    }
    url
}

/// Whitespace-separated tokens with their byte offsets. Leading brackets and
/// quotes are dropped so `(https://x)` and `（https://x）` still yield the URL.
fn tokens(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push((s, &text[s..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out.into_iter().map(|(s, tok)| {
        let stripped = tok.trim_start_matches(LEADING);
        (s + (tok.len() - stripped.len()), stripped)
    })
}

fn is_handle(s: &str) -> bool {
    let labels: Vec<&str> = s.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
        // The TLD cannot start with a digit (that is an IP address or a typo).
        && !labels.last().unwrap().starts_with(|c: char| c.is_ascii_digit())
}

/// Characters that end a tag without showing: Bluesky's app stops a tag
/// there, so a tag never carries an invisible character the reader cannot
/// see or type.
const TAG_STOP: &[char] = &[
    '\u{ad}', '\u{2060}', '\u{200a}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{20e2}',
];

/// The tag a token starts, as Bluesky's app reads it: `#` or the full-width
/// `＃` Japanese and Chinese keyboards type, then the tag up to a
/// zero-width character, less the punctuation that ends a sentence. A `#`
/// followed by the emoji presentation selector is the keycap emoji `#️⃣`,
/// not a tag. Returns the byte length of the `#` and the tag.
fn tag_of(token: &str) -> Option<(usize, &str)> {
    let hash = token.chars().next().filter(|c| matches!(c, '#' | '＃'))?;
    let rest = &token[hash.len_utf8()..];
    if rest.starts_with('\u{fe0f}') {
        return None;
    }
    let tag = &rest[..rest.find(TAG_STOP).unwrap_or(rest.len())];
    // A mention or a link written straight after the tag is not part of it.
    let tag = &tag[..glued(tag).unwrap_or(tag.len())];
    let tag = tag.trim_end_matches(is_punctuation);
    is_tag(tag).then_some((hash.len_utf8(), tag))
}

/// Bluesky's app takes a tag to be a run with at least one character that
/// is neither an ASCII digit nor punctuation, so `#1.5` and `#--` are text.
fn is_tag(s: &str) -> bool {
    s.chars().count() <= 64
        && s.chars().any(|c| !c.is_ascii_digit() && !is_punctuation(c))
        && !s.contains('#')
}

/// Whether `c` is Unicode punctuation (general category P) in the blocks
/// people type it from: ASCII, Latin-1, General Punctuation, CJK, and the
/// full-width forms. Bluesky's app strips these from the end of a tag, so
/// `#Rust！` tags `Rust` there and must here too.
fn is_punctuation(c: char) -> bool {
    matches!(c,
        '!'..='#' | '%'..='*' | ','..='/' | ':' | ';' | '?' | '@' | '['..=']' | '_' | '{' | '}'
        | '\u{A1}' | '\u{A7}' | '\u{AB}' | '\u{B6}' | '\u{B7}' | '\u{BB}' | '\u{BF}'
        | '\u{2010}'..='\u{2027}' | '\u{2030}'..='\u{2043}' | '\u{2045}'..='\u{2051}'
        | '\u{2053}'..='\u{205E}'
        | '\u{3001}'..='\u{3003}' | '\u{3008}'..='\u{3011}' | '\u{3014}'..='\u{301F}'
        | '\u{30FB}'
        | '\u{FF01}'..='\u{FF03}' | '\u{FF05}'..='\u{FF0A}' | '\u{FF0C}'..='\u{FF0F}'
        | '\u{FF1A}' | '\u{FF1B}' | '\u{FF1F}' | '\u{FF20}' | '\u{FF3B}'..='\u{FF3D}'
        | '\u{FF3F}' | '\u{FF5B}' | '\u{FF5D}' | '\u{FF5F}'..='\u{FF65}')
}

/// The facet JSON for one span. A mention needs its resolved DID; spans of
/// other kinds ignore `did`.
pub fn to_json(span: &Span, did: Option<&str>) -> Option<Value> {
    let feature = match &span.target {
        Target::Link(uri) => json!({"$type": "app.bsky.richtext.facet#link", "uri": uri}),
        Target::Tag(tag) => json!({"$type": "app.bsky.richtext.facet#tag", "tag": tag}),
        Target::Mention(_) => {
            json!({"$type": "app.bsky.richtext.facet#mention", "did": did?})
        }
    };
    Some(json!({
        "index": {"byteStart": span.start, "byteEnd": span.end},
        "features": [feature],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn targets(text: &str) -> Vec<(String, Target)> {
        detect(text)
            .into_iter()
            .map(|s| (text[s.start..s.end].to_string(), s.target))
            .collect()
    }

    #[test]
    fn finds_all_three_kinds() {
        let got = targets("hi @alice.test see https://example.com/a?b=1 #rust");
        assert_eq!(
            got,
            vec![
                ("@alice.test".into(), Target::Mention("alice.test".into())),
                (
                    "https://example.com/a?b=1".into(),
                    Target::Link("https://example.com/a?b=1".into())
                ),
                ("#rust".into(), Target::Tag("rust".into())),
            ]
        );
    }

    // A tag runs to the next space, so in text without spaces it used to
    // swallow a mention or a link written straight after it: the handle went
    // into the tag and nobody was mentioned.
    #[rstest]
    #[case(
        "#Rust\u{3010}@alice.test\u{3011}",
        vec![("#Rust", Target::Tag("Rust".into())), ("@alice.test", Target::Mention("alice.test".into()))]
    )]
    #[case(
        "#rust(@alice.test)",
        vec![("#rust", Target::Tag("rust".into())), ("@alice.test", Target::Mention("alice.test".into()))]
    )]
    #[case(
        "#\u{30bf}\u{30b0}\u{3001}https://example.com/a",
        vec![("#\u{30bf}\u{30b0}", Target::Tag("\u{30bf}\u{30b0}".into())), ("https://example.com/a", Target::Link("https://example.com/a".into()))]
    )]
    // A handle that is not one, and a tag that only looks like the start of
    // a link, are left alone.
    #[case("#rust@notahandle", vec![("#rust@notahandle", Target::Tag("rust@notahandle".into()))])]
    #[case("#webhttp", vec![("#webhttp", Target::Tag("webhttp".into()))])]
    #[case("#@alice.test", vec![("#@alice.test", Target::Tag("@alice.test".into()))])]
    fn a_tag_ends_where_a_mention_or_a_link_begins(
        #[case] text: &str,
        #[case] want: Vec<(&str, Target)>,
    ) {
        let want: Vec<(String, Target)> = want
            .into_iter()
            .map(|(t, target)| (t.to_string(), target))
            .collect();
        assert_eq!(targets(text), want);
    }

    #[test]
    fn byte_offsets_account_for_multibyte_text() {
        let text = "こんにちは https://例.jp です";
        let spans = detect(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(&text[spans[0].start..spans[0].end], "https://例.jp");
        assert_eq!(spans[0].start, "こんにちは ".len());
    }

    #[rstest]
    #[case("see https://example.com.", "https://example.com")]
    #[case("(https://example.com)", "https://example.com")]
    #[case("「https://example.com」", "https://example.com")]
    fn sentence_punctuation_is_not_part_of_a_link(#[case] text: &str, #[case] want: &str) {
        let spans = detect(text);
        assert_eq!(spans.len(), 1, "{text}");
        assert_eq!(&text[spans[0].start..spans[0].end], want);
    }

    #[rstest]
    #[case("@alice")] // no dot
    #[case("@1.2.3.4")] // numeric TLD
    #[case("@-bad.test")]
    #[case("#123")] // digits only
    #[case("#")]
    #[case("https://")]
    #[case("mail@alice.test")] // @ not at token start
    fn rejects_non_facets(#[case] text: &str) {
        assert!(detect(text).is_empty(), "{text}");
    }

    #[rstest]
    #[case("@alice.test、こんにちは", "@alice.test")]
    #[case("@alice.test.", "@alice.test")]
    #[case("@alice.test's post", "@alice.test")]
    fn a_mention_ends_where_a_handle_cannot_continue(#[case] text: &str, #[case] want: &str) {
        let spans = detect(text);
        assert_eq!(spans.len(), 1, "{text}");
        assert_eq!(&text[spans[0].start..spans[0].end], want);
    }

    #[rstest]
    #[case(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        "https://en.wikipedia.org/wiki/Rust_(programming_language)"
    )]
    #[case(
        "(see https://en.wikipedia.org/wiki/Rust_(lang)).",
        "https://en.wikipedia.org/wiki/Rust_(lang)"
    )]
    #[case("(https://example.com/a)", "https://example.com/a")]
    fn a_url_keeps_the_parentheses_it_opened(#[case] text: &str, #[case] want: &str) {
        let spans = detect(text);
        assert_eq!(spans.len(), 1, "{text}");
        assert_eq!(&text[spans[0].start..spans[0].end], want);
    }

    // Bluesky's own detector strips any trailing Unicode punctuation from a
    // tag, not only ASCII, so "#Rust！" tags "Rust" in the official app.
    #[rstest]
    #[case("#Rust！", "#Rust", "Rust")]
    #[case("#rust？ yes", "#rust", "rust")]
    #[case("#日本語。", "#日本語", "日本語")]
    #[case("#タグ』", "#タグ", "タグ")]
    #[case("#tag…", "#tag", "tag")]
    #[case("#tag）", "#tag", "tag")]
    #[case("#tag】", "#tag", "tag")]
    fn a_tag_ends_before_any_trailing_punctuation(
        #[case] text: &str,
        #[case] range: &str,
        #[case] tag: &str,
    ) {
        assert_eq!(targets(text), vec![(range.into(), Target::Tag(tag.into()))]);
    }

    // A tag needs a character that is neither a digit nor punctuation.
    #[rstest]
    #[case("#1.5")]
    #[case("#--")]
    #[case("#3-2")]
    #[case("#！？")]
    fn rejects_a_tag_of_digits_and_punctuation(#[case] text: &str) {
        assert!(detect(text).is_empty(), "{text}");
    }

    #[rstest]
    #[case("（https://example.com）", "https://example.com")]
    #[case("『https://example.com』", "https://example.com")]
    #[case("【https://example.com】", "https://example.com")]
    #[case("見て https://example.com！", "https://example.com")]
    #[case("見て https://example.com？", "https://example.com")]
    #[case("（@alice.test）", "@alice.test")]
    #[case("（#rust）", "#rust")]
    #[case("“#rust”", "#rust")]
    #[case("#rust…", "#rust")]
    fn full_width_brackets_and_punctuation_are_not_part_of_a_facet(
        #[case] text: &str,
        #[case] want: &str,
    ) {
        let spans = detect(text);
        assert_eq!(spans.len(), 1, "{text}");
        assert_eq!(&text[spans[0].start..spans[0].end], want, "{text}");
    }

    #[test]
    fn a_url_keeps_full_width_parentheses_it_opened() {
        let text = "https://ja.wikipedia.org/wiki/東京（曖昧さ回避）";
        let spans = detect(text);
        assert_eq!(&text[spans[0].start..spans[0].end], text);
    }

    /// Tags as Bluesky's own app makes them (checked against @atproto/api's
    /// RichText): a full-width ＃ starts one too, a keycap #️⃣ is an emoji,
    /// not a tag, and a zero-width character ends the tag, so the same text
    /// gets the same tag, and turns up in the same tag search, whichever
    /// client posted it.
    #[rstest]
    #[case("＃タグ", Some((0, 9, "タグ")))]
    #[case("テスト ＃タグ です", Some((10, 19, "タグ")))]
    #[case("＃123", None)]
    #[case("#️⃣", None)]
    #[case("a #️⃣ b", None)]
    #[case("#family👨\u{200d}👩\u{200d}👧 x", Some((0, 11, "family👨")))]
    #[case("#ok\u{200b}after", Some((0, 3, "ok")))]
    #[case("#soft\u{ad}hyphen", Some((0, 5, "soft")))]
    #[case("#👍🏽", Some((0, 9, "👍🏽")))]
    fn tags_match_the_official_app(#[case] text: &str, #[case] want: Option<(usize, usize, &str)>) {
        let got: Vec<(usize, usize, String)> = detect(text)
            .into_iter()
            .filter_map(|s| match s.target {
                Target::Tag(t) => Some((s.start, s.end, t)),
                _ => None,
            })
            .collect();
        let want: Vec<(usize, usize, String)> = want
            .into_iter()
            .map(|(a, b, t)| (a, b, t.to_string()))
            .collect();
        assert_eq!(got, want, "{text:?}");
    }

    /// Japanese and Chinese are written without spaces, so a link or mention
    /// in parentheses is often glued to the text before it. Bluesky's app
    /// starts one after an ASCII `(`; the full-width `（` is taken the same
    /// way. A parenthesis inside a link does not start anything.
    #[rstest]
    #[case("説明はこちら(https://example.com)", &[("https://example.com", "link")])]
    #[case("説明はこちら（https://example.com）です", &[("https://example.com", "link")])]
    #[case("詳細は(@alice.test)まで", &[("@alice.test", "mention")])]
    #[case("テキスト（#タグ）", &[("#タグ", "tag")])]
    #[case("http://a.test/x_(y)", &[("http://a.test/x_(y)", "link")])]
    #[case("https://a.test/(#frag)", &[("https://a.test/(#frag)", "link")])]
    #[case("(https://example.com)", &[("https://example.com", "link")])]
    #[case("f(x)", &[])]
    fn a_facet_glued_after_a_parenthesis_is_found(
        #[case] text: &str,
        #[case] want: &[(&str, &str)],
    ) {
        let got: Vec<(&str, &str)> = detect(text)
            .iter()
            .map(|s| {
                let kind = match s.target {
                    Target::Link(_) => "link",
                    Target::Mention(_) => "mention",
                    Target::Tag(_) => "tag",
                };
                (&text[s.start..s.end], kind)
            })
            .collect();
        assert_eq!(got, want, "{text:?}");
    }

    /// Japanese text follows a link without a space. The link ends at the
    /// first full-width punctuation mark, which a web address does not hold,
    /// while letters of the address (`wiki/東京`) and full-width parentheses
    /// the address opened itself are kept. Bluesky's app links the whole run.
    #[rstest]
    #[case("https://example.com、あと", "https://example.com")]
    #[case("https://example.com！詳しくは@alice.test", "https://example.com")]
    #[case("https://example.com。次の文", "https://example.com")]
    #[case("見て https://example.com」と言った", "https://example.com")]
    #[case(
        "https://ja.wikipedia.org/wiki/東京",
        "https://ja.wikipedia.org/wiki/東京"
    )]
    #[case(
        "https://ja.wikipedia.org/wiki/東京（曖昧さ回避）、ほか",
        "https://ja.wikipedia.org/wiki/東京（曖昧さ回避）"
    )]
    fn a_link_ends_at_full_width_punctuation(#[case] text: &str, #[case] want: &str) {
        let spans = detect(text);
        let links: Vec<&str> = spans
            .iter()
            .filter(|s| matches!(s.target, Target::Link(_)))
            .map(|s| &text[s.start..s.end])
            .collect();
        assert_eq!(links, [want], "{text:?}");
    }

    #[rstest]
    #[case("example.com", "example.com", "https://example.com")]
    #[case(
        "see docs.bsky.app/blog.",
        "docs.bsky.app/blog",
        "https://docs.bsky.app/blog"
    )]
    #[case("Example.com/A?b=1", "Example.com/A?b=1", "https://Example.com/A?b=1")]
    #[case("(example.com)", "example.com", "https://example.com")]
    #[case("説明はこちら(example.jp)", "example.jp", "https://example.jp")]
    #[case("見て example.com、あと", "example.com", "https://example.com")]
    #[case("「example.co.jp」を見て", "example.co.jp", "https://example.co.jp")]
    #[case("😀 example.dev 🎉", "example.dev", "https://example.dev")]
    #[case("main.rs", "main.rs", "https://main.rs")] // .rs is Serbia's
    fn a_bare_domain_links_with_https_in_front(
        #[case] text: &str,
        #[case] covered: &str,
        #[case] uri: &str,
    ) {
        assert_eq!(
            targets(text),
            [(covered.to_string(), Target::Link(uri.to_string()))],
            "{text:?}"
        );
    }

    #[rstest]
    #[case("file.txt")] // not a top-level domain
    #[case("v1.2")]
    #[case("1password.com")] // starts with a digit
    #[case("my-site.com")] // Bluesky's app reads no hyphen in a bare domain
    #[case("EXAMPLE.COM")] // its list is in lower case
    #[case("example.")]
    #[case("example")]
    #[case("me@example.com")]
    #[case("日本語example.com")] // glued inside a word
    #[case("example.コム")]
    fn text_that_is_not_a_bare_domain_stays_text(#[case] text: &str) {
        assert_eq!(targets(text), [], "{text:?}");
    }

    #[test]
    fn a_bare_domain_link_counts_bytes_after_multibyte_text() {
        let text = "日本語 example.com";
        let span = &detect(text)[0];
        let json = to_json(span, None).unwrap();
        assert_eq!(json["index"]["byteStart"], 10);
        assert_eq!(json["index"]["byteEnd"], 21);
        assert_eq!(json["features"][0]["uri"], "https://example.com");
    }

    #[test]
    fn the_tld_list_is_sorted_lower_case_ascii() {
        let names: Vec<&str> = TLDS.lines().filter(|l| !l.starts_with('#')).collect();
        assert!(names.len() > 1000);
        assert!(names.contains(&"com") && names.contains(&"jp") && !names.contains(&"txt"));
        assert!(names.windows(2).all(|w| w[0] < w[1]));
        assert!(names.iter().all(|n| {
            n.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        }));
    }

    #[test]
    fn mention_without_did_has_no_facet() {
        let span = &detect("@alice.test")[0];
        assert!(to_json(span, None).is_none());
        let json = to_json(span, Some("did:plc:alice")).unwrap();
        assert_eq!(json["features"][0]["did"], "did:plc:alice");
        assert_eq!(json["index"]["byteEnd"], 11);
    }
}

#[cfg(test)]
mod differential_dump {
    /// Writes bsky's facets for each JSON string line of $BSKY_FACET_INPUTS
    /// to $BSKY_FACET_OUTPUT, one JSON array of [kind, start, end, value] per
    /// line, the shape the bluesky-dig-bug skill's official-app dumper
    /// writes, so the two can be compared line by line.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "comparison tool"]
    fn dump() {
        let Ok(path) = std::env::var("BSKY_FACET_INPUTS") else {
            return;
        };
        let out_path = std::env::var("BSKY_FACET_OUTPUT").unwrap();
        let mut out = String::new();
        for line in std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
        {
            let text: String = serde_json::from_str(line).unwrap();
            let mut v: Vec<serde_json::Value> = super::detect(&text)
                .into_iter()
                .map(|s| {
                    let (kind, value) = match s.target {
                        super::Target::Link(u) => ("link", u),
                        super::Target::Tag(t) => ("tag", t),
                        super::Target::Mention(h) => ("mention", h),
                    };
                    serde_json::json!([kind, s.start, s.end, value])
                })
                .collect();
            v.sort_by_key(|x| x[1].as_u64());
            out.push_str(&serde_json::to_string(&v).unwrap());
            out.push('\n');
        }
        std::fs::write(out_path, out).unwrap();
    }
}
