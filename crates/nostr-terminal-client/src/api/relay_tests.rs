//! Real WebSocket frames over in-memory duplex streams exercise sessions without sockets.

use super::*;
use k256::{elliptic_curve::Generate, schnorr::SigningKey};
use std::collections::VecDeque;
use tungstenite::protocol::Role;

pub(crate) struct MemoryStream {
    input: mpsc::Receiver<Vec<u8>>,
    output: mpsc::Sender<Vec<u8>>,
    buffered: VecDeque<u8>,
}

impl Read for MemoryStream {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.buffered.is_empty() {
            match self.input.recv_timeout(Duration::from_millis(500)) {
                Ok(bytes) => self.buffered.extend(bytes),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(0),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(io::Error::from(io::ErrorKind::TimedOut));
                }
            }
        }
        let count = out.len().min(self.buffered.len());
        for byte in out.iter_mut().take(count) {
            *byte = self.buffered.pop_front().unwrap();
        }
        Ok(count)
    }
}

impl Write for MemoryStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output
            .send(bytes.to_vec())
            .map_err(|_invalid| io::Error::from(io::ErrorKind::BrokenPipe))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn pair() -> (Session<MemoryStream>, WebSocket<MemoryStream>) {
    let (send_a, recv_a) = mpsc::channel();
    let (send_b, recv_b) = mpsc::channel();
    let client = MemoryStream {
        input: recv_a,
        output: send_b,
        buffered: VecDeque::new(),
    };
    let server = MemoryStream {
        input: recv_b,
        output: send_a,
        buffered: VecDeque::new(),
    };
    (
        Session {
            socket: WebSocket::from_raw_socket(client, Role::Client, None),
            deadline: Instant::now() + Duration::from_secs(5),
            notices: vec![],
            limits: Limits::default(),
            incomplete: false,
        },
        WebSocket::from_raw_socket(server, Role::Server, None),
    )
}

fn send(socket: &mut WebSocket<MemoryStream>, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .unwrap();
}
fn receive(socket: &mut WebSocket<MemoryStream>) -> Value {
    serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap()
}
fn note(time: u64, text: &str) -> Event {
    let key = SigningKey::generate();
    Event::sign(&key, time, 1, vec![], text.into()).unwrap()
}

#[test]
fn session_handles_frames_ping_notice_dedup_and_close() {
    let event = note(1, "test");
    let expected = event.clone();
    let (mut client, mut relay) = pair();
    let server = thread::spawn(move || {
        let req = receive(&mut relay);
        assert_eq!(req[0], "REQ");
        relay.send(Message::Ping(vec![1, 2].into())).unwrap();
        send(&mut relay, json!(["NOTICE", "hello"]));
        send(&mut relay, json!(["EOSE", "other"]));
        send(&mut relay, json!(["EVENT", "sub", event]));
        send(&mut relay, json!(["EVENT", "sub", event]));
        send(&mut relay, json!(["EOSE", "sub"]));
        assert!(matches!(relay.read().unwrap(), Message::Pong(_)));
        assert_eq!(receive(&mut relay), json!(["CLOSE", "sub"]));
    });
    let events = client
        .page(
            &Filter {
                kinds: vec![1],
                limit: 1,
                ..Filter::default()
            },
            "sub",
        )
        .unwrap();
    assert_eq!(events, [expected]);
    assert_eq!(client.notices, ["hello"]);
    server.join().unwrap();
}

#[test]
fn session_reads_tied_timestamps_and_short_pages() {
    for cap in [2, 100] {
        let events: Vec<_> = (0..230)
            .map(|i| note(if cap == 2 { i } else { i / 3 }, &i.to_string()))
            .collect();
        let mut expected = events.clone();
        expected.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        expected.truncate(220);
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            while let Ok(message) = relay.read() {
                let request: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                if request[0] == "CLOSE" {
                    continue;
                }
                let filter: Filter = serde_json::from_value(request[2].clone()).unwrap();
                let mut selected: Vec<_> = events.iter().filter(|e| filter.matches(e)).collect();
                selected.sort_by(|a, b| {
                    b.created_at
                        .cmp(&a.created_at)
                        .then_with(|| a.id.cmp(&b.id))
                });
                // A realistic page cap applies independently of the request size.
                selected.truncate(filter.limit.min(cap));
                for e in selected {
                    send(&mut relay, json!(["EVENT", request[1], e]));
                }
                send(&mut relay, json!(["EOSE", request[1]]));
            }
        });
        let mut actual = client
            .read(
                &Filter {
                    kinds: vec![1],
                    ..Filter::default()
                },
                220,
            )
            .unwrap();
        if cap == 2 {
            assert!(client.notices.iter().any(|n| n.starts_with("relay capped")));
        }
        actual.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        actual.truncate(220);
        assert_eq!(actual, expected);
        drop(client);
        server.join().unwrap();
    }
}

