//! The facets found in a post are ranges Bluesky can use: each is a
//! non-empty UTF-8 byte range inside the text that starts and ends on a
//! character boundary, covers the text its target names, and no two ranges
//! overlap. The facet JSON carries the same range, and text around the post
//! does not move what is found inside it.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/api/facets.rs"]
mod facets;

use facets::{Span, Target};

fn check(text: &str, spans: &[Span]) {
    for span in spans {
        assert!(span.start < span.end, "empty {span:?}");
        assert!(span.end <= text.len(), "{span:?} past {}", text.len());
        assert!(
            text.is_char_boundary(span.start),
            "{span:?} starts inside a character"
        );
        assert!(
            text.is_char_boundary(span.end),
            "{span:?} ends inside a character"
        );
        let covered = &text[span.start..span.end];
        match &span.target {
            Target::Link(uri) => assert!(
                uri == covered || uri.strip_prefix("https://") == Some(covered),
                "{covered:?} links to {uri:?}"
            ),
            Target::Mention(handle) => {
                assert_eq!(covered.strip_prefix('@'), Some(handle.as_str()));
            }
            Target::Tag(tag) => assert!(
                covered.strip_prefix('#') == Some(tag) || covered.strip_prefix('＃') == Some(tag),
                "{covered:?} tags {tag:?}"
            ),
        }

        let json = facets::to_json(span, Some("did:plc:fuzz")).expect("a DID was given");
        assert_eq!(json["index"]["byteStart"], span.start);
        assert_eq!(json["index"]["byteEnd"], span.end);
        if matches!(span.target, Target::Mention(_)) {
            assert!(facets::to_json(span, None).is_none());
        }
    }
    // The lexicon does not ask for facets in order, and a tag glued to a
    // mention is found after it, but two over the same text are never drawn.
    let mut sorted: Vec<&Span> = spans.iter().collect();
    sorted.sort_by_key(|s| s.start);
    for pair in sorted.windows(2) {
        assert!(
            pair[0].end <= pair[1].start,
            "{:?} and {:?} overlap in {text:?}",
            pair[0],
            pair[1]
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let spans = facets::detect(text);
    check(text, &spans);

    // A space before or after the text only moves the ranges by its length.
    let padded = format!(" {text} ");
    let shifted: Vec<Span> = spans
        .iter()
        .map(|s| Span {
            start: s.start + 1,
            end: s.end + 1,
            target: s.target.clone(),
        })
        .collect();
    assert_eq!(facets::detect(&padded), shifted, "{text:?}");
});
