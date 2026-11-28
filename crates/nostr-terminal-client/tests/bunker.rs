//! CLI integration tests: generated keys, local relay and remote signer only.
#[cfg(test)]
#[path = "../src/error.rs"]
mod error;
#[cfg(test)]
#[path = "../src/api/nip44.rs"]
mod nip44;
#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use super::{error, nip44, support};

    use k256::{
        schnorr::{Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashVerifier},
        sha2::{Digest, Sha256},
    };
    use serde_json::{Value, json};
    use std::{
        process::{Command, Output},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };
    use support::{Behavior, Relay, event, key};

    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Success,
        WrongPublic,
        Rejected,
        Auth,
        Timeout,
        WrongIds,
        ChangedNote,
        BadSignature,
    }

    struct Signer {
        transport: SigningKey,
        owner: SigningKey,
        mode: Mode,
        requests: Mutex<Vec<Value>>,
        clients: Mutex<Vec<String>>,
    }

    fn verify(event: &Value) {
        let digest: [u8; 32] = Sha256::digest(
            json!([
                0,
                event["pubkey"],
                event["created_at"],
                event["kind"],
                event["tags"],
                event["content"]
            ])
            .to_string()
            .as_bytes(),
        )
        .into();
        assert_eq!(hex::encode(digest), event["id"]);
        let public =
            VerifyingKey::from_slice(&hex::decode(event["pubkey"].as_str().unwrap()).unwrap())
                .unwrap();
        let signature = Signature::try_from(
            hex::decode(event["sig"].as_str().unwrap())
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        public.verify_prehash(&digest, &signature).unwrap();
    }

    impl Signer {
        fn answer(&self, wire: &Value) -> Option<Vec<Value>> {
            if wire[0] == "REQ" && wire[2]["kinds"] == json!([24133]) {
                assert_eq!(
                    wire[2]["authors"],
                    json!([hex::encode(self.transport.verifying_key().to_bytes())])
                );
                return Some(vec![json!(["EOSE", wire[1]])]);
            }
            if wire[0] != "EVENT" || wire[1]["kind"] != 24133 {
                return None;
            }
            let envelope = &wire[1];
            verify(envelope);
            assert_eq!(
                envelope["tags"],
                json!([["p", hex::encode(self.transport.verifying_key().to_bytes())]])
            );
            let client = envelope["pubkey"].as_str().unwrap();
            let conversation = nip44::conversation_key(
                &self.transport,
                &hex::decode(client).unwrap().try_into().unwrap(),
            )
            .unwrap();
            let plaintext =
                nip44::decrypt(&conversation, envelope["content"].as_str().unwrap()).unwrap();
            let request: Value = serde_json::from_str(&plaintext).unwrap();
            self.requests.lock().unwrap().push(request.clone());
            if self.mode == Mode::Timeout {
                return Some(vec![]);
            }
            let method = request["method"].as_str().unwrap();
            let result = match method {
                "connect" => {
                    assert_eq!(
                        request["params"][0],
                        hex::encode(self.transport.verifying_key().to_bytes())
                    );
                    assert_eq!(request["params"][1], "test-secret+&");
                    assert!(
                        request["params"][2]
                            .as_str()
                            .unwrap()
                            .starts_with("sign_event:")
                            || ["nip44_encrypt", "nip44_decrypt"]
                                .contains(&request["params"][2].as_str().unwrap())
                    );
                    self.clients.lock().unwrap().push(client.to_owned());
                    "ack".into()
                }
                "get_public_key" => hex::encode(if self.mode == Mode::WrongPublic {
                    self.transport.verifying_key().to_bytes()
                } else {
                    self.owner.verifying_key().to_bytes()
                }),
                "sign_event" => {
                    let note: Value =
                        serde_json::from_str(request["params"][0].as_str().unwrap()).unwrap();
                    let text = if self.mode == Mode::ChangedNote {
                        "changed by signer"
                    } else {
                        note["content"].as_str().unwrap()
                    };
                    let mut signed = event(
                        &self.owner,
                        note["kind"].as_u64().unwrap() as u16,
                        note["created_at"].as_u64().unwrap(),
                        text,
                        note["tags"].clone(),
                    );
                    if self.mode == Mode::BadSignature {
                        signed["sig"] = json!("00".repeat(64));
                    }
                    signed.to_string()
                }
                "nip44_encrypt" | "nip44_decrypt" => {
                    let peer: [u8; 32] = hex::decode(request["params"][0].as_str().unwrap())
                        .unwrap()
                        .try_into()
                        .unwrap();
                    let conversation = nip44::conversation_key(&self.owner, &peer).unwrap();
                    let text = request["params"][1].as_str().unwrap();
                    if method == "nip44_encrypt" {
                        nip44::encrypt(&conversation, &key().to_bytes().into(), text).unwrap()
                    } else {
                        nip44::decrypt(&conversation, text).unwrap().to_string()
                    }
                }
                other => panic!("unexpected method: {other}"),
            };
            let wrap = |payload: Value| {
                let encrypted = nip44::encrypt(
                    &conversation,
                    &key().to_bytes().into(),
                    &payload.to_string(),
                )
                .unwrap();
                json!([
                    "EVENT",
                    "nstr-bunker",
                    event(
                        &self.transport,
                        24133,
                        1,
                        &encrypted,
                        json!([["p", client]])
                    )
                ])
            };
            // A valid signed response for another ID must never complete an RPC.
            let mut replies = vec![wrap(
                json!({"id":"unrelated", "error":"do not accept this rejection"}),
            )];
            if self.mode == Mode::WrongIds {
                return Some(replies);
            }
            if self.mode == Mode::Auth && method == "connect" {
                replies.push(wrap(json!({"id":request["id"], "result":"auth_url", "error":"https://signer.invalid/authorize"})));
            }
            let response = if self.mode == Mode::Rejected && method == "sign_event" {
                json!({"id":request["id"], "error":"rejected test-secret+&"})
            } else {
                json!({"id":request["id"], "result":result})
            };
            replies.push(wrap(response));
            // Duplicate delivery on a relay cannot complete the following RPC.
            replies.push(replies.last().unwrap().clone());
            Some(replies)
        }
    }

    struct Fixture {
        dir: tempfile::TempDir,
        signer: Arc<Signer>,
        remote: Relay,
        publication: Relay,
    }

    impl Fixture {
        fn new(mode: Mode) -> Self {
            let signer = Arc::new(Signer {
                transport: key(),
                owner: key(),
                mode,
                requests: Mutex::new(vec![]),
                clients: Mutex::new(vec![]),
            });
            let handler = signer.clone();
            let remote = Relay::with_handler(vec![], Behavior::default(), move |message| {
                handler.answer(message)
            });
            let publication = Relay::start(vec![], Behavior::default());
            let dir = tempfile::tempdir().unwrap();
            let mut bunker = url::Url::parse(&format!(
                "bunker://{}",
                hex::encode(signer.transport.verifying_key().to_bytes())
            ))
            .unwrap();
            bunker
                .query_pairs_mut()
                .append_pair("relay", &remote.url)
                .append_pair("secret", "test-secret+&");
            std::fs::write(dir.path().join("config.json"), json!({"publicKey":hex::encode(signer.owner.verifying_key().to_bytes()), "relays":[publication.url], "bunker":bunker.as_str(), "timeoutSeconds":1}).to_string()).map_err(error::Error::file).unwrap();
            Self {
                dir,
                signer,
                remote,
                publication,
            }
        }
        fn run(&self, command: &[&str]) -> Output {
            Command::new(env!("CARGO_BIN_EXE_nstr"))
                .env("NSTR_CONFIG_DIR", self.dir.path())
                .env("XDG_CONFIG_HOME", self.dir.path())
                .args(command)
                .arg("--json")
                .output()
                .unwrap()
        }
    }

    #[test]
    fn bunker_signs_with_an_ephemeral_client_and_publishes_verified_notes() {
        let f = Fixture::new(Mode::Success);
        for _ in 0..2 {
            let out = f.run(&["post", "hello 世界"]);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let value: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(value["acceptedRelays"], 1);
            assert_eq!(value["action"], "post");
        }
        let clients = f.signer.clients.lock().unwrap();
        assert_eq!(clients.len(), 2);
        assert_ne!(clients[0], clients[1]);
        let requests = f.signer.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|r| r["method"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "connect",
                "get_public_key",
                "sign_event",
                "connect",
                "get_public_key",
                "sign_event"
            ]
        );
        let publications = f.publication.log.lock().unwrap();
        assert_eq!(publications.len(), 2);
        for message in publications.iter() {
            verify(&message[1]);
            assert_eq!(message[1]["content"], "hello 世界");
            assert_eq!(
                message[1]["pubkey"],
                hex::encode(f.signer.owner.verifying_key().to_bytes())
            );
        }
        assert_eq!(std::fs::read_dir(f.dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn bunker_wrong_public_key_rejection_and_changed_events_never_publish() {
        for (mode, expected) in [
            (Mode::WrongPublic, "does not match"),
            (Mode::Rejected, "rejected sign_event"),
            (Mode::ChangedNote, "changed the requested event"),
            (Mode::BadSignature, "invalid event"),
        ] {
            let f = Fixture::new(mode);
            let out = f.run(&["post", "original"]);
            assert_eq!(
                out.status.code(),
                Some(4),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(String::from_utf8_lossy(&out.stderr).contains(expected));
            assert!(!String::from_utf8_lossy(&out.stderr).contains("test-secret"));
            assert!(!String::from_utf8_lossy(&out.stdout).contains("test-secret"));
            assert!(f.publication.log.lock().unwrap().is_empty());
            if mode == Mode::WrongPublic {
                assert_eq!(f.signer.requests.lock().unwrap().len(), 2);
            }
        }
    }

    #[test]
    fn bunker_auth_url_is_stderr_instruction_and_same_request_can_finish() {
        let f = Fixture::new(Mode::Auth);
        let out = f.run(&["post", "authorized"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .contains("Open this URL yourself: https://signer.invalid/authorize")
        );
        assert!(!String::from_utf8_lossy(&out.stdout).contains("authorize"));
        assert_eq!(f.signer.requests.lock().unwrap().len(), 3);
    }

    #[test]
    fn bunker_timeout_and_unmatched_ids_do_not_publish_or_retry() {
        for mode in [Mode::Timeout, Mode::WrongIds] {
            let f = Fixture::new(mode);
            let start = Instant::now();
            let out = f.run(&["post", "timeout"]);
            assert_eq!(out.status.code(), Some(4));
            assert!(start.elapsed() < Duration::from_secs(4));
            assert!(String::from_utf8_lossy(&out.stderr).contains("timed out"));
            assert_eq!(f.signer.requests.lock().unwrap().len(), 1);
            assert!(f.publication.log.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn bunker_is_not_contacted_for_reads() {
        let f = Fixture::new(Mode::Success);
        for command in ["whoami", "timeline", "notifications"] {
            assert!(f.run(&[command]).status.success());
        }
        assert!(f.remote.log.lock().unwrap().is_empty());
    }

    #[test]
    fn key_file_and_bunker_conflict_is_a_config_error_without_sockets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), json!({"publicKey":hex::encode(key().verifying_key().to_bytes()),"keyFile":"must-not-open", "bunker":"bunker://invalid?secret=hidden", "relays":["ws://127.0.0.1:1"]}).to_string()).unwrap();
        for cmd in [vec!["post", "test"], vec!["whoami"]] {
            let out = Command::new(env!("CARGO_BIN_EXE_nstr"))
                .env("NSTR_CONFIG_DIR", dir.path())
                .args(cmd)
                .arg("--json")
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(3));
            let value: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert!(
                value["error"]
                    .as_str()
                    .unwrap()
                    .contains("keyFile and bunker")
            );
            assert!(!String::from_utf8_lossy(&out.stderr).contains("hidden"));
        }
    }
    #[test]
    fn remote_signer_signs_new_kinds_and_dry_run_never_connects() {
        let f = Fixture::new(Mode::Success);
        assert_eq!(error::Error::unavailable("unconfigured").status, 5);
        let out = f.run(&["post", "dry", "--dry-run"]);
        assert!(out.status.success());
        assert!(f.remote.log.lock().unwrap().is_empty());
        let owner = hex::encode(f.signer.owner.verifying_key().to_bytes());
        for args in [
            vec!["follow", owner.as_str()],
            vec!["mute", owner.as_str()],
            vec!["profile-update", r#"{"name":"updated"}"#],
        ] {
            let out = f.run(&args);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let requests = f.signer.requests.lock().unwrap();
        let kinds: Vec<u64> = requests
            .iter()
            .filter(|r| r["method"] == "sign_event")
            .map(|r| {
                serde_json::from_str::<Value>(r["params"][0].as_str().unwrap()).unwrap()["kind"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(kinds, [3, 10000, 0]);
    }
    #[test]
    fn bunker_nip17_encrypts_seals_and_decrypts_gift_wraps() {
        let f = Fixture::new(Mode::Success);
        let inbox = Relay::start(
            vec![],
            Behavior {
                auth: true,
                ..Behavior::default()
            },
        );
        let owner = hex::encode(f.signer.owner.verifying_key().to_bytes());
        let directory = Relay::start(
            vec![event(
                &f.signer.owner,
                10050,
                1,
                "",
                json!([["relay", inbox.url]]),
            )],
            Behavior::default(),
        );
        let path = f.dir.path().join("config.json");
        let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        config["relays"] = json!([directory.url]);
        std::fs::write(&path, config.to_string()).unwrap();
        let out = f.run(&["chat", &owner, "private bunker message"]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            f.signer.clients.lock().unwrap().len(),
            1,
            "encryption and signing must reuse one authorized client"
        );
        let messages = inbox.log.lock().unwrap();
        let wrap = messages.iter().find(|m| m[0] == "EVENT").unwrap()[1].clone();
        verify(&wrap);
        let auth = messages.iter().find(|m| m[0] == "AUTH").unwrap();
        verify(&auth[1]);
        assert_eq!(auth[1]["kind"], 22242);
        assert_eq!(auth[1]["pubkey"], owner);
        assert_ne!(wrap["pubkey"], owner);
        assert_eq!(wrap["kind"], 1059);
        assert!(!wrap.to_string().contains("private bunker message"));
        drop(messages);
        let incoming = Relay::start(vec![wrap], Behavior::default());
        let directory = Relay::start(
            vec![event(
                &f.signer.owner,
                10050,
                2,
                "",
                json!([["relay", incoming.url]]),
            )],
            Behavior::default(),
        );
        config["relays"] = json!([directory.url]);
        std::fs::write(&path, config.to_string()).unwrap();
        let out = f.run(&["chat", &owner]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["text"], "private bunker message");
        assert_eq!(
            f.signer.clients.lock().unwrap().len(),
            2,
            "each CLI process connects once, including multi-layer decryption"
        );
        let requests = f.signer.requests.lock().unwrap();
        assert!(requests.iter().any(|r| r["method"] == "nip44_encrypt"));
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["method"] == "nip44_decrypt")
                .count(),
            2
        );
    }
}
