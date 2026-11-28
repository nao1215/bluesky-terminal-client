#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Recompute checksums over mutated TLV bytes to explore past the checksum gate.
    for prefix in ["npub", "nsec", "note", "nprofile", "nevent"] {
        if let Ok(encoded) =
            bech32::encode::<bech32::Bech32>(bech32::Hrp::parse(prefix).unwrap(), data)
        {
            check(&encoded);
        }
    }
    if let Ok(text) = std::str::from_utf8(data) {
        check(text);
    }
});

fn check(text: &str) {
    if let Ok(id) = nip19::Identifier::decode(text) {
        let encoded = id.encode().unwrap();
        assert!(nip19::Identifier::decode(&encoded).unwrap() == id);
    }
}
