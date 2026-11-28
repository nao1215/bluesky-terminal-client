//! An in-process relay bound exclusively to loopback, with an inspectable wire log.

use k256::schnorr::signature::hazmat::RandomizedPrehashSigner;
use k256::sha2::{Digest, Sha256};
use k256::{
    elliptic_curve::{Generate, common::getrandom::SysRng},
    schnorr::SigningKey,
};
use serde_json::{Value, json};
use std::{
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
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (record, stopping) = (log.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let stream = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("local listener: {e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let Ok(mut socket) = tungstenite::accept(stream) else {
                    continue;
                };
                while !stopping.load(Ordering::Relaxed) {
                    let message = match socket.read() {
                        Ok(Message::Text(t)) => serde_json::from_str::<Value>(&t).unwrap(),
                        Ok(_) => continue,
                        Err(tungstenite::Error::Io(e))
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            continue;
                        }
                        Err(_) => break,
                    };
                    record.lock().unwrap().push(message.clone());
                    if behavior.stall {
                        continue;
                    }
                    let mut send =
                        |v: Value| socket.send(Message::Text(v.to_string().into())).is_ok();
                    match message[0].as_str().unwrap() {
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
                            if !send(json!(["EOSE", sub])) {
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
                                !behavior.reject,
                                if behavior.reject { "blocked: test" } else { "" }
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
