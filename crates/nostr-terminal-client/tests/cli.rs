//! Every network interaction in this suite is with an in-process loopback relay.

#[cfg(test)]
mod support;

#[cfg(test)]
mod tests {
    use super::support;

    use k256::schnorr::{Signature, SigningKey, VerifyingKey, signature::hazmat::PrehashVerifier};
    use k256::sha2::{Digest, Sha256};
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader},
        path::PathBuf,
        process::{Command, Output, Stdio},
        time::{Duration, Instant},
    };
    use support::{Behavior, Relay, event, key};

    struct Fixture {
        dir: tempfile::TempDir,
        path: PathBuf,
    }

    impl Fixture {
        fn new(key: &SigningKey, relays: &[&Relay], secret: bool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("config.json");
            let key_path = dir.path().join("generated-test-key");
            if secret {
                std::fs::write(&key_path, hex::encode(key.to_bytes())).unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                        .unwrap();
                }
            }
            std::fs::write(&path, json!({"publicKey": hex::encode(key.verifying_key().to_bytes()), "keyFile": key_path, "relays": relays.iter().map(|r| &r.url).collect::<Vec<_>>(), "timeoutSeconds": 1}).to_string()).unwrap();
            Self { dir, path }
        }

        fn command(&self) -> Command {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_nstr"));
            cmd.env("NSTR_CONFIG_DIR", self.dir.path())
                .env("XDG_CONFIG_HOME", self.dir.path())
                .env("TZ", "UTC");
            cmd
        }

        fn run(&self, args: &[&str]) -> Output {
            self.command().args(args).output().unwrap()
        }
    }

    fn lines(output: &Output) -> Vec<Value> {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout.clone())
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    #[test]
    fn help_and_cli_errors_work_without_configuration_or_sockets() {
        let dir = tempfile::tempdir().unwrap();
        for (args, status) in [
            (vec!["--help"], 0),
            (vec!["--version"], 0),
            (vec!["--json"], 1),
            (vec!["timeline", "-n", "0", "--json"], 1),
            (vec!["post", "", "--json"], 1),
            (vec!["--config", "unused.json", "--json"], 1),
            (vec!["whoami", "--json"], 3),
        ] {
            let out = Command::new(env!("CARGO_BIN_EXE_nstr"))
                .env("NSTR_CONFIG_DIR", dir.path())
                .args(&args)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(status));
            if args == ["--help"] {
                let help = String::from_utf8_lossy(&out.stdout);
                assert!(help.contains("NSTR_CONFIG_DIR"));
                assert!(help.contains("nstr/config.json"));
                assert!(!help.contains("--config"));
            }
            if status != 0 {
                let error: Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(error["status"], status);
                assert!(error["error"].is_string() && error.get("hint").is_some());
                assert!(String::from_utf8_lossy(&out.stderr).starts_with("error: "));
            }
        }
    }

    #[test]
    fn config_directory_override_ignores_the_old_file_variable() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("custom config");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.json"), "{}").unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_nstr"))
            .env("NSTR_CONFIG_DIR", &config_dir)
            .env("NSTR_CONFIG", dir.path().join("unused.json"))
            .env("XDG_CONFIG_HOME", dir.path())
            .args(["whoami", "--json"])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(3));
        let error: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            error["error"]
                .as_str()
                .unwrap()
                .contains("missing field `publicKey`")
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unset_and_empty_config_directory_use_the_platform_nstr_directory() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("nstr");
        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.json"), "{}").unwrap();
        for value in [None, Some("")] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_nstr"));
            cmd.env_remove("NSTR_CONFIG_DIR");
            if let Some(value) = value {
                cmd.env("NSTR_CONFIG_DIR", value);
            }
            let out = cmd
                .env("XDG_CONFIG_HOME", dir.path())
                .args(["whoami", "--json"])
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(3));
            let error: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert!(
                error["error"]
                    .as_str()
                    .unwrap()
                    .contains("missing field `publicKey`")
            );
        }
    }

    #[test]
    fn whoami_reads_latest_profile_across_relays_without_opening_key_file() {
        let key = key();
        let first = Relay::start(
            vec![event(&key, 0, 1, r#"{"name":"old"}"#, json!([]))],
            Behavior::default(),
        );
        let second = Relay::start(
            vec![event(
                &key,
                0,
                2,
                r#"{"name":"alice","display_name":"Alice","about":"hello"}"#,
                json!([]),
            )],
            Behavior::default(),
        );
        let fixture = Fixture::new(&key, &[&first, &second], false);
        let values = lines(&fixture.run(&["whoami", "--json"]));
        assert_eq!(values[0]["name"], "Alice");
        assert_eq!(values[0]["description"], "hello");
        assert_eq!(values[0]["id"], hex::encode(key.verifying_key().to_bytes()));
        assert!(values[0]["username"].as_str().unwrap().starts_with("npub1"));
        assert!(
            values[0]["nprofile"]
                .as_str()
                .unwrap()
                .starts_with("nprofile1")
        );
        for relay in [&first, &second] {
            let log = relay.log.lock().unwrap();
            assert_eq!(log.iter().filter(|v| v[0] == "REQ").count(), 1);
            assert_eq!(log.iter().filter(|v| v[0] == "CLOSE").count(), 1);
        }
    }

    #[test]
    fn timeline_pages_and_deduplicates_contacts_notes_and_timestamp_boundaries() {
        let (owner, followed, stranger) = (key(), key(), key());
        let contact = event(
            &owner,
            3,
            1,
            "",
            json!([
                [
                    "p",
                    hex::encode(followed.verifying_key().to_bytes()),
                    "wss://ignored.invalid"
                ],
                ["p", "bad"]
            ]),
        );
        let mut events = vec![
            contact.clone(),
            event(&stranger, 1, 999, "must not appear", json!([])),
        ];
        for i in 0..230 {
            events.push(event(
                &followed,
                1,
                100 + i / 3,
                &format!("note {i}"),
                json!([]),
            ));
        }
        events.push(event(&owner, 1, 1000, "own note", json!([])));
        let first = Relay::start(events.clone(), Behavior::default());
        let second = Relay::start(events, Behavior::default());
        let fixture = Fixture::new(&owner, &[&first, &second], false);
        let values = lines(&fixture.run(&["timeline", "-n", "220", "--json"]));
        assert_eq!(values.len(), 220);
        assert_eq!(values[0]["text"], "own note");
        let ids: std::collections::HashSet<_> = values.iter().map(|v| &v["id"]).collect();
        assert_eq!(ids.len(), 220);
        assert!(values.iter().all(|v| v["text"] != "must not appear"));
        assert!(
            values
                .windows(2)
                .all(|w| w[0]["createdAt"].as_str() >= w[1]["createdAt"].as_str())
        );
        let log = first.log.lock().unwrap();
        let requests: Vec<_> = log.iter().filter(|v| v[0] == "REQ").collect();
        assert!(requests.len() >= 4);
        assert_eq!(
            requests.len(),
            log.iter().filter(|v| v[0] == "CLOSE").count()
        );
        assert!(requests.iter().any(|v| v[2].get("until").is_some()));
    }

    #[test]
    fn short_relay_pages_are_followed_until_limit_or_exhaustion() {
        let owner = key();
        let events = (0..7)
            .map(|i| event(&owner, 1, i, &format!("note {i}"), json!([])))
            .collect();
        let relay = Relay::start(
            events,
            Behavior {
                page_cap: Some(2),
                ..Behavior::default()
            },
        );
        let fixture = Fixture::new(&owner, &[&relay], false);
        assert_eq!(
            lines(&fixture.run(&["timeline", "-n", "10", "--json"])).len(),
            7
        );
    }

    #[test]
    fn notifications_select_mentions_and_classify_reply_tags() {
        let (owner, author) = (key(), key());
        let public = hex::encode(owner.verifying_key().to_bytes());
        let parent = event(&owner, 1, 1, "parent", json!([]));
        let relay = Relay::start(
            vec![
                event(
                    &author,
                    1,
                    3,
                    "reply",
                    json!([["p", public], ["e", parent["id"], "", "root"]]),
                ),
                event(
                    &author,
                    1,
                    2,
                    "mention",
                    json!([["p", public], ["e", parent["id"], "", "mention"]]),
                ),
                event(&author, 1, 4, "unrelated", json!([])),
                parent,
            ],
            Behavior::default(),
        );
        let fixture = Fixture::new(&owner, &[&relay], false);
        let values = lines(&fixture.run(&["notifications", "--json"]));
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["type"], "reply");
        assert_eq!(values[1]["type"], "mention");
        assert_eq!(values[0]["postId"], values[0]["post"]["id"]);
        assert_eq!(
            lines(&fixture.run(&["notifications", "-n", "1", "--json"])).len(),
            1
        );
    }

    #[test]
    fn publish_signs_once_sends_identical_events_and_requires_matching_ok() {
        let owner = key();
        let accepted = Relay::start(vec![], Behavior::default());
        let rejected = Relay::start(
            vec![],
            Behavior {
                reject: true,
                ..Behavior::default()
            },
        );
        let fixture = Fixture::new(&owner, &[&accepted, &rejected], true);
        let result = fixture.run(&["post", "hello 世界\nnext", "--json"]);
        let values = lines(&result);
        assert_eq!(values[0]["action"], "post");
        for name in ["target", "id", "url", "already"] {
            assert!(values[0].get(name).is_some());
        }
        assert_eq!(values[0]["acceptedRelays"], 1);
        assert!(String::from_utf8_lossy(&result.stderr).contains("blocked: test"));
        let log_a = accepted.log.lock().unwrap();
        let log_b = rejected.log.lock().unwrap();
        assert_eq!(*log_a, *log_b);
        assert_eq!(log_a.len(), 1);
        let signed = &log_a[0][1];
        assert_eq!(signed["kind"], 1);
        assert_eq!(signed["content"], "hello 世界\nnext");
        let digest: [u8; 32] = Sha256::digest(
            json!([
                0,
                signed["pubkey"],
                signed["created_at"],
                1,
                [],
                signed["content"]
            ])
            .to_string()
            .as_bytes(),
        )
        .into();
        assert_eq!(hex::encode(digest), signed["id"]);
        let signature = Signature::try_from(
            hex::decode(signed["sig"].as_str().unwrap())
                .unwrap()
                .as_slice(),
        )
        .unwrap();
        let public =
            VerifyingKey::from_slice(&hex::decode(signed["pubkey"].as_str().unwrap()).unwrap())
                .unwrap();
        public.verify_prehash(&digest, &signature).unwrap();
    }

    #[test]
    fn relay_failures_have_network_status_and_timeouts_are_bounded() {
        for behavior in [
            Behavior {
                reject: true,
                ..Behavior::default()
            },
            Behavior {
                wrong_ack: true,
                ..Behavior::default()
            },
            Behavior {
                stall: true,
                ..Behavior::default()
            },
        ] {
            let owner = key();
            let relay = Relay::start(vec![], behavior);
            let fixture = Fixture::new(&owner, &[&relay], true);
            let start = Instant::now();
            let output = fixture.run(&["post", "test", "--json"]);
            assert_eq!(output.status.code(), Some(4));
            assert!(start.elapsed() < Duration::from_secs(5));
            let error: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(error["status"], 4);
            assert!(error.get("hint").is_some());
            assert_eq!(
                relay
                    .log
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|v| v[0] == "EVENT")
                    .count(),
                1
            );
        }
    }

    #[test]
    fn invalid_events_closed_subscriptions_and_malformed_messages_are_rejected() {
        let owner = key();
        let mut invalid = event(&owner, 0, 1, "{}", json!([]));
        invalid["content"] = json!("tampered");
        for (events, behavior) in [
            (vec![invalid], Behavior::default()),
            (
                vec![],
                Behavior {
                    closed: true,
                    ..Behavior::default()
                },
            ),
            (
                vec![],
                Behavior {
                    malformed: true,
                    ..Behavior::default()
                },
            ),
            (
                vec![event(&key(), 1, 1, "wrong", json!([]))],
                Behavior {
                    wrong_filter: true,
                    ..Behavior::default()
                },
            ),
        ] {
            let relay = Relay::start(events, behavior);
            let fixture = Fixture::new(&owner, &[&relay], false);
            assert_eq!(fixture.run(&["whoami", "--json"]).status.code(), Some(4));
        }
    }

    #[test]
    fn successful_relay_survives_another_relays_timeout() {
        let owner = key();
        let healthy = Relay::start(vec![], Behavior::default());
        let stalled = Relay::start(
            vec![],
            Behavior {
                stall: true,
                ..Behavior::default()
            },
        );
        let fixture = Fixture::new(&owner, &[&healthy, &stalled], false);
        let output = fixture.run(&["whoami", "--json"]);
        assert_eq!(lines(&output).len(), 1);
        assert!(String::from_utf8_lossy(&output.stderr).contains("warning:"));
    }

    #[test]
    fn usage_and_file_errors_do_not_contact_relays() {
        let owner = key();
        let relay = Relay::start(vec![], Behavior::default());
        let fixture = Fixture::new(&owner, &[&relay], false);
        for args in [
            vec!["--json"],
            vec!["timeline", "-n", "0", "--json"],
            vec!["post", " ", "--json"],
            vec!["unknown", "--json"],
        ] {
            let output = fixture.run(&args);
            assert_eq!(output.status.code(), Some(1));
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["status"], 1);
        }
        assert_eq!(
            fixture.run(&["post", "test", "--json"]).status.code(),
            Some(3)
        );
        std::fs::remove_file(&fixture.path).unwrap();
        assert_eq!(fixture.run(&["whoami", "--json"]).status.code(), Some(3));
        assert!(relay.log.lock().unwrap().is_empty());
    }

    #[test]
    fn pipe_closed_by_head_is_a_success_without_a_panic() {
        let owner = key();
        let large = "x".repeat(100_000);
        let events = (1..6)
            .map(|i| event(&owner, 1, i, &large, json!([])))
            .collect();
        let relay = Relay::start(events, Behavior::default());
        let fixture = Fixture::new(&owner, &[&relay], false);
        let mut child = fixture
            .command()
            .args(["timeline", "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut first = String::new();
        reader.read_line(&mut first).unwrap();
        assert!(!first.is_empty());
        drop(reader);
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    }
}
