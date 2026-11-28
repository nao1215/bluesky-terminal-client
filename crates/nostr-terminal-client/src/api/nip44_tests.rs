//! Selected paulmillr/nip44 vectors and generated authenticated-payload properties.
use super::*;
use k256::{elliptic_curve::Generate, sha2::Digest};
use proptest::prelude::*;
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("../../tests/testdata/nip44-v2.json")).unwrap()
}
fn raw(value: &Value) -> [u8; 32] {
    hex::decode(value.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

#[test]
fn official_key_derivation_padding_and_payload_vectors() {
    let vectors = vectors();
    for v in vectors["conversation"].as_array().unwrap() {
        let secret = SigningKey::from_slice(&raw(&v["sec1"])).unwrap();
        assert_eq!(
            *conversation_key(&secret, &raw(&v["pub2"])).unwrap(),
            raw(&v["conversation_key"])
        );
    }
    let v = &vectors["message_keys"];
    let keys = message_keys(&raw(&v["conversation_key"]), &raw(&v["nonce"])).unwrap();
    assert_eq!(hex::encode(&keys[..32]), v["chacha_key"]);
    assert_eq!(hex::encode(&keys[32..44]), v["chacha_nonce"]);
    assert_eq!(hex::encode(&keys[44..]), v["hmac_key"]);
    for v in vectors["padding"].as_array().unwrap() {
        assert_eq!(
            padded_len(v[0].as_u64().unwrap() as usize),
            v[1].as_u64().unwrap() as usize
        );
    }
    for v in vectors["encrypt_decrypt"].as_array().unwrap() {
        let a = SigningKey::from_slice(&raw(&v["sec1"])).unwrap();
        let b = SigningKey::from_slice(&raw(&v["sec2"])).unwrap();
        let key = conversation_key(&a, &b.verifying_key().to_bytes().into()).unwrap();
        assert_eq!(*key, raw(&v["conversation_key"]));
        let payload = encrypt(&key, &raw(&v["nonce"]), v["plaintext"].as_str().unwrap()).unwrap();
        assert_eq!(payload, v["payload"]);
        assert_eq!(
            decrypt(&key, &payload).unwrap().as_str(),
            v["plaintext"].as_str().unwrap()
        );
    }
    let v = &vectors["long"];
    let text = v["pattern"]
        .as_str()
        .unwrap()
        .repeat(v["repeat"].as_u64().unwrap() as usize);
    let payload = encrypt(&raw(&v["conversation_key"]), &raw(&v["nonce"]), &text).unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(text.as_bytes())),
        v["plaintext_sha256"]
    );
    assert_eq!(
        hex::encode(Sha256::digest(payload.as_bytes())),
        v["payload_sha256"]
    );
    assert_eq!(
        decrypt(&raw(&v["conversation_key"]), &payload)
            .unwrap()
            .as_str(),
        text
    );
}

#[test]
fn official_invalid_vectors_and_length_boundaries() {
    let vectors = vectors();
    for v in vectors["invalid_decrypt"].as_array().unwrap() {
        assert!(
            decrypt(&raw(&v["conversation_key"]), v["payload"].as_str().unwrap()).is_err(),
            "{}",
            v["note"]
        );
    }
    for v in vectors["invalid_public"].as_array().unwrap() {
        assert!(conversation_key(&SigningKey::generate(), &raw(v)).is_err());
    }
    for len in [0, 65536, 100000] {
        assert!(encrypt(&[1; 32], &[2; 32], &"x".repeat(len)).is_err());
    }
    for text in ["", "Ag==", "!!!!"] {
        assert!(decrypt(&[1; 32], text).is_err());
    }
    assert!(decrypt(&[1; 32], &"A".repeat(MAX_PAYLOAD + 1)).is_err());
    for len in [1, 31, 32, 33, 255, 256, 257, 65534, 65535] {
        let text = "x".repeat(len);
        assert_eq!(
            *decrypt(&[1; 32], &encrypt(&[1; 32], &[2; 32], &text).unwrap()).unwrap(),
            text
        );
    }
}

proptest! {
    #[test]
    fn roundtrip_ecdh_symmetry_and_tamper_detection(text in ".{1,512}", nonce in any::<[u8;32]>(), bit in 0usize..8, offset in any::<usize>()) {
        let a = SigningKey::generate();
        let b = SigningKey::generate();
        let ab = conversation_key(&a, &b.verifying_key().to_bytes().into()).unwrap();
        let ba = conversation_key(&b, &a.verifying_key().to_bytes().into()).unwrap();
        prop_assert_eq!(*ab, *ba);
        let payload = encrypt(&ab, &nonce, &text).unwrap();
        let decrypted = decrypt(&ba, &payload).unwrap();
        prop_assert_eq!(decrypted.as_str(), text.as_str());
        let mut data = STANDARD.decode(payload).unwrap();
        let index = offset % data.len();
        data[index] ^= 1 << bit;
        prop_assert!(decrypt(&ba, &STANDARD.encode(data)).is_err());
    }
    #[test]
    fn vector_payloads_detect_every_generated_bit_change(case in 0usize..3, offset in any::<usize>(), bit in 0u8..8) {
        let vectors = vectors();
        let v = &vectors["encrypt_decrypt"][case];
        let mut data = STANDARD.decode(v["payload"].as_str().unwrap()).unwrap();
        let index = offset % data.len();
        data[index] ^= 1 << bit;
        prop_assert!(decrypt(&raw(&v["conversation_key"]), &STANDARD.encode(data)).is_err());
    }
    #[test]
    fn arbitrary_payloads_are_rejected_without_panicking(text in ".{0,1024}") { let _result = decrypt(&[1;32], &text); }
}
