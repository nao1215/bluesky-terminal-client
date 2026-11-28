//! URI validation and authenticated response correlation, without sockets.
use super::*;
use proptest::prelude::*;

fn signer() -> (SigningKey, String) {
    let key = SigningKey::generate();
    let public = hex::encode(key.verifying_key().to_bytes());
    (key, public)
}

#[test]
fn bunker_uri_validates_authority_relays_and_secret() {
    let (_, public) = signer();
    let uri = format!(
        "bunker://{public}?relay=wss%3A%2F%2Fexample.invalid&relay=ws%3A%2F%2F127.0.0.1%3A1234&secret=a%2Bb%26c"
    );
    let parsed = Bunker::parse(&uri).unwrap();
    assert_eq!(parsed.public, public);
    assert_eq!(
        parsed.relays,
        ["wss://example.invalid/", "ws://127.0.0.1:1234/"]
    );
    assert_eq!(parsed.secret.unwrap().as_str(), "a+b&c");
    assert!(
        Bunker::parse(&format!("bunker://{public}?relay=ws://localhost"))
            .unwrap()
            .secret
            .is_none()
    );
    for uri in [
        format!("nostrconnect://{public}?relay=ws://localhost"),
        format!("bunker://{public}"),
        format!("bunker://{public}:123?relay=ws://localhost"),
        format!("bunker://user@{public}?relay=ws://localhost"),
        format!("bunker://{public}/path?relay=ws://localhost"),
        format!("bunker://{public}?relay=ws://localhost#fragment"),
        format!("bunker://{public}?relay=https://localhost"),
        format!("bunker://{public}?relay=ws://u:p@localhost"),
        format!("bunker://{public}?relay=ws://localhost&secret=a&secret=b"),
        format!("bunker://{public}?relay=ws://localhost&secret="),
        format!("bunker://{public}?relay=ws://localhost&unexpected=a"),
        format!("bunker://{}?relay=ws://localhost", "00".repeat(32)),
        format!(
            "bunker://{public}?{}",
            vec!["relay=ws://localhost"; 17].join("&")
        ),
        format!("\nbunker://{public}?relay=ws://localhost"),
    ] {
        assert!(Bunker::parse(&uri).is_err(), "{uri}");
    }
    let bad = "bunker://bad?relay=ws://localhost&secret=do-not-disclose";
    assert!(
        !Bunker::parse(bad)
            .err()
            .unwrap()
            .error
            .contains("do-not-disclose")
    );
}

fn envelope(signer: &SigningKey, client: &str, payload: serde_json::Value) -> Event {
    Event::sign(
        signer,
        1,
        24133,
        vec![vec!["p".into(), client.into()]],
        nip44::encrypt(&[1; 32], &[2; 32], &payload.to_string()).unwrap(),
    )
    .unwrap()
}

#[test]
fn response_checks_envelope_signature_auth_and_errors() {
    let (key, public) = signer();
    let (_, client) = signer();
    for (payload, expected) in [
        (
            json!({"id":"request", "result":"ack"}),
            Reply::Value("ack".into()),
        ),
        (
            json!({"id":"request", "error":"secret text"}),
            Reply::Rejected,
        ),
        (
            json!({"id":"request", "result":"auth_url", "error":"https://example.invalid/auth"}),
            Reply::Auth("https://example.invalid/auth".into()),
        ),
    ] {
        let event = envelope(&key, &client, payload);
        assert_eq!(
            response(&event, &public, &client, &[1; 32], "request").unwrap(),
            Some(expected)
        );
        assert_eq!(
            response(&event, &public, &client, &[1; 32], "other").unwrap(),
            None
        );
        assert_eq!(
            response(&event, "other", &client, &[1; 32], "request").unwrap(),
            None
        );
        assert_eq!(
            response(&event, &public, "other", &[1; 32], "request").unwrap(),
            None
        );
        let mut changed = event.clone();
        changed.content.push('A');
        assert!(response(&changed, &public, &client, &[1; 32], "request").is_err());
        changed = event;
        changed.kind = 1;
        assert_eq!(
            response(&changed, &public, &client, &[1; 32], "request").unwrap(),
            None
        );
    }
    for payload in [
        json!({"id":"request"}),
        json!({"id":5, "result":"ack"}),
        json!({"id":"request", "result":"auth_url"}),
        json!({"id":"request", "result":"auth_url", "error":"javascript:alert(1)"}),
        json!({"id":"request", "result":"auth_url", "error":"https://example.invalid/\u{1b}[31m"}),
    ] {
        assert!(
            response(
                &envelope(&key, &client, payload),
                &public,
                &client,
                &[1; 32],
                "request"
            )
            .is_err()
        );
    }
}

