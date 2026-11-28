//! Local configuration, common JSON keys and output failure behavior.

use super::*;
use crate::api::nip19::Identifier;
use k256::{elliptic_curve::Generate, schnorr::SigningKey};
use serde_json::json;
use std::{path::PathBuf, process::Command};

#[path = "../../../doc/json_keys.rs"]
mod json_keys;

fn key() -> SigningKey {
    SigningKey::generate()
}

// Give config tests their own process environment without mutating the test runner's.
fn isolated_config_dir(test_name: &str) -> Option<PathBuf> {
    if std::env::var("NSTR_CONFIG_TEST").as_deref() == Ok(test_name) {
        return Some(std::env::var_os(config::CONFIG_DIR_ENV).unwrap().into());
    }
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env("NSTR_CONFIG_TEST", test_name)
        .env(config::CONFIG_DIR_ENV, dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    None
}

fn assert_shape(value: &Value, keys: &[&str]) {
    let have: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(json_keys::missing(keys, &have).is_empty());
    assert!(have.iter().all(|key| !key.contains('_')));
}

#[test]
fn common_output_shapes_and_safe_text() {
    let event = Event::sign(&key(), 1, 1, vec![], "hello\u{1b}]52;c;bad\u{7}".into()).unwrap();
    let post = output::post(&event).unwrap();
    assert_shape(&post, json_keys::POST);
    assert_shape(&output::publication(&event, 2).unwrap(), json_keys::WROTE);
    assert_shape(&post["user"], json_keys::USER);
    assert_eq!(post["createdAt"], "1970-01-01T00:00:01Z");
    let mut out = Vec::new();
    assert!(output::emit(&mut out, &post, false).unwrap());
    assert!(!out.contains(&0x1b) && !out.contains(&7));
    assert_shape(
        &output::account(&event.pubkey, None).unwrap(),
        json_keys::ACCOUNT,
    );
    let notif = output::notification(&event).unwrap();
    assert_shape(&notif, json_keys::NOTIFICATION);
    assert!(json_keys::NOTIFICATION_TYPES.contains(&notif["type"].as_str().unwrap()));
    assert!(
        post["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|file| json_keys::FILE_TYPES.contains(&file["type"].as_str().unwrap()))
    );
}

#[test]
fn reply_markers_and_legacy_positions() {
    let id = "ab".repeat(32);
    let other = "cd".repeat(32);
    for (tags, expected) in [
        (vec![vec!["e", &id, "", "mention"]], None),
        (vec![vec!["e", &id, "", "root"]], Some(id.as_str())),
        (
            vec![vec!["e", &id, "", "root"], vec!["e", &other, "", "reply"]],
            Some(other.as_str()),
        ),
        (
            vec![vec!["e", &id], vec!["e", &other]],
            Some(other.as_str()),
        ),
        (vec![vec!["e", "malformed"]], None),
    ] {
        let tags = tags
            .into_iter()
            .map(|t| t.into_iter().map(str::to_owned).collect())
            .collect();
        let event = Event::sign(&key(), 1, 1, tags, "reply".into()).unwrap();
        assert_eq!(event.reply_id(), expected);
    }
}

#[test]
fn broken_pipe_is_success_and_other_write_errors_are_local() {
    struct Fail(io::ErrorKind);
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(self.0))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(!output::emit(&mut Fail(io::ErrorKind::BrokenPipe), &json!({}), true).unwrap());
    assert_eq!(
        output::emit(
            &mut Fail(io::ErrorKind::PermissionDenied),
            &json!({}),
            false
        )
        .unwrap_err()
        .status,
        3
    );
}

#[test]
fn cli_requires_positive_limits() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
    for cmd in ["timeline", "notifications"] {
        for count in ["0", "-1", "no"] {
            assert!(Cli::try_parse_from(["nstr", cmd, "-n", count]).is_err());
        }
        assert!(Cli::try_parse_from(["nstr", cmd, "--limit", "1"]).is_ok());
    }
}

#[test]
fn config_and_keys_are_validated_without_disclosing_secret_contents() {
    let Some(dir) = isolated_config_dir(
        "tests::config_and_keys_are_validated_without_disclosing_secret_contents",
    ) else {
        return;
    };
    let path = dir.join("config.json");
    let key = key();
    let mut settings = json!({"publicKey": hex::encode(key.verifying_key().to_bytes()), "relays": ["ws://127.0.0.1:1"], "keyFile": "key"});
    std::fs::write(&path, settings.to_string()).unwrap();
    let config = Config::load().unwrap();
    assert!(config.signing_key().is_err());
    let key_path = dir.join("key");
    std::fs::write(&key_path, "invalid-private-material").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            config
                .signing_key()
                .unwrap_err()
                .error
                .contains("chmod 600")
        );
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(
        !config
            .signing_key()
            .unwrap_err()
            .error
            .contains("invalid-private-material")
    );
    for content in [
        hex::encode(key.to_bytes()),
        Identifier::Secret(key.to_bytes().into()).encode().unwrap(),
    ] {
        std::fs::write(&key_path, content).unwrap();
        assert_eq!(
            config.signing_key().unwrap().verifying_key(),
            key.verifying_key()
        );
    }
    settings["publicKey"] = json!(
        Identifier::Profile {
            public: key.verifying_key().to_bytes().into(),
            relays: vec!["wss://unused.invalid".into()]
        }
        .encode()
        .unwrap()
    );
    std::fs::write(&path, settings.to_string()).unwrap();
    assert_eq!(Config::load().unwrap().relays, ["ws://127.0.0.1:1/"]);
    for bad in [
        json!([]),
        json!(["https://example.invalid"]),
        json!(["ws://user:password@example.invalid"]),
        json!(["ws://example.invalid/#fragment"]),
    ] {
        settings["relays"] = bad;
        std::fs::write(&path, settings.to_string()).unwrap();
        assert!(Config::load().is_err());
    }
}

#[test]
fn mismatched_keys_and_unusable_key_paths_are_local_errors() {
    let Some(dir) =
        isolated_config_dir("tests::mismatched_keys_and_unusable_key_paths_are_local_errors")
    else {
        return;
    };
    let path = dir.join("config.json");
    let key_path = dir.join("key");
    let owner = key();
    let other = key();
    std::fs::write(&path, json!({"publicKey": hex::encode(owner.verifying_key().to_bytes()), "keyFile": "key", "relays": ["ws://127.0.0.1:1"]}).to_string()).unwrap();
    std::fs::write(&key_path, hex::encode(other.to_bytes())).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = Config::load().unwrap();
    assert!(
        config
            .signing_key()
            .unwrap_err()
            .error
            .contains("does not match")
    );
    std::fs::write(&key_path, "0".repeat(64)).unwrap();
    assert!(
        config
            .signing_key()
            .unwrap_err()
            .error
            .contains("invalid secret scalar")
    );
    std::fs::write(&key_path, "x".repeat(1025)).unwrap();
    assert!(config.signing_key().unwrap_err().error.contains("1024"));
    std::fs::remove_file(&key_path).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("missing", &key_path).unwrap();
        assert!(config.signing_key().unwrap_err().error.contains("symlink"));
        std::fs::remove_file(&key_path).unwrap();
    }
    std::fs::create_dir(&key_path).unwrap();
    assert!(config.signing_key().is_err());
}
