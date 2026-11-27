//! Logging in with MiAuth: msky makes a session ID, the user opens
//! `https://<host>/miauth/<session>?name=msky&permission=...` and allows the
//! access there, signed in to the server in the browser, and msky asks
//! `POST /api/miauth/<session>/check` every few seconds until the server
//! hands over the token. msky never sees the password.

use std::time::Duration;

use serde_json::Value;

use crate::api::Client;
use crate::api::types::User;
use crate::error::{Error, Kind, Result};

/// The name the server shows on the page that asks for the access.
pub const APP_NAME: &str = "msky";

/// What msky asks to do. The home and social timelines and the
/// notifications need an account to read; the reactions and the accounts
/// followed are what those show. `post` and the client write notes (and
/// delete your own, and renote), and upload their files to the drive;
/// `follow` and `unfollow` change whom you follow; the client reacts to
/// notes and marks the notifications read; `mute`, `block` and their
/// lists read and change whom you mute and block. Nothing else is asked:
/// msky changes no setting and deletes no account.
pub const PERMISSIONS: &[&str] = &[
    "read:account",
    "read:notifications",
    "read:following",
    "read:reactions",
    "write:notes",
    "write:drive",
    "write:following",
    "write:reactions",
    "write:notifications",
    "read:mutes",
    "write:mutes",
    "read:blocks",
    "write:blocks",
    "read:chat",
    "write:chat",
    "write:account",
    "write:report-abuse",
];

/// How long `msky login` waits for the access by default.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// How long msky waits between two checks.
pub const INTERVAL: Duration = Duration::from_secs(2);

/// Environment variable naming the session ID to use instead of a random
/// one: the E2E suite's stand-in server answers one known address.
pub const SESSION_ENV: &str = "MSKY_MIAUTH_SESSION";

/// The session ID: the one `MSKY_MIAUTH_SESSION` names, else a new random
/// one. A named one goes in the address as it is, so it is letters, digits
/// and `-` only.
pub fn session() -> Result<String> {
    match std::env::var(SESSION_ENV) {
        Ok(v) if !v.is_empty() => named_session(&v),
        _ => new_session(),
    }
}

fn named_session(v: &str) -> Result<String> {
    if v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Ok(v.to_string())
    } else {
        Err(Error::usage(format!(
            "{SESSION_ENV} is not a session ID: letters, digits and - only, 64 at most"
        )))
    }
}

/// A new session ID: a random (version 4) UUID.
pub fn new_session() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| {
        Error::new(
            Kind::Io,
            format!("cannot make a login session: no randomness: {e}"),
        )
    })?;
    Ok(uuid_v4(b))
}

