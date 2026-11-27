//! A small blocking client for the Misskey API calls msky makes.
//!
//! Every call is `POST https://<host>/api/<endpoint>` with a JSON body, and
//! none of them is signed in: msky reads what the server shows to visitors.
//! A call the server turns away for being asked too often (HTTP 429,
//! `RATE_LIMIT_EXCEEDED`), or that a busy gateway drops (502, 503, 504), is
//! tried again after the wait the server asks for, a few times; anything
//! else is returned at once.

pub mod types;

#[cfg(test)]
mod stub_tests;

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Error, Kind, Result};
use types::{Note, User};

/// Server used when neither `--host` nor `MSKY_HOST` is given.
pub const DEFAULT_HOST: &str = "misskey.io";

const USER_AGENT: &str = concat!("msky/", env!("CARGO_PKG_VERSION"));

/// How many times a call the server turned away is tried again.
const MAX_RETRIES: u32 = 3;

/// The longest wait before trying again, whatever `Retry-After` asks: a
/// command that hangs for an hour looks broken.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// The largest answer read from the API; a page of 100 notes is well under it.
const MAX_ANSWER_BYTES: u64 = 32 * 1024 * 1024;

/// How long opening a connection may take, all of a host's addresses and
/// the TLS handshake together.
const CONNECT_WITHIN: Duration = Duration::from_secs(10);

/// The most notes one call asks for: the API's limit.
pub const MAX_PAGE: usize = 100;

/// Build the HTTP agent every request uses. Non-2xx statuses are returned as
/// responses so the error body can be read.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        // ureq gives a host's first address two thirds of the global
        // timeout; where IPv6 comes first and goes nowhere, every new
        // connection waited 20 s before it tried IPv4.
        .timeout_connect(Some(CONNECT_WITHIN))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// The server's origin, `https://misskey.io`, from what was typed: a host
/// (`misskey.io`, `@misskey.io`), or an http(s) URL with nothing after the
/// host but a slash. The host is put in lower case. A path is refused
/// rather than left to make every request go to an address that does not
/// exist.
pub fn normalize_host(typed: &str) -> Result<String> {
    let typed = typed.trim();
    let invalid = || {
        Error::usage(format!("invalid host {typed:?}"))
            .with_hint("pass a host such as misskey.io, or an http:// or https:// URL with no path")
    };
    let (scheme, rest) = match typed.split_once("://") {
        Some((s, r)) if s.eq_ignore_ascii_case("https") || s.eq_ignore_ascii_case("http") => {
            (s.to_ascii_lowercase(), r)
        }
        Some(_) => return Err(invalid()),
        None => ("https".to_string(), typed.trim_start_matches('@')),
    };
    let host = rest.strip_suffix('/').unwrap_or(rest);
    if host.is_empty()
        || host.contains(|c: char| c.is_whitespace() || c.is_control())
        || host.contains(['/', '?', '#', '@'])
    {
        return Err(invalid());
    }
    Ok(format!("{scheme}://{}", host.to_ascii_lowercase()))
}

/// The host (and port) of an origin `normalize_host` made.
pub fn authority(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, rest)| rest)
}

/// What a server that turned a call away wrote: `{"error": {"code", "message"}}`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ErrorDetail {
    code: String,
    message: String,
}

/// Waits before a call is tried again; the tests wait without sleeping.
type Wait = Arc<dyn Fn(Duration, &str) + Send + Sync>;

/// A client of one Misskey server.
#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    origin: String,
    wait: Wait,
}

impl Client {
    /// A client of the server at `origin` (see [`normalize_host`]).
    pub fn new(origin: &str) -> Self {
        Self::with_wait(origin, Arc::new(wait_and_say))
    }

    /// [`Client::new`] that waits with `wait` before a call is tried again.
    fn with_wait(origin: &str, wait: Wait) -> Self {
        Self {
            agent: agent(),
            origin: origin.to_string(),
            wait,
        }
    }

    /// The server's origin.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The server's host, as an account's `@name@host` is written.
    pub fn host(&self) -> &str {
        authority(&self.origin)
    }