#[test]
fn session_rejects_tampering_wrong_filters_and_closed() {
    for case in 0..3 {
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            let req = receive(&mut relay);
            let mut event = note(1, "valid");
            match case {
                0 => {
                    event.content = "tampered".into();
                    send(&mut relay, json!(["EVENT", req[1], event]));
                }
                1 => send(&mut relay, json!(["EVENT", req[1], event])),
                _ => send(&mut relay, json!(["CLOSED", req[1], "restricted: test"])),
            }
        });
        let error = client
            .page(
                &Filter {
                    kinds: vec![0],
                    limit: 1,
                    ..Filter::default()
                },
                "sub",
            )
            .unwrap_err();
        assert_eq!(error.status, 4);
        server.join().unwrap();
    }
}

#[test]
fn session_matches_publication_ack_and_reports_rejection() {
    for accepted in [true, false] {
        let (mut client, mut relay) = pair();
        let event = note(1, "publish");
        let expected = event.clone();
        let server = thread::spawn(move || {
            let sent = receive(&mut relay);
            assert_eq!(sent, json!(["EVENT", expected]));
            send(&mut relay, json!(["OK", "other", true, ""]));
            send(
                &mut relay,
                json!(["OK", expected.id, accepted, "blocked: test"]),
            );
        });
        assert_eq!(
            client.publish(&event, "ws://127.0.0.1/", None).is_ok(),
            accepted
        );
        server.join().unwrap();
    }
}

#[test]
fn deadline_is_checked_even_for_buffered_frames() {
    let (mut client, _relay) = pair();
    client.deadline = Instant::now() - Duration::from_millis(1);
    assert!(client.receive().is_err());
    assert!(client.send(json!(["REQ", "sub", {}])).is_err());
}

#[test]
fn thread_filters_enforce_ids_and_referenced_events_before_accepting_notes() {
    let target = note(1, "parent");
    let reply = Event::sign(
        &SigningKey::generate(),
        2,
        1,
        vec![vec![
            "e".into(),
            target.id.clone(),
            String::new(),
            "reply".into(),
        ]],
        "reply".into(),
    )
    .unwrap();
    let by_id = Filter {
        kinds: vec![1],
        ids: vec![target.id.clone()],
        ..Filter::default()
    };
    let by_reference = Filter {
        kinds: vec![1],
        references: vec![target.id.clone()],
        ..Filter::default()
    };
    assert!(by_id.matches(&target));
    assert!(!by_id.matches(&reply));
    assert!(by_reference.matches(&reply));
    assert!(!by_reference.matches(&target));
    assert_eq!(
        serde_json::to_value(&by_reference).unwrap()["#e"],
        json!([target.id])
    );
}

#[test]
fn publication_authenticates_and_retries_only_after_both_acknowledgements() {
    for rejection_first in [true, false] {
        let key = SigningKey::generate();
        let event = Event::sign(&key, 1, 1, vec![], "note".into()).unwrap();
        let expected = event.clone();
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            assert_eq!(receive(&mut relay), json!(["EVENT", expected]));
            if rejection_first {
                send(
                    &mut relay,
                    json!(["OK", expected.id, false, "auth-required: login"]),
                );
            }
            send(&mut relay, json!(["AUTH", "challenge"]));
            let message = receive(&mut relay);
            assert_eq!(message[0], "AUTH");
            let auth: Event = serde_json::from_value(message[1].clone()).unwrap();
            auth.verify().unwrap();
            assert_eq!(auth.kind, 22242);
            assert_eq!(auth.pubkey, expected.pubkey);
            assert_eq!(auth.content, "");
            assert_eq!(
                auth.tags,
                vec![
                    vec!["relay", "ws://127.0.0.1/"],
                    vec!["challenge", "challenge"]
                ]
            );
            assert!(
                auth.created_at
                    .abs_diff(jiff::Timestamp::now().as_second() as u64)
                    < 10
            );
            send(&mut relay, json!(["OK", auth.id, true, ""]));
            if !rejection_first {
                send(
                    &mut relay,
                    json!(["OK", expected.id, false, "auth-required: login"]),
                );
            }
            assert_eq!(receive(&mut relay), json!(["EVENT", expected]));
            send(&mut relay, json!(["OK", expected.id, true, ""]));
        });
        client
            .publish(&event, "ws://127.0.0.1/", Some(&key))
            .unwrap();
        server.join().unwrap();
    }
}

