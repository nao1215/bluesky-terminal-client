//! A small blocking client for the Misskey API calls msky makes.
//!
//! Every call is `POST https://<host>/api/<endpoint>` with a JSON body. A
//! client made with the token `msky login` saved sends it as the body's `i`,
//! to that server only; one without reads what the server shows to visitors.
//! A call the server turns away for being asked too often (HTTP 429,
//! `RATE_LIMIT_EXCEEDED`), or that a busy gateway drops (502, 503, 504), is
//! tried again after the wait the server asks for, a few times; anything
//! else is returned at once.

pub mod types;

#[cfg(test)]
pub(crate) mod stub_tests;

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Error, Kind, Result};
use types::{ChatMessage, DriveFile, Note, Notification, Reaction, Relation, User};

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

/// The most characters a note's text may have on a server that does not
/// say: Misskey's default `maxNoteTextLength`.
pub const DEFAULT_MAX_NOTE_CHARS: usize = 3000;

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

/// A client of one Misskey server. It has no `Debug`: it holds the token.
#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    origin: String,
    wait: Wait,
    /// The access token of the account logged in on this server.
    token: Option<String>,
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
            token: None,
        }
    }

    /// This client, sending `token` with every call.
    #[must_use]
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token.filter(|t| !t.is_empty());
        self
    }

    /// Whether calls are made as an account.
    pub fn logged_in(&self) -> bool {
        self.token.is_some()
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
        match self.send(endpoint, body)? {
            Answer::Took(v) => Ok(v),
            Answer::Refused(r) => Err(self.refusal(endpoint, r.status, r.detail)),
        }
    }

    /// Call `endpoint` with `body`, telling a refusal (the server answered,
    /// and said no) from a call that failed on the way.
    fn send(&self, endpoint: &str, body: &Value) -> Result<Answer> {
        let url = format!("{}/api/{endpoint}", self.origin);
        let sent = self.signed(body);
        self.exchange(endpoint, true, &|| {
            self.agent
                .post(&url)
                .header("Content-Type", "application/json")
                .send(sent.as_str())
        })
    }

    /// `body` with the token in it as `i`, as the server reads it.
    fn signed(&self, body: &Value) -> String {
        match (&self.token, body) {
            (Some(t), Value::Object(fields)) => {
                let mut fields = fields.clone();
                fields.insert("i".into(), json!(t));
                Value::Object(fields).to_string()
            }
            _ => body.to_string(),
        }
    }

    /// Make the request `make` makes and read the answer. When `retry`, a
    /// call turned away for being asked too often, or dropped by a busy
    /// gateway, is made again after a wait; a write is not, as bsky sends
    /// a write once: a gateway that dropped the answer may have let the
    /// write through.
    fn exchange(
        &self,
        endpoint: &str,
        retry: bool,
        make: &dyn Fn() -> std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Answer> {
        let mut attempt = 0;
        loop {
            let mut resp = make().map_err(|e| transport(endpoint, &e))?;
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
                    .map(Answer::Took)
                    .map_err(|e| Error::api(format!("{endpoint}: unexpected answer: {e}")));
            }
            let err: ErrorBody = serde_json::from_str(&text).unwrap_or_default();
            let limited = status == 429 || err.error.code == "RATE_LIMIT_EXCEEDED";
            let busy = matches!(status, 502..=504);
            if retry && (limited || busy) && attempt < MAX_RETRIES {
                let wait = retry_after
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_secs(1 << attempt))
                    .min(MAX_WAIT);
                (self.wait)(wait, self.host());
                attempt += 1;
                continue;
            }
            return Ok(Answer::Refused(Refusal {
                status,
                detail: err.error,
            }));
        }
    }

    /// Make a write: `endpoint` with `body`, sent once.
    fn write(&self, endpoint: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/api/{endpoint}", self.origin);
        let sent = self.signed(body);
        let answer = self.exchange(endpoint, false, &|| {
            self.agent
                .post(&url)
                .header("Content-Type", "application/json")
                .send(sent.as_str())
        })?;
        match answer {
            Answer::Took(v) => Ok(v),
            Answer::Refused(r) => Err(self.refusal(endpoint, r.status, r.detail)),
        }
    }

    /// `drive/files/create`: upload `data` to the account's drive as
    /// `name`, with `comment` as its alt text, and return the file. Sent
    /// once, as every write is.
    pub fn upload(&self, name: &str, data: &[u8], comment: Option<&str>) -> Result<DriveFile> {
        let endpoint = "drive/files/create";
        let url = format!("{}/api/{endpoint}", self.origin);
        let boundary = boundary()?;
        let mut fields: Vec<(&str, &str)> = Vec::new();
        if let Some(t) = &self.token {
            fields.push(("i", t));
        }
        fields.push(("name", name));
        if let Some(c) = comment.filter(|c| !c.is_empty()) {
            fields.push(("comment", c));
        }
        let body = multipart(&boundary, &fields, name, data);
        let answer = self.exchange(endpoint, false, &|| {
            self.agent
                .post(&url)
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .send(&body[..])
        })?;
        let v = match answer {
            Answer::Took(v) => v,
            Answer::Refused(r) => return Err(self.refusal(endpoint, r.status, r.detail)),
        };
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("{endpoint}: unexpected answer: {e}")))
    }

    /// `notes/create`: post a note; the note the server made.
    pub fn create_note(&self, body: &Value) -> Result<Note> {
        let v = self.write("notes/create", body)?;
        v.get("createdNote")
            .cloned()
            .and_then(|n| serde_json::from_value::<Note>(n).ok())
            .filter(|n| !n.id.is_empty())
            .ok_or_else(|| Error::api("notes/create: the answer has no note in it"))
    }

    /// `notes/reactions/create`: react to note `id` with `reaction` (a
    /// Unicode emoji, or `:name:` for a custom one).
    pub fn react(&self, id: &str, reaction: &str) -> Result<()> {
        self.write(
            "notes/reactions/create",
            &json!({"noteId": id, "reaction": reaction}),
        )
        .map(drop)
    }

    /// `notes/reactions/delete`: take back the reaction given to note `id`.
    pub fn unreact(&self, id: &str) -> Result<()> {
        self.write("notes/reactions/delete", &json!({"noteId": id}))
            .map(drop)
    }

    /// `chat/history`: the latest message of each conversation with an
    /// account, and with `room` of each room.
    pub fn chat_history(&self, room: bool, limit: usize) -> Result<Vec<ChatMessage>> {
        chat_messages(
            "chat/history",
            self.call(
                "chat/history",
                &json!({"limit": limit.clamp(1, MAX_PAGE), "room": room}),
            )?,
        )
    }

    /// The messages with account `user_id` (`chat/messages/user-timeline`)
    /// or in room `room_id` (`chat/messages/room-timeline`), newest first,
    /// older than `until` when given.
    pub fn chat_timeline(
        &self,
        user_id: Option<&str>,
        room_id: Option<&str>,
        limit: usize,
        until: Option<&str>,
    ) -> Result<Vec<ChatMessage>> {
        let (endpoint, mut body) = match (user_id, room_id) {
            (_, Some(r)) => ("chat/messages/room-timeline", json!({"roomId": r})),
            (Some(u), None) => ("chat/messages/user-timeline", json!({"userId": u})),
            (None, None) => return Ok(Vec::new()),
        };
        body["limit"] = json!(limit.clamp(1, MAX_PAGE));
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        chat_messages(endpoint, self.call(endpoint, &body)?)
    }

    /// Send `text` to account `user_id` (`chat/messages/create-to-user`) or
    /// room `room_id` (`chat/messages/create-to-room`); the message made.
    pub fn chat_send(
        &self,
        user_id: Option<&str>,
        room_id: Option<&str>,
        text: &str,
    ) -> Result<ChatMessage> {
        let (endpoint, body) = match (user_id, room_id) {
            (_, Some(r)) => (
                "chat/messages/create-to-room",
                json!({"toRoomId": r, "text": text}),
            ),
            (Some(u), None) => (
                "chat/messages/create-to-user",
                json!({"toUserId": u, "text": text}),
            ),
            (None, None) => return Err(Error::usage("a message goes to an account or a room")),
        };
        let v = self.write(endpoint, &body)?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("{endpoint}: unexpected answer: {e}")))
    }

    /// `emojis`: the server's custom emoji, each its name and its
    /// picture's address.
    pub fn emojis(&self) -> Result<Vec<(String, String)>> {
        let v = self.call("emojis", &json!({}))?;
        let list = v
            .get("emojis")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::api("emojis: the server did not answer with a list"))?;
        Ok(list
            .iter()
            .filter_map(|e| {
                let name = e.get("name")?.as_str()?;
                let url = e.get("url")?.as_str()?;
                (!name.is_empty() && !url.is_empty()).then(|| (name.to_string(), url.to_string()))
            })
            .collect())
    }

    /// `notes/reactions`: a page of who reacted to note `id`, with what,
    /// newest first, older than `until` when given.
    pub fn reactions(&self, id: &str, limit: usize, until: Option<&str>) -> Result<Vec<Reaction>> {
        let mut body = json!({"noteId": id, "limit": limit.clamp(1, MAX_PAGE)});
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        let Value::Array(items) = self.call("notes/reactions", &body)? else {
            return Err(Error::api(
                "notes/reactions: the server did not answer with a list",
            ));
        };
        Ok(items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect())
    }

    /// `notes/renotes`: a page of the renotes of note `id`, newest first.
    pub fn renotes(&self, id: &str, limit: usize, until: Option<&str>) -> Result<Vec<Note>> {
        let mut body = json!({"noteId": id, "limit": limit.clamp(1, MAX_PAGE)});
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        notes(self.call("notes/renotes", &body)?)
    }

    /// A write about account `user_id`: `mute/create`, `mute/delete`,
    /// `blocking/create`, `blocking/delete`.
    pub fn account_write(&self, endpoint: &str, user_id: &str) -> Result<()> {
        self.write(endpoint, &json!({"userId": user_id})).map(drop)
    }

    /// `notes/unrenote`: take back every renote of note `id` the account
    /// made.
    pub fn unrenote(&self, id: &str) -> Result<()> {
        self.write("notes/unrenote", &json!({"noteId": id}))
            .map(drop)
    }

    /// `notes/delete`: delete note `id`, one of the account's own.
    pub fn delete_note(&self, id: &str) -> Result<()> {
        self.write("notes/delete", &json!({"noteId": id})).map(drop)
    }

    /// `notifications/mark-all-as-read`: mark every notification read.
    pub fn mark_all_read(&self) -> Result<()> {
        self.write("notifications/mark-all-as-read", &json!({}))
            .map(drop)
    }

    /// `following/create`: follow the account `user_id`.
    pub fn follow(&self, user_id: &str) -> Result<()> {
        self.write("following/create", &json!({"userId": user_id}))
            .map(drop)
    }

    /// `following/delete`: stop following the account `user_id`.
    pub fn unfollow(&self, user_id: &str) -> Result<()> {
        self.write("following/delete", &json!({"userId": user_id}))
            .map(drop)
    }

    /// `following/requests/cancel`: take back the request to follow the
    /// account `user_id`, which approves its followers.
    pub fn cancel_follow_request(&self, user_id: &str) -> Result<()> {
        self.write("following/requests/cancel", &json!({"userId": user_id}))
            .map(drop)
    }

    /// `GET /url?url=...&lang=...`: the preview the server makes of the
    /// page at `page`, as Misskey's web client asks for it under every
    /// address in a note's text. A refusal is returned as one: the server
    /// answers 422 for a page it could not read, 403 when it makes no
    /// previews.
    pub fn url_preview(
        &self,
        page: &str,
        lang: &str,
    ) -> Result<std::result::Result<Value, Refusal>> {
        let url = format!("{}/url", self.origin);
        Ok(
            match self.exchange("url", true, &|| {
                self.agent
                    .get(&url)
                    .query("url", page)
                    .query("lang", lang)
                    .call()
            })? {
                Answer::Took(v) => Ok(v),
                Answer::Refused(r) => Err(r),
            },
        )
    }

    /// The error for a call the server answered with `status` and `detail`.
    fn refusal(&self, endpoint: &str, status: u16, detail: ErrorDetail) -> Error {
        let detail = Refusal { status, detail };
        let shown = detail.said();
        let detail = detail.detail;
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
                "{host} shows this only to accounts that are logged in; run `msky login --host {host}`"
            )),
            "AUTHENTICATION_FAILED" => err.with_hint(format!(
                "{host} no longer takes the token msky saved; run `msky login --host {host}` again"
            )),
            "PERMISSION_DENIED" => err.with_hint(format!(
                "the token msky has for {host} was not given that permission; run `msky login --host {host}` again"
            )),
            "NO_SUCH_REPLY_TARGET" | "NO_SUCH_RENOTE_TARGET" => err.with_hint(format!(
                "check the note given to --reply or --quote, and that it is on {host}"
            )),
            "MAX_FILE_SIZE_EXCEEDED" | "NO_FREE_SPACE" => err.with_hint(format!(
                "{host} does not take the file; make it smaller, or free space in your drive"
            )),
            "LTL_DISABLED" | "GTL_DISABLED" | "STL_DISABLED" => err.with_hint(format!(
                "{host} does not show this timeline to you; try another one, or log in with `msky login --host {host}`"
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

    /// `users/show` by ID: the account `id`.
    pub fn user_by_id(&self, id: &str) -> Result<User> {
        let v = self.call("users/show", &json!({"userId": id}))?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("users/show: unexpected answer: {e}")))
    }

    /// `users/search`: a page of the accounts that match `query`, from
    /// the `offset`th on (the API pages accounts by offset).
    pub fn search_users(&self, query: &str, limit: usize, offset: usize) -> Result<Vec<User>> {
        let v = self.call(
            "users/search",
            &json!({"query": query, "limit": limit.clamp(1, MAX_PAGE), "offset": offset, "detail": true}),
        )?;
        let Value::Array(items) = v else {
            return Err(Error::api(
                "users/search: the server did not answer with a list",
            ));
        };
        Ok(items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect())
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

    /// `i`: the account the token is of.
    /// The notes of an antenna, a list or a channel (`endpoint` with the
    /// ID under `key`), newest first, older than `until` if given.
    pub fn feed_notes(
        &self,
        endpoint: &str,
        key: &str,
        id: &str,
        limit: usize,
        until: Option<&str>,
    ) -> Result<Vec<Note>> {
        let mut body = json!({"limit": limit.clamp(1, MAX_PAGE)});
        body[key] = json!(id);
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        notes(self.call(endpoint, &body)?)
    }

    /// The ID and name of each item `endpoint` lists: the account's
    /// antennas, its lists, or the channels it follows or favorites.
    pub fn named_items(&self, endpoint: &str) -> Result<Vec<(String, String)>> {
        let v = self.call(endpoint, &json!({"limit": 100}))?;
        let Value::Array(items) = v else {
            return Err(Error::api(format!(
                "{endpoint}: the server did not answer with a list"
            )));
        };
        Ok(items
            .iter()
            .filter_map(|i| {
                Some((
                    i.get("id")?.as_str()?.to_string(),
                    i.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ))
            })
            .collect())
    }

    /// `users/report-abuse`: report account `user_id` to the server's
    /// moderators with `comment`. Sent once, as every write is.
    pub fn report(&self, user_id: &str, comment: &str) -> Result<()> {
        self.call(
            "users/report-abuse",
            &json!({"userId": user_id, "comment": comment}),
        )
        .map(drop)
    }

    /// `i/update`: change the account's own profile with the fields of
    /// `body` (`name`, `description`, `avatarId`, `bannerId`), and return
    /// it as it now is. Sent once, as every write is.
    pub fn update_me(&self, body: &Value) -> Result<User> {
        let v = self.call("i/update", body)?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("i/update: unexpected answer: {e}")))
    }

    pub fn me(&self) -> Result<User> {
        let v = self.call("i", &json!({}))?;
        serde_json::from_value(v).map_err(|e| Error::api(format!("i: unexpected answer: {e}")))
    }

    /// A timeline (`notes/timeline`, `notes/local-timeline`, ...): a page of
    /// notes, newest first, older than `until` when given.
    pub fn timeline(
        &self,
        endpoint: &str,
        renotes: bool,
        limit: usize,
        until: Option<&str>,
    ) -> Result<Vec<Note>> {
        let mut body = json!({"limit": limit.clamp(1, MAX_PAGE), "withRenotes": renotes});
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        notes(self.call(endpoint, &body)?)
    }

    /// `i/notifications`: a page of the account's notifications, newest
    /// first, older than `until` when given. They are left unread: reading
    /// them here changes nothing on the server.
    pub fn notifications(&self, limit: usize, until: Option<&str>) -> Result<Vec<Notification>> {
        let mut body = json!({"limit": limit.clamp(1, MAX_PAGE), "markAsRead": false});
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        let Value::Array(items) = self.call("i/notifications", &body)? else {
            return Err(Error::api(
                "the server did not answer with a list of notifications",
            ));
        };
        Ok(items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect())
    }

    /// `miauth/<session>/check`: whether the access asked for with
    /// `session` was given, and the token when it was.
    /// A refusal is returned as one: a server that is told no is a denial.
    pub fn miauth_check(&self, session: &str) -> Result<std::result::Result<Value, Refusal>> {
        Ok(
            match self.send(&format!("miauth/{session}/check"), &json!({}))? {
                Answer::Took(v) => Ok(v),
                Answer::Refused(r) => Err(r),
            },
        )
    }

    /// `notes/children`: the replies to note `id` (and the quotes of it
    /// with text of their own), oldest first as the server lists them.
    pub fn children(&self, id: &str, limit: usize) -> Result<Vec<Note>> {
        notes(self.call(
            "notes/children",
            &json!({"noteId": id, "limit": limit.clamp(1, MAX_PAGE)}),
        )?)
    }

    /// `notes/conversation`: the notes note `id` answers, its parent first,
    /// then the parent's, up to `limit` of them.
    pub fn conversation(&self, id: &str, limit: usize) -> Result<Vec<Note>> {
        notes(self.call(
            "notes/conversation",
            &json!({"noteId": id, "limit": limit.clamp(1, MAX_PAGE)}),
        )?)
    }

    /// A page of a list of follows: `users/following` or `users/followers`
    /// with `body` naming the account, or `following/requests/sent` or
    /// `following/requests/list` for the account logged in; older than
    /// `until` when given.
    pub fn relations(
        &self,
        endpoint: &str,
        mut body: Value,
        limit: usize,
        until: Option<&str>,
    ) -> Result<Vec<Relation>> {
        body["limit"] = json!(limit.clamp(1, MAX_PAGE));
        if let Some(u) = until {
            body["untilId"] = json!(u);
        }
        let Value::Array(items) = self.call(endpoint, &body)? else {
            return Err(Error::api(format!(
                "{endpoint}: the server did not answer with a list"
            )));
        };
        Ok(items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect())
    }

    /// `meta`: the most characters a note's text may have on this server
    /// (`maxNoteTextLength`), or [`DEFAULT_MAX_NOTE_CHARS`] when the server
    /// does not say: an old version, a fork, or a call that failed, which
    /// `notes/create` then decides.
    pub fn max_note_chars(&self) -> usize {
        self.call("meta", &json!({"detail": false}))
            .ok()
            .and_then(|v| v.get("maxNoteTextLength").and_then(Value::as_u64))
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_MAX_NOTE_CHARS)
    }

    /// `notes/show`: one note.
    pub fn note(&self, id: &str) -> Result<Note> {
        let v = self.call("notes/show", &json!({"noteId": id}))?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("notes/show: unexpected answer: {e}")))
    }
}