proptest! {
    #[test]
    fn responses_match_only_the_requested_id(id in "[a-z0-9]{1,64}", result in ".{0,128}") {
        prop_assume!(result != "auth_url");
        let (key, public) = signer();
        let (_, client) = signer();
        let event = envelope(&key, &client, json!({"id":id,"result":result}));
        prop_assert_eq!(response(&event, &public, &client, &[1;32], &id).unwrap(), Some(Reply::Value(result)));
        prop_assert_eq!(response(&event, &public, &client, &[1;32], &format!("{id}-stale")).unwrap(), None);
    }
    #[test]
    fn bunker_query_roundtrips(secret in "[a-zA-Z0-9+ &%=/]{1,64}", ports in prop::collection::vec(1u16..65535, 1..16)) {
        let (_, public) = signer();
        let mut uri = url::Url::parse(&format!("bunker://{public}")).unwrap();
        for port in &ports { uri.query_pairs_mut().append_pair("relay", &format!("ws://127.0.0.1:{port}")); }
        uri.query_pairs_mut().append_pair("secret", &secret);
        let parsed = Bunker::parse(uri.as_str()).unwrap();
        let parsed_secret = parsed.secret.unwrap();
        prop_assert_eq!(parsed_secret.as_str(), secret.as_str());
        prop_assert_eq!(parsed.relays.len(), ports.iter().collect::<std::collections::HashSet<_>>().len());
    }
    #[test]
    fn arbitrary_bunker_inputs_do_not_panic(uri in ".{0,512}") { let _result = Bunker::parse(&uri); }
}

#[test]
fn memory_session_preserves_subscription_across_auth_and_multiple_requests() {
    use super::super::relay::tests::pair;
    use tungstenite::Message;
    let (key, public) = signer();
    let (client_key, client_public) = signer();
    let (mut session, mut relay) = pair();
    let (send, requests) = mpsc::channel();
    let (responses, incoming) = mpsc::sync_channel(32);
    let connection = Connection {
        signer: &public,
        client: &client_public,
        conversation: &[1; 32],
        deadline: Instant::now() + Duration::from_secs(5),
    };
    thread::scope(|scope| {
        let worker = scope.spawn(move || connection.serve(&mut session, requests, &responses));
        let req: serde_json::Value =
            serde_json::from_str(relay.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(req[0], "REQ");
        for method in ["connect", "get_public_key", "sign_event"] {
            let request = Event::sign(
                &client_key,
                1,
                24133,
                vec![vec!["p".into(), public.clone()]],
                nip44::encrypt(
                    &[1; 32],
                    &random_bytes().unwrap(),
                    &json!({"id":method,"method":method,"params":[]}).to_string(),
                )
                .unwrap(),
            )
            .unwrap();
            send.send((method.into(), request.clone())).unwrap();
            let sent: serde_json::Value =
                serde_json::from_str(relay.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(sent, json!(["EVENT", request]));
            relay
                .send(Message::Text(
                    json!(["EOSE", "nstr-bunker"]).to_string().into(),
                ))
                .unwrap();
            // Wrong subscriptions, IDs and replayed responses cannot finish this request.
            for (sub, id, result, error) in [
                ("other", method, "ignored", None),
                ("nstr-bunker", "stale", "ignored", None),
                (
                    "nstr-bunker",
                    method,
                    "auth_url",
                    Some("https://example.invalid/auth"),
                ),
                ("nstr-bunker", method, "ack", None),
            ] {
                let event = envelope(
                    &key,
                    &client_public,
                    json!({"id":id,"result":result,"error":error}),
                );
                relay
                    .send(Message::Text(
                        json!(["EVENT", sub, event]).to_string().into(),
                    ))
                    .unwrap();
            }
            assert_eq!(
                incoming.recv_timeout(Duration::from_secs(1)).unwrap(),
                (
                    method.into(),
                    Reply::Auth("https://example.invalid/auth".into())
                )
            );
            assert_eq!(
                incoming.recv_timeout(Duration::from_secs(1)).unwrap(),
                (method.into(), Reply::Value("ack".into()))
            );
        }
        drop(send);
        let close: serde_json::Value =
            serde_json::from_str(relay.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(close, json!(["CLOSE", "nstr-bunker"]));
        worker.join().unwrap().unwrap();
    });
}

#[test]
fn signed_note_must_match_every_requested_field() {
    let (key, public) = signer();
    let original = Event::sign(&key, 1, 1, vec![], "original".into()).unwrap();
    assert_eq!(
        validate_note(
            &serde_json::to_string(&original).unwrap(),
            &public,
            1,
            "original"
        )
        .unwrap(),
        original
    );
    // These are valid signatures over changed data; verification alone cannot suffice.
    for (changed_key, time, kind, tags, text) in [
        (SigningKey::generate(), 1, 1, vec![], "original"),
        (key.clone(), 2, 1, vec![], "original"),
        (key.clone(), 1, 2, vec![], "original"),
        (
            key.clone(),
            1,
            1,
            vec![vec!["p".into(), public.clone()]],
            "original",
        ),
        (key.clone(), 1, 1, vec![], "changed"),
    ] {
        let changed = Event::sign(&changed_key, time, kind, tags, text.into()).unwrap();
        assert!(
            validate_note(
                &serde_json::to_string(&changed).unwrap(),
                &public,
                1,
                "original"
            )
            .unwrap_err()
            .error
            .contains("changed")
        );
    }
    let mut broken = original;
    broken.sig = "00".repeat(64);
    assert!(
        validate_note(
            &serde_json::to_string(&broken).unwrap(),
            &public,
            1,
            "original"
        )
        .is_err()
    );
    assert!(validate_note("{}", &public, 1, "original").is_err());
}