#[test]
fn machine_readable_rejections_preserve_reason_and_duplicate_is_success() {
    for prefix in [
        "blocked",
        "rate-limited",
        "invalid",
        "pow",
        "duplicate",
        "restricted",
        "auth-required",
    ] {
        let event = note(1, "post");
        let id = event.id.clone();
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            receive(&mut relay);
            send(
                &mut relay,
                json!(["OK", id, false, format!("{prefix}: test")]),
            );
        });
        let result = client.publish(&event, "ws://127.0.0.1/", None);
        if prefix == "duplicate" {
            assert!(result.is_ok());
        } else {
            assert!(
                result
                    .unwrap_err()
                    .error
                    .contains(&format!("{prefix}: test"))
            );
        }
        server.join().unwrap();
    }
}

#[test]
fn auth_challenge_on_read_or_without_local_key_reports_requirement() {
    for reading in [true, false] {
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            receive(&mut relay);
            send(&mut relay, json!(["AUTH", "challenge"]));
        });
        let error = if reading {
            client.read(&Filter::default(), 1).unwrap_err()
        } else {
            client
                .publish(&note(1, "post"), "ws://127.0.0.1/", None)
                .unwrap_err()
        };
        assert!(error.error.contains("relay requires auth"));
        server.join().unwrap();
    }
}

#[test]
fn limitations_are_checked_before_sending() {
    for limits in [
        Limits {
            max_subscriptions: Some(0),
            ..Limits::default()
        },
        Limits {
            max_limit: Some(0),
            ..Limits::default()
        },
        Limits {
            auth_required: true,
            ..Limits::default()
        },
        Limits {
            payment_required: true,
            ..Limits::default()
        },
    ] {
        let (mut client, _relay) = pair();
        client.limits = limits;
        assert!(client.read(&Filter::default(), 1).is_err());
    }
    let (mut client, _relay) = pair();
    let message = json!(["EVENT", "世界"]);
    client.limits.max_message_length = Some(message.to_string().len() - 1);
    assert!(
        client
            .send(message.clone())
            .unwrap_err()
            .error
            .contains("max_message_length")
    );
    client.limits.max_message_length = Some(message.to_string().len());
    client.send(message).unwrap();
}

#[test]
fn missing_eose_retains_verified_events_but_notices_incompleteness() {
    for with_event in [true, false] {
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            let req = receive(&mut relay);
            if with_event {
                send(&mut relay, json!(["EVENT", req[1], note(1, "partial")]));
            }
            // Hold the transport open without EOSE, independently of socket permissions.
            thread::sleep(Duration::from_millis(650));
        });
        let result = client.read(
            &Filter {
                kinds: vec![1],
                ..Filter::default()
            },
            2,
        );
        if with_event {
            assert_eq!(result.unwrap().len(), 1);
            assert!(
                client
                    .notices
                    .iter()
                    .any(|n| n.contains("EOSE unavailable"))
            );
        } else {
            assert!(result.unwrap_err().error.contains("EOSE unavailable"));
        }
        server.join().unwrap();
    }
}

#[test]
fn advertised_cap_keeps_one_subscription_and_reports_crowded_boundary() {
    let events: Vec<_> = (0..3).map(|i| note(5, &i.to_string())).collect();
    let (mut client, mut relay) = pair();
    client.limits.max_limit = Some(2);
    client.limits.max_subscriptions = Some(1);
    let server = thread::spawn(move || {
        while let Ok(message) = relay.read() {
            let req: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
            assert_eq!(req[0], "REQ");
            assert!(req[2]["limit"].as_u64().unwrap() <= 2);
            if req[2]["until"].as_u64().is_none_or(|t| t >= 5) {
                for e in events.iter().take(2) {
                    send(&mut relay, json!(["EVENT", req[1], e]));
                }
            }
            send(&mut relay, json!(["EOSE", req[1]]));
            assert_eq!(receive(&mut relay), json!(["CLOSE", req[1]]));
        }
    });
    assert_eq!(
        client
            .read(
                &Filter {
                    kinds: vec![1],
                    ..Filter::default()
                },
                3
            )
            .unwrap()
            .len(),
        2
    );
    assert!(
        client
            .notices
            .iter()
            .any(|n| n.contains("timestamp boundary"))
    );
    drop(client);
    server.join().unwrap();
}