/// What a server answered a call with.
enum Answer {
    Took(Value),
    Refused(Refusal),
}

/// A call the server answered with an error status.
#[derive(Debug)]
pub struct Refusal {
    pub status: u16,
    detail: ErrorDetail,
}

impl Refusal {
    /// The server's code and message, or the status when it wrote neither.
    pub fn said(&self) -> String {
        match (self.detail.code.is_empty(), self.detail.message.is_empty()) {
            (false, false) => format!("{}: {}", self.detail.code, self.detail.message),
            (false, true) => self.detail.code.clone(),
            (true, false) => self.detail.message.clone(),
            (true, true) => format!("HTTP {}", self.status),
        }
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

/// The messages of an answer that is a list of them.
fn chat_messages(endpoint: &str, v: Value) -> Result<Vec<ChatMessage>> {
    let Value::Array(items) = v else {
        return Err(Error::api(format!(
            "{endpoint}: the server did not answer with a list"
        )));
    };
    Ok(items
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// Whether `e`, from a chat call, says the server has no chat or keeps it
/// from this account: an endpoint it does not have (a version before
/// chat), or a role that may not chat.
pub fn chat_unavailable(e: &Error) -> bool {
    let m = e.message();
    m.contains("NO_SUCH_ENDPOINT")
        || m.contains("HTTP 404")
        || m.contains("CHAT_UNAVAILABLE")
        || m.contains("PERMISSION_DENIED")
        || m.contains("ROLE_PERMISSION_DENIED")
}

fn transport(endpoint: &str, e: &ureq::Error) -> Error {
    Error::new(
        Kind::Api,
        format!("{endpoint}: cannot reach the server: {e}"),
    )
    .with_hint("check the network connection and --host")
}

/// A boundary for a multipart body: random, so no file holds it.
fn boundary() -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b)
        .map_err(|e| Error::io(format!("cannot upload a file: no randomness: {e}")))?;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!("msky-{hex}"))
}

/// A `multipart/form-data` body: `fields` as text, then the file `data`
/// named `name`. A quote or a line break in the name would end its header
/// early, so they are written as a browser writes them.
fn multipart(boundary: &str, fields: &[(&str, &str)], name: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(data.len() + 512);
    for (k, v) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                .as_bytes(),
        );
    }
    let shown = name
        .replace('"', "%22")
        .replace('\r', "%0D")
        .replace('\n', "%0A");
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{shown}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
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
