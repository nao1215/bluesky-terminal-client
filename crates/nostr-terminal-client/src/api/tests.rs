//! Protocol vectors and properties use public test material or freshly generated keys.

use super::{
    event::{Event, bytes},
    nip19::Identifier,
    relay::RelayMessage,
};
use k256::elliptic_curve::Generate;
use k256::schnorr::{Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashVerifier};
use proptest::prelude::*;
use serde_json::json;

fn key() -> SigningKey {
    SigningKey::generate()
}

#[test]
fn bip340_vector_zero() {
    // https://github.com/bitcoin/bips/blob/master/bip-0340/test-vectors.csv, vector 0, referenced by NIP-01.
    let public = VerifyingKey::from_slice(
        &bytes::<32>("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9").unwrap(),
    )
    .unwrap();
    let signature = Signature::try_from(bytes::<64>("e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0").unwrap().as_slice()).unwrap();
    public.verify_prehash(&[0; 32], &signature).unwrap();
    assert!(public.verify_prehash(&[1; 32], &signature).is_err());
    let mut scalar = [0; 32];
    scalar[31] = 3;
    let key = SigningKey::from_slice(&scalar).unwrap();
    assert_eq!(key.verifying_key(), &public);
    assert_eq!(key.sign_raw(&[0; 32], &[0; 32]).unwrap(), signature);
}

#[test]
fn nip19_published_vectors() {
    // https://github.com/nostr-protocol/nips/blob/master/19.md#examples
    for (encoded, kind, raw) in [
        (
            "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg",
            "public",
            "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e",
        ),
        (
            "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5",
            "secret",
            "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa",
        ),
    ] {
        let decoded = Identifier::decode(encoded).unwrap();
        let expected = if kind == "public" {
            Identifier::Public(bytes(raw).unwrap())
        } else {
            Identifier::Secret(bytes(raw).unwrap())
        };
        assert!(decoded == expected);
        assert_eq!(decoded.encode().unwrap(), encoded);
    }
    let encoded = "nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p";
    let expected = Identifier::Profile {
        public: bytes("3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d").unwrap(),
        relays: vec!["wss://r.x.com".into(), "wss://djbas.sadkb.com".into()],
    };
    assert!(Identifier::decode(encoded).unwrap() == expected);
    assert_eq!(expected.encode().unwrap(), encoded);
}

#[test]
fn nip19_rejects_bad_checksum_lengths_duplicates_and_truncated_tlv() {
    use bech32::{Bech32, Bech32m, Hrp};
    let npub = Hrp::parse("npub").unwrap();
    assert!(Identifier::decode(&bech32::encode::<Bech32m>(npub, &[1; 32]).unwrap()).is_err());
    for len in [0, 31, 33] {
        assert!(
            Identifier::decode(&bech32::encode::<Bech32>(npub, &vec![1; len]).unwrap()).is_err()
        );
    }
    let valid = Identifier::Note([0; 32]).encode().unwrap();
    assert!(Identifier::decode(&format!("{}q", &valid[..valid.len() - 1])).is_err());
    assert!(Identifier::decode(&format!("N{}", &valid[1..])).is_err());
    for data in [
        vec![0],
        vec![0, 32, 1],
        vec![0, 0],
        [vec![0, 32], vec![0; 32], vec![0, 32], vec![0; 32]].concat(),
    ] {
        let text = bech32::encode::<Bech32>(Hrp::parse("nprofile").unwrap(), &data).unwrap();
        assert!(Identifier::decode(&text).is_err());
    }
    let data = [vec![0, 32], vec![0; 32], vec![250, 3, 1, 2, 3]].concat();
    let text = bech32::encode::<Bech32>(Hrp::parse("nprofile").unwrap(), &data).unwrap();
    assert!(matches!(
        Identifier::decode(&text).unwrap(),
        Identifier::Profile { .. }
    ));
}

#[test]
fn all_relay_messages_have_strict_types() {
    let event = Event::sign(&key(), 1, 1, vec![], "test".into()).unwrap();
    assert_eq!(
        RelayMessage::parse(&json!(["EVENT", "a", event]).to_string()).unwrap(),
        RelayMessage::Event("a".into(), event)
    );
    for (text, expected) in [
        (json!(["EOSE", "a"]), RelayMessage::Eose("a".into())),
        (
            json!(["OK", "id", false, "blocked: no"]),
            RelayMessage::Ok("id".into(), false, "blocked: no".into()),
        ),
        (
            json!(["NOTICE", "hello"]),
            RelayMessage::Notice("hello".into()),
        ),
        (
            json!(["CLOSED", "a", "restricted"]),
            RelayMessage::Closed("a".into(), "restricted".into()),
        ),
        (
            json!(["AUTH", "challenge"]),
            RelayMessage::Auth("challenge".into()),
        ),
    ] {
        assert_eq!(RelayMessage::parse(&text.to_string()).unwrap(), expected);
    }
    for text in [
        "{}",
        "[]",
        "null",
        "broken",
        r#"["EOSE",3]"#,
        r#"["OK","id",1,""]"#,
        r#"["NOTICE"]"#,
        r#"["AUTH"]"#,
        r#"["AUTH",42]"#,
        r#"["CLOSED","a"]"#,
        r#"["EVENT","a",{}]"#,
    ] {
        assert!(RelayMessage::parse(text).is_err(), "{text}");
    }
}

