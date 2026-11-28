//! Real WebSocket frames over in-memory duplex streams exercise sessions without sockets.

use super::*;
use k256::{elliptic_curve::Generate, schnorr::SigningKey};
use std::collections::VecDeque;
use tungstenite::protocol::Role;

struct MemoryStream {
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

fn pair() -> (Session<MemoryStream>, WebSocket<MemoryStream>) {
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
        assert_eq!(client.publish(&event).is_ok(), accepted);
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
