//! A status's HTML reads as text without a panic, whatever the server
//! sent: no line ends with a space, no more than one blank line is in a
//! row and none is at the start or the end, and the mark a blank line of a
//! quote is written with while it is read never reaches the text.
//!
//! The input is the HTML.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../src/html.rs"]
mod html;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let text = html::to_text(source);
    for line in text.split('\n') {
        assert!(!line.ends_with(' '), "{source:?} read as {text:?}");
    }
    assert!(!text.contains("\n\n\n"), "{source:?} read as {text:?}");
    assert!(
        !text.starts_with('\n') && !text.ends_with('\n'),
        "{source:?} read as {text:?}"
    );
    assert!(!text.contains('\u{1}'), "{source:?} read as {text:?}");
});
