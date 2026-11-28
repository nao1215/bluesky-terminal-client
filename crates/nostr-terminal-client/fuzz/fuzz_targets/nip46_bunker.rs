#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = api::nip46::Bunker::parse(text);
    // A valid public key lets mutations explore query parameters as well.
    let _ = api::nip46::Bunker::parse(&format!(
        "bunker://79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798?{text}"
    ));
});
