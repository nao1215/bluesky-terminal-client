//! A page's link card is read from any text without panicking: its picture
//! is a web address or nothing, its title and description are trimmed with
//! single spaces, and what follows `<body` does not change it.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/og.rs"]
mod og;

const BASE: &str = "https://example.com/a/b?c=1#d";

fn check_text(text: &str) {
    assert_eq!(text, text.trim(), "{text:?} is not trimmed");
    assert!(
        !text.chars().any(|c| c.is_whitespace() && c != ' ') && !text.contains("  "),
        "{text:?} keeps white space as written"
    );
}

fuzz_target!(|data: &[u8]| {
    let html = String::from_utf8_lossy(data);
    let page = og::parse(&html, BASE);
    check_text(&page.title);
    check_text(&page.description);
    if let Some(image) = &page.image {
        let lower = image.to_ascii_lowercase();
        assert!(
            lower.starts_with("http://") || lower.starts_with("https://"),
            "{image:?} is not a web address"
        );
        assert!(
            !image.chars().any(|c| c.is_control() || c == ' '),
            "{image:?} has a space or a control character"
        );
    }
    let longer = format!("{html}<body><meta property=\"og:title\" content=\"body\"></head>");
    assert_eq!(og::parse(&longer, BASE), page, "the body changed the card");
});