proptest! {
    #[test]
    fn signed_events_round_trip_and_detect_changes(content in ".{0,256}", time in 0u64..2_000_000_000, tags in prop::collection::vec(prop::collection::vec(".{0,20}", 0..5), 0..8)) {
        let event = Event::sign(&key(), time, 1, tags, content).unwrap();
        event.verify().unwrap();
        let decoded: Event = serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        prop_assert_eq!(&event, &decoded);
        prop_assert_eq!(event.id.clone(), hex::encode(event.digest().unwrap()));
        for changed in 0..6 {
            let mut bad = event.clone();
            match changed {
                0 => bad.content.push('x'), 1 => bad.created_at += 1, 2 => bad.kind = 3,
                3 => bad.tags.push(vec!["p".into(), "x".into()]),
                4 => bad.pubkey = hex::encode(key().verifying_key().to_bytes()),
                _ => bad.sig.replace_range(..2, "ff"),
            }
            // A signature may already start with ff; change a guaranteed bit instead.
            if changed == 5 {
                let mut sig = bytes::<64>(&event.sig).unwrap(); sig[0] ^= 1; bad.sig = hex::encode(sig);
            }
            prop_assert!(bad.verify().is_err());
        }
    }

    #[test]
    fn nip19_round_trips(raw in any::<[u8;32]>(), author in any::<Option<[u8;32]>>(), kind in any::<Option<u32>>(), relays in prop::collection::vec("wss://[a-z]{1,15}\\.test", 0..5)) {
        for id in [Identifier::Public(raw), Identifier::Secret(raw), Identifier::Note(raw), Identifier::Profile { public: raw, relays: relays.clone() }, Identifier::Event { id: raw, relays, author, kind }] {
            prop_assert!(Identifier::decode(&id.encode().unwrap()).unwrap() == id);
        }
    }

    #[test]
    fn relay_ok_and_notice_preserve_arbitrary_strings(id in ".{0,64}", message in ".{0,128}", accepted in any::<bool>()) {
        prop_assert_eq!(RelayMessage::parse(&json!(["OK", id, accepted, message]).to_string()).unwrap(), RelayMessage::Ok(id, accepted, message.clone()));
        prop_assert_eq!(RelayMessage::parse(&json!(["NOTICE", message]).to_string()).unwrap(), RelayMessage::Notice(message));
    }

    #[test]
    fn arbitrary_inputs_never_panic(text in ".{0,512}") {
        let _identifier = Identifier::decode(&text);
        let _message = RelayMessage::parse(&text);
    }
}

#[test]
fn nip01_canonical_utf8_and_escape_digest() {
    // NIP-01's serialization rules applied to BIP-340's vector-0 public key; digest independently computed with Python hashlib.
    let public = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
    let event = Event {
        id: String::new(),
        pubkey: public.into(),
        created_at: 1231006505,
        kind: 1,
        tags: vec![vec!["p".into(), public.into()]],
        content: "日本語 / \" \\ \n\r\t\u{8}\u{c}".into(),
        sig: String::new(),
    };
    assert_eq!(
        hex::encode(event.digest().unwrap()),
        "298c45c2ac6cc720c02150c6b2d608dbf034bf4b15c659110c55cd64661bfe9c"
    );
}

#[test]
fn nip19_uppercase_and_optional_relay_parameters() {
    let id = Identifier::Public([1; 32]);
    assert!(Identifier::decode(&id.encode().unwrap().to_uppercase()).unwrap() == id);
    assert_eq!(
        RelayMessage::parse(r#"["OK","id",true,"","parameter"]"#).unwrap(),
        RelayMessage::Ok("id".into(), true, "".into())
    );
    assert_eq!(
        RelayMessage::parse(r#"["CLOSED","sub","restricted:","parameter"]"#).unwrap(),
        RelayMessage::Closed("sub".into(), "restricted:".into())
    );
}
