//! The links msky finds in a note's MFM are ranges of the text: each starts
//! and ends on a character boundary, none overlaps the next, and each holds
//! the address it names. Silencing the links leaves none that gets a preview
//! and reads as the same plain text, the plain text never grows, and a line
//! before and after the note does not move what is found inside it.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/mfm.rs"]
mod mfm;

use mfm::{Form, Link};

fn check(text: &str, links: &[Link]) {
    let mut last = 0;
    for link in links {
        assert!(link.start < link.end, "empty {link:?}");
        assert!(link.end <= text.len(), "{link:?} past {}", text.len());
        assert!(last <= link.start, "{link:?} overlaps the link before");
        assert!(
            text.is_char_boundary(link.start) && text.is_char_boundary(link.end),
            "{link:?} cuts a character"
        );
        last = link.end;
        let lower = link.url.to_ascii_lowercase();
        assert!(
            lower.starts_with("https://") || lower.starts_with("http://"),
            "{:?} is not a web address",
            link.url
        );
        let covered = &text[link.start..link.end];
        match link.form {
            Form::Bare => assert_eq!(covered, link.url),
            Form::Angle => assert_eq!(covered, format!("<{}>", link.url)),
            Form::Labeled { silent } => {
                assert_eq!(covered.starts_with('?'), silent, "{covered:?}");
                assert!(covered.ends_with(')'), "{covered:?}");
                assert!(covered.contains(link.url.as_str()), "{covered:?}");
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let links = mfm::links(text);
    check(text, &links);

    // What previews a note gets is a subset of its links, each once.
    let previewed = mfm::previewed(text, &[]);
    assert!(previewed.len() <= links.len());
    for url in &previewed {
        assert!(links.iter().any(|l| &l.url == url));
    }

    // A silenced note gets no preview and shows the same text.
    let silenced = mfm::silenced(text);
    assert_eq!(
        mfm::previewed(&silenced, &[]),
        Vec::<String>::new(),
        "{text:?}"
    );
    assert_eq!(
        mfm::plain_text(&silenced),
        mfm::plain_text(text),
        "{text:?}"
    );

    // Markup is only taken away.
    let plain = mfm::plain_text(text);
    assert!(plain.len() <= text.len(), "{text:?} grew to {plain:?}");

    // A line before the text and one after it only move the links by the
    // line break. (A space would not do: a code block opens only at the
    // start of a line.)
    let padded = format!("\n{text}\n");
    let shifted: Vec<Link> = links
        .iter()
        .map(|l| Link {
            start: l.start + 1,
            end: l.end + 1,
            ..l.clone()
        })
        .collect();
    assert_eq!(mfm::links(&padded), shifted, "{text:?}");
});
