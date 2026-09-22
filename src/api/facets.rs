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
    /// An `http://` or `https://` URL.
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

/// Characters that end a sentence rather than a URL, handle, or tag.
const TRAILING: &[char] = &[
    '.', ',', ';', ':', '!', '?', ')', ']', '}', '"', '\'', '」', '。', '、',
];

/// Find every link, mention, and hashtag in `text`.
pub fn detect(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for (start, token) in tokens(text) {
        if token.starts_with("https://") || token.starts_with("http://") {
            let url = trim_url(token);
            if url.len() > url.find("://").unwrap() + 3 {
                spans.push(Span {
                    start,
                    end: start + url.len(),
                    target: Target::Link(url.to_string()),
                });
            }
        } else if let Some(rest) = token.strip_prefix('@') {
            // A handle is ASCII, so it ends at the first character that
            // cannot be in one: "@alice.test、こんにちは" mentions alice.test.
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
                .unwrap_or(rest.len());
            let handle = rest[..len].trim_end_matches('.');
            if is_handle(handle) {
                spans.push(Span {
                    start,
                    end: start + 1 + handle.len(),
                    target: Target::Mention(handle.to_string()),
                });
            }
        } else if let Some(tag) = token.trim_end_matches(TRAILING).strip_prefix('#')
            && is_tag(tag)
        {
            spans.push(Span {
                start,
                end: start + 1 + tag.len(),
                target: Target::Tag(tag.to_string()),
            });
        }
    }
    spans
}

/// Strip sentence punctuation from the end of a URL, except a closing
/// parenthesis the URL itself opened (`https://en.wikipedia.org/wiki/Rust_(programming_language)`).
fn trim_url(token: &str) -> &str {
    let mut url = token;
    while let Some(c) = url.chars().next_back() {
        if !TRAILING.contains(&c) {
            break;
        }
        if c == ')' && url.matches('(').count() >= url.matches(')').count() {
            break;
        }
        url = &url[..url.len() - c.len_utf8()];
    }
    url
}

/// Whitespace-separated tokens with their byte offsets. A leading `(` is
/// dropped so `(https://x)` still yields the URL.
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
        let stripped = tok.trim_start_matches(['(', '[', '「']);
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

fn is_tag(s: &str) -> bool {
    !s.is_empty()
        && s.chars().count() <= 64
        && !s.chars().all(|c| c.is_ascii_digit())
        && !s.contains('#')
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

    #[test]
    fn mention_without_did_has_no_facet() {
        let span = &detect("@alice.test")[0];
        assert!(to_json(span, None).is_none());
        let json = to_json(span, Some("did:plc:alice")).unwrap();
        assert_eq!(json["features"][0]["did"], "did:plc:alice");
        assert_eq!(json["index"]["byteEnd"], 11);
    }
}