/// The 16 bytes `b` as a version 4 UUID.
fn uuid_v4(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The page on the server at `origin` that asks for the access.
pub fn auth_url(origin: &str, session: &str) -> String {
    format!(
        "{origin}/miauth/{session}?name={APP_NAME}&permission={}",
        PERMISSIONS.join(",")
    )
}

/// The token the server gave and the account it is of. No `Debug`: it holds
/// the token.
pub struct Access {
    pub token: String,
    pub user: User,
}

/// The login to save for the access given on `client`'s server: the
/// account the check named, else the one the server says the token is of.
pub fn session_of(client: &Client, access: Access) -> Result<crate::config::Session> {
    let me = if access.user.id.is_empty() || access.user.username.is_empty() {
        Client::new(client.origin())
            .with_token(Some(access.token.clone()))
            .me()?
    } else {
        access.user
    };
    Ok(crate::config::Session {
        service: client.origin().to_string(),
        user_id: me.id,
        username: me.username,
        token: access.token,
        permissions: PERMISSIONS.iter().map(|p| p.to_string()).collect(),
    })
}

/// Time as `wait_for_access` sees it, so the tests wait without sleeping.
pub trait Clock {
    /// How long it has been since the wait began.
    fn elapsed(&self) -> Duration;
    fn sleep(&mut self, d: Duration);
}

/// The clock on the wall.
pub struct WallClock(std::time::Instant);

impl WallClock {
    pub fn start() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Clock for WallClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// Ask the server whether the access was given, every `interval`, until it
/// hands over the token or `timeout` has gone by. A server that turns the
/// check down has been told no; that, and a wait that runs out, is exit 1
/// with what to do, and nothing is saved.
pub fn wait_for_access(
    client: &Client,
    session: &str,
    timeout: Duration,
    interval: Duration,
    clock: &mut dyn Clock,
) -> Result<Access> {
    let host = client.host().to_string();
    loop {
        match client.miauth_check(session)? {
            Ok(v) => {
                if let Some(access) = took(&v)? {
                    return Ok(access);
                }
            }
            Err(r) if (400..500).contains(&r.status) => {
                return Err(Error::new(
                    Kind::Usage,
                    format!("{host} did not give msky the access: {}", r.said()),
                )
                .with_hint("nothing was saved; run `msky login` to ask again"));
            }
            Err(r) => {
                return Err(Error::api(format!(
                    "miauth check failed: {}",
                    crate::text::one_line(&r.said())
                )));
            }
        }
        let gone = clock.elapsed();
        if gone >= timeout {
            return Err(Error::new(
                Kind::Usage,
                format!(
                    "the access was not given on {host} within {} s",
                    timeout.as_secs()
                ),
            )
            .with_hint(
                "nothing was saved; if you denied it, that is all, else run `msky login` and allow it on the page",
            ));
        }
        clock.sleep(interval.min(timeout - gone));
    }
}

/// The access in a check's answer, `None` while it is not given yet. The
/// token is never quoted in an error.
fn took(v: &Value) -> Result<Option<Access>> {
    if v.get("ok").and_then(Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let token = v
        .get("token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| Error::api("the server said the access was given, but sent no token"))?;
    let user: User = v
        .get("user")
        .cloned()
        .and_then(|u| serde_json::from_value(u).ok())
        .unwrap_or_default();
    Ok(Some(Access {
        token: token.to_string(),
        user,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn a_session_is_a_version_4_uuid() {
        assert_eq!(uuid_v4([0xff; 16]), "ffffffff-ffff-4fff-bfff-ffffffffffff");
        assert_eq!(uuid_v4([0; 16]), "00000000-0000-4000-8000-000000000000");
        let a = new_session().unwrap();
        let b = new_session().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
    }

    #[test]
    fn a_named_session_is_letters_digits_and_dashes() {
        assert_eq!(named_session("e2e-0001").unwrap(), "e2e-0001");
        for bad in ["../x", "a?b", "a b", "セッション", &"a".repeat(65)] {
            assert_eq!(named_session(bad).unwrap_err().kind(), Kind::Usage, "{bad}");
        }
    }

    // The page asks for reading, and for writing notes, drive files,
    // follows, reactions and the notifications' read mark only: nothing
    // that deletes an account or changes a setting.
    #[test]
    fn the_page_asks_for_what_the_commands_need_and_no_more() {
        let url = auth_url("https://misskey.io", "abc");
        assert_eq!(
            url,
            "https://misskey.io/miauth/abc?name=msky&permission=read:account,read:notifications,read:following,read:reactions,write:notes,write:drive,write:following,write:reactions,write:notifications,read:mutes,write:mutes,read:blocks,write:blocks,read:chat,write:chat,write:account,write:report-abuse"
        );
        let writes: Vec<&str> = PERMISSIONS
            .iter()
            .copied()
            .filter(|p| !p.starts_with("read:"))
            .collect();
        assert_eq!(
            writes,
            [
                "write:notes",
                "write:drive",
                "write:following",
                "write:reactions",
                "write:notifications",
                "write:mutes",
                "write:blocks",
                "write:chat",
                "write:account",
                "write:report-abuse"
            ]
        );
    }

    #[rstest]
    #[case(serde_json::json!({"ok": false}))]
    #[case(serde_json::json!({}))]
    #[case(serde_json::json!(null))]
    #[case(serde_json::json!({"ok": "true", "token": "x"}))]
    fn an_answer_without_ok_true_is_not_given_yet(#[case] v: Value) {
        assert!(took(&v).unwrap().is_none());
    }

    #[test]
    fn ok_without_a_token_is_an_error() {
        let e = took(&serde_json::json!({"ok": true, "token": ""}))
            .err()
            .unwrap();
        assert_eq!(e.kind(), Kind::Api);
    }
}