    /// Call `endpoint` with `body`; the answer as the server wrote it.
    pub fn call(&self, endpoint: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/api/{endpoint}", self.origin);
        let sent = body.to_string();
        let mut attempt = 0;
        loop {
            let mut resp = self
                .agent
                .post(&url)
                .header("Content-Type", "application/json")
                .send(sent.as_str())
                .map_err(|e| transport(endpoint, &e))?;
            let status = resp.status().as_u16();
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok());
            let text = resp
                .body_mut()
                .with_config()
                .limit(MAX_ANSWER_BYTES)
                .read_to_string()
                .map_err(|e| Error::api(format!("{endpoint}: cannot read the answer: {e}")))?;
            if (200..300).contains(&status) {
                let text = if text.trim().is_empty() {
                    "null"
                } else {
                    &text
                };
                return serde_json::from_str(text)
                    .map_err(|e| Error::api(format!("{endpoint}: unexpected answer: {e}")));
            }
            let err: ErrorBody = serde_json::from_str(&text).unwrap_or_default();
            let limited = status == 429 || err.error.code == "RATE_LIMIT_EXCEEDED";
            let busy = matches!(status, 502..=504);
            if (limited || busy) && attempt < MAX_RETRIES {
                let wait = retry_after
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_secs(1 << attempt))
                    .min(MAX_WAIT);
                (self.wait)(wait, self.host());
                attempt += 1;
                continue;
            }
            return Err(self.refusal(endpoint, status, err.error));
        }
    }

    /// The error for a call the server answered with `status` and `detail`.
    fn refusal(&self, endpoint: &str, status: u16, detail: ErrorDetail) -> Error {
        let shown = match (detail.code.is_empty(), detail.message.is_empty()) {
            (false, false) => format!("{}: {}", detail.code, detail.message),
            (false, true) => detail.code.clone(),
            (true, false) => detail.message.clone(),
            (true, true) => format!("HTTP {status}"),
        };
        let err = Error::api(format!("{endpoint} failed: {shown}"));
        let host = self.host();
        match detail.code.as_str() {
            "RATE_LIMIT_EXCEEDED" => err.with_hint(format!(
                "{host} limits how often it is asked; wait a while and try again"
            )),
            "NO_SUCH_USER" | "NO_SUCH_NOTE" => err.with_hint(format!(
                "check the name, and that it is on {host} (or pass --host)"
            )),
            "UNAVAILABLE" if endpoint == "notes/search" => err.with_hint(format!(
                "{host} does not let visitors search notes; try another --host"
            )),
            "SIGNIN_REQUIRED" | "CREDENTIAL_REQUIRED" => err.with_hint(format!(
                "{host} shows this only to accounts that are logged in, and msky cannot log in yet"
            )),
            _ if status == 429 => err.with_hint(format!(
                "{host} limits how often it is asked; wait a while and try again"
            )),
            _ => err,
        }
    }

    /// `users/show`: the account `username` on `host` (`None` for one of
    /// this server).
    pub fn user(&self, username: &str, host: Option<&str>) -> Result<User> {
        let v = self.call("users/show", &json!({"username": username, "host": host}))?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("users/show: unexpected answer: {e}")))
    }

    /// `users/notes`: a page of an account's notes, newest first, older
    /// than `until` when given.
    pub fn user_notes(
        &self,
        user_id: &str,
        filter: &NoteFilter,
        limit: usize,
        until: Option<&str>,
    ) -> Result<Vec<Note>> {
        let mut body = json!({
            "userId": user_id,
            "limit": limit.clamp(1, MAX_PAGE),
            "withReplies": filter.replies,
            "withRenotes": filter.renotes,
            "withFiles": filter.files,
        });
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        notes(self.call("users/notes", &body)?)
    }

    /// `notes/search`: a page of the notes that match `query`, newest
    /// first, older than `until` when given.
    pub fn search(&self, query: &str, limit: usize, until: Option<&str>) -> Result<Vec<Note>> {
        let mut body = json!({"query": query, "limit": limit.clamp(1, MAX_PAGE)});
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        notes(self.call("notes/search", &body)?)
    }

    /// `notes/show`: one note.
    pub fn note(&self, id: &str) -> Result<Note> {
        let v = self.call("notes/show", &json!({"noteId": id}))?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("notes/show: unexpected answer: {e}")))
    }
}

/// Which of an account's notes `users/notes` lists besides its own posts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoteFilter {
    pub replies: bool,
    pub renotes: bool,
    /// Only notes with files.
    pub files: bool,
}

/// The notes of a page. One that cannot be read as a note is left out
/// rather than failing the page; an answer that is not a list is an error.
fn notes(v: Value) -> Result<Vec<Note>> {
    let Value::Array(items) = v else {
        return Err(Error::api("the server did not answer with a list of notes"));
    };
    Ok(items
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

fn transport(endpoint: &str, e: &ureq::Error) -> Error {
    Error::new(
        Kind::Api,
        format!("{endpoint}: cannot reach the server: {e}"),
    )
    .with_hint("check the network connection and --host")
}

/// Say on stderr why the command is quiet, then wait. stdout carries the
/// output, so it is not written there.
fn wait_and_say(wait: Duration, host: &str) {
    use std::io::Write;
    #[expect(
        clippy::let_underscore_must_use,
        reason = "a note on stderr while waiting; the wait and the call go on whether it showed or not"
    )]
    let _: std::io::Result<()> = writeln!(
        std::io::stderr(),
        "{host} asks to wait; trying again in {} s",
        wait.as_secs()
    );
    std::thread::sleep(wait);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("misskey.io", "https://misskey.io")]
    #[case(" Misskey.IO ", "https://misskey.io")]
    #[case("@misskey.io", "https://misskey.io")]
    #[case("https://misskey.io/", "https://misskey.io")]
    #[case("HTTP://127.0.0.1:8080", "http://127.0.0.1:8080")]
    #[case("xn--r8jz45g.xn--zckzah", "https://xn--r8jz45g.xn--zckzah")]
    fn a_host_is_put_as_an_origin(#[case] typed: &str, #[case] want: &str) {
        assert_eq!(normalize_host(typed).unwrap(), want);
    }

    #[rstest]
    #[case("")]
    #[case("https://")]
    #[case("ftp://misskey.io")]
    #[case("https://misskey.io/api")]
    #[case("misskey.io/notes/x")]
    #[case("https://misskey.io?x=1")]
    #[case("mis key.io")]
    #[case("user@misskey.io")]
    fn a_host_with_a_path_or_another_scheme_is_refused(#[case] typed: &str) {
        let e = normalize_host(typed).unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
    }

    #[test]
    fn the_authority_is_what_follows_the_scheme() {
        assert_eq!(authority("https://misskey.io"), "misskey.io");
        assert_eq!(authority("http://127.0.0.1:9"), "127.0.0.1:9");
    }
}
