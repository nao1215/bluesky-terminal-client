#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let key = [1; 32];
    let _ = nip44::decrypt(&key, text);
    if let Ok(payload) = nip44::encrypt(&key, &[2; 32], text) {
        assert_eq!(nip44::decrypt(&key, &payload).unwrap().as_str(), text);
        use base64::Engine;
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(
            nip44::decrypt(
                &key,
                &base64::engine::general_purpose::STANDARD.encode(bytes)
            )
            .is_err()
        );
    }
});
