#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let key = k256::schnorr::SigningKey::from_slice(&[1; 32]).unwrap();
    let public = hex::encode(key.verifying_key().to_bytes());
    let Ok(payload) = nip44::encrypt(&[1; 32], &[2; 32], text) else {
        return;
    };
    // Authenticate arbitrary plaintext so it reaches the actual response parser.
    let event = event::Event::sign(
        &key,
        1,
        24133,
        vec![vec!["p".into(), public.clone()]],
        payload,
    )
    .unwrap();
    let _ = api::nip46::response(&event, &public, &public, &[1; 32], "request");
    assert_eq!(
        api::nip46::response(&event, &public, "unrelated", &[1; 32], "request").unwrap(),
        None
    );
});
