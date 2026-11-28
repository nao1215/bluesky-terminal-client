//! The actual Link parser, extracted at build time without network dependencies.
#![no_main]
use libfuzzer_sys::fuzz_target;
include!(concat!(env!("OUT_DIR"), "/link.rs"));

fuzz_target!(|text: &str| {
    let parsed = next_of(Some(text));
    assert_eq!(next_of(Some(text.trim())), parsed);
    assert_eq!(next_of(None), Next::Unsaid);
    if !text.is_empty() {
        let header = format!(
            "<http://127.0.0.1/?max_id={}>; rel=\"next\"",
            percent_encode(text)
        );
        assert_eq!(next_of(Some(&header)), Next::MaxId(text.to_owned()));
    }
});