#[test]
fn authentication_rejection_and_repeated_challenge_do_not_retry_publication() {
    for repeat in [false, true] {
        let key = SigningKey::generate();
        let event = note(1, "post");
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            receive(&mut relay);
            send(&mut relay, json!(["AUTH", "first"]));
            let auth = receive(&mut relay);
            if repeat {
                send(&mut relay, json!(["AUTH", "second"]));
            } else {
                send(
                    &mut relay,
                    json!(["OK", auth[1]["id"], false, "restricted: denied"]),
                );
            }
        });
        let error = client
            .publish(&event, "ws://127.0.0.1/", Some(&key))
            .unwrap_err();
        assert!(error.error.contains(if repeat {
            "repeated AUTH"
        } else {
            "restricted: denied"
        }));
        server.join().unwrap();
    }
}

#[test]
fn unsupported_auth_stays_status_five_after_publication_is_rejected() {
    for notice in [false, true] {
        let key = SigningKey::generate();
        let event = note(1, "post");
        let (mut client, mut relay) = pair();
        let server = thread::spawn(move || {
            let publication = receive(&mut relay);
            send(
                &mut relay,
                json!(["OK", publication[1]["id"], false, "auth-required: login"]),
            );
            send(&mut relay, json!(["AUTH", "first"]));
            let auth = receive(&mut relay);
            let response = if notice {
                json!(["NOTICE", "unsupported: AUTH"])
            } else {
                json!(["OK", auth[1]["id"], false, "unsupported: AUTH"])
            };
            send(&mut relay, response);
        });
        let error = client
            .publish(&event, "ws://127.0.0.1/", Some(&key))
            .unwrap_err();
        assert_eq!(error.status, 5);
        assert!(error.error.contains("unsupported: AUTH"));
        server.join().unwrap();
    }
}

#[test]
fn publish_respects_payment_auth_and_message_limits_before_sending() {
    let event = note(1, "bounded");
    for (limits, reason) in [
        (
            Limits {
                payment_required: true,
                ..Limits::default()
            },
            "payment-required",
        ),
        (
            Limits {
                auth_required: true,
                ..Limits::default()
            },
            "auth-required:",
        ),
        (
            Limits {
                max_message_length: Some(10),
                ..Limits::default()
            },
            "max_message_length",
        ),
    ] {
        let (mut client, _relay) = pair();
        client.limits = limits;
        assert!(
            client
                .publish(&event, "ws://127.0.0.1/", None)
                .unwrap_err()
                .error
                .contains(reason)
        );
    }
}

#[test]
fn timeout_on_a_later_page_does_not_discard_completed_pages() {
    let (mut client, mut relay) = pair();
    let server = thread::spawn(move || {
        let req = receive(&mut relay);
        send(&mut relay, json!(["EVENT", req[1], note(1, "retained")]));
        send(&mut relay, json!(["EOSE", req[1]]));
        assert_eq!(receive(&mut relay)[0], "CLOSE");
        assert_eq!(receive(&mut relay)[0], "REQ");
        thread::sleep(Duration::from_millis(650));
    });
    let result = client
        .read(
            &Filter {
                kinds: vec![1],
                ..Filter::default()
            },
            2,
        )
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "retained");
    assert!(
        client
            .notices
            .iter()
            .any(|n| n.contains("EOSE unavailable"))
    );
    server.join().unwrap();
}

#[test]
fn unadvertised_search_is_unsupported_before_a_request_is_sent() {
    let (mut client, _relay) = pair();
    let error = client
        .read(
            &Filter {
                kinds: vec![1],
                search: Some("word".into()),
                ..Filter::default()
            },
            1,
        )
        .unwrap_err();
    assert_eq!(error.status, 5);
    assert!(error.error.contains("NIP-50 search"));
}

#[test]
fn unsupported_notice_preserves_feature_reason() {
    let (mut client, mut relay) = pair();
    send(&mut relay, json!(["NOTICE", "unsupported: search"]));
    let error = client.receive().unwrap_err();
    assert_eq!(error.status, 5);
    assert!(error.error.contains("search"));
}
