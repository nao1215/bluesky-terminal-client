//! An in-process relay bound exclusively to loopback, with an inspectable wire log.

use k256::schnorr::signature::hazmat::RandomizedPrehashSigner;
use k256::sha2::{Digest, Sha256};
use k256::{
    elliptic_curve::{Generate, common::getrandom::SysRng},
    schnorr::SigningKey,
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tungstenite::Message;

pub(crate) fn key() -> SigningKey {
    SigningKey::generate()
}

pub(crate) fn event(key: &SigningKey, kind: u16, time: u64, content: &str, tags: Value) -> Value {
    let public = hex::encode(key.verifying_key().to_bytes());
    let canonical = json!([0, public, time, kind, tags, content]).to_string();
    let digest: [u8; 32] = Sha256::digest(canonical.as_bytes()).into();
    json!({"id": hex::encode(digest), "pubkey": public, "created_at": time, "kind": kind, "tags": tags, "content": content, "sig": hex::encode(key.sign_prehash_with_rng(&mut SysRng, &digest).unwrap().to_bytes())})
}

#[derive(Clone, Default)]
pub(crate) struct Behavior {
    pub(crate) information: Option<Value>,
    pub(crate) auth: bool,
    pub(crate) no_eose: bool,
    pub(crate) reason: Option<String>,
    pub(crate) page_cap: Option<usize>,
    pub(crate) reject: bool,
    pub(crate) stall: bool,
    pub(crate) closed: bool,
    pub(crate) malformed: bool,
    pub(crate) wrong_filter: bool,
    pub(crate) wrong_ack: bool,
}

pub(crate) struct Relay {
    pub(crate) url: String,
    pub(crate) log: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Relay {
    pub(crate) fn start(events: Vec<Value>, behavior: Behavior) -> Self {
        Self::with_handler(events, behavior, |_message| None)
    }

    pub(crate) fn with_handler(
        events: Vec<Value>,
        behavior: Behavior,
        mut handler: impl FnMut(&Value) -> Option<Vec<Value>> + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (record, stopping) = (log.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let mut stream = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("local listener: {e}"),
                };
                // On Windows an accepted socket inherits the listener's nonblocking mode, and a large frame then fails to write; WinSock also leaves a socket unusable after a receive timeout. Each connection is read blocking until nstr closes it.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                // Peek without consuming the WebSocket upgrade; HTTP information uses the same port.
                let mut header = [0; 8192];
                let count = loop {
                    let count = stream.peek(&mut header).unwrap();
                    if count == 0 || header[..count].windows(4).any(|w| w == b"\r\n\r\n") {
                        break count;
                    }
                    thread::sleep(Duration::from_millis(1));
                };
                if String::from_utf8_lossy(&header[..count])
                    .to_ascii_lowercase()
                    .contains("accept: application/nostr+json")
                {
                    stream.read_exact(&mut header[..count]).unwrap();
                    let info = behavior.information.clone().unwrap_or_else(|| json!({}));
                    if behavior.information.is_some() {
                        record.lock().unwrap().push(json!(["NIP11"]));
                    }
                    let body = info.to_string();
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                    continue;
                }
                let Ok(mut socket) = tungstenite::accept(stream) else {
                    continue;
                };
                let mut authenticated = false;
                if behavior.auth
                    && socket
                        .send(Message::Text(
                            json!(["AUTH", "local challenge"]).to_string().into(),
                        ))
                        .is_err()
                {
                    continue;
                }
                while !stopping.load(Ordering::Relaxed) {
                    let message = match socket.read() {
                        Ok(Message::Text(t)) => serde_json::from_str::<Value>(&t).unwrap(),
                        Ok(_) => continue,
                        Err(_) => break,
                    };
                    record.lock().unwrap().push(message.clone());
                    if behavior.stall {
                        continue;
                    }
                    let mut send =
                        |v: Value| socket.send(Message::Text(v.to_string().into())).is_ok();
                    if let Some(replies) = handler(&message) {
                        if !replies.into_iter().all(&mut send) {
                            break;
                        }
                        continue;
                    }
                    if behavior.auth && !authenticated && message[0] == "EVENT" {
                        if !send(json!([
                            "OK",
                            message[1]["id"],
                            false,
                            "auth-required: test"
                        ])) {
                            break;
                        }
                        continue;
                    }
                    match message[0].as_str().unwrap() {
                        "AUTH" => {
                            authenticated = true;
                            if !send(json!(["OK", message[1]["id"], true, ""])) {
                                break;
                            }
                        }
                        "REQ" => {
                            let sub = &message[1];
                            if behavior.closed {
                                if !send(json!(["CLOSED", sub, "restricted: test"])) {
                                    break;
                                }
                                continue;
                            }
                            if behavior.malformed {
                                if !send(json!(["EOSE", 42])) {
                                    break;
                                }
                                continue;
                            }
                            if !send(json!(["NOTICE", "local test relay"])) {
                                break;
                            }
                            // An unrelated EOSE must never finish the requested subscription.
                            if !send(json!(["EOSE", "unrelated"])) {
                                break;
                            }
                            let filter = &message[2];
                            let mut selected: Vec<_> = events
                                .iter()
                                .filter(|e| {
                                    behavior.wrong_filter
                                        || (filter["kinds"]
                                            .as_array()
                                            .unwrap()
                                            .contains(&e["kind"])
                                            && filter.get("authors").is_none_or(|a| {
                                                a.as_array().unwrap().contains(&e["pubkey"])
                                            })
                                            && filter.get("#t").is_none_or(|tags| {
                                                e["tags"].as_array().unwrap().iter().any(|t| {
                                                    t[0] == "t"
                                                        && tags.as_array().unwrap().contains(&t[1])
                                                })
                                            })
                                            && filter.get("search").is_none_or(|query| {
                                                e["content"]
                                                    .as_str()
                                                    .unwrap()
                                                    .contains(query.as_str().unwrap())
                                            })
                                            && filter.get("until").is_none_or(|t| {
                                                e["created_at"].as_u64().unwrap()
                                                    <= t.as_u64().unwrap()
                                            })
                                            && filter.get("#p").is_none_or(|p| {
                                                e["tags"].as_array().unwrap().iter().any(|t| {
                                                    t[0] == "p"
                                                        && p.as_array().unwrap().contains(&t[1])
                                                })
                                            }))
                                })
                                .cloned()
                                .collect();
                            selected.sort_by(|a, b| {
                                b["created_at"]
                                    .as_u64()
                                    .cmp(&a["created_at"].as_u64())
                                    .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
                            });
                            let limit = filter["limit"].as_u64().unwrap() as usize;
                            selected.truncate(behavior.page_cap.unwrap_or(limit).min(limit));
                            for event in selected {
                                if !send(json!(["EVENT", sub, event])) {
                                    break;
                                }
                            }
                            if !behavior.no_eose && !send(json!(["EOSE", sub])) {
                                break;
                            }
                        }
                        "EVENT" => {
                            let id = if behavior.wrong_ack {
                                json!("unrelated")
                            } else {
                                message[1]["id"].clone()
                            };
                            if !send(json!([
                                "OK",
                                id,
                                !behavior.reject && behavior.reason.is_none(),
                                behavior.reason.as_deref().unwrap_or(if behavior.reject {
                                    "blocked: test"
                                } else {
                                    ""
                                })
                            ])) {
                                break;
                            }
                        }
                        "CLOSE" => (),
                        other => panic!("unexpected client message {other}"),
                    }
                }
            }
        });
        Self {
            url,
            log,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
