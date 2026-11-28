//! A small blocking client for the Mastodon API calls mstdn makes.
//!
//! A read is `GET https://<host>/api/...` with its options in the query; a
//! write is a `POST` of JSON (or of a form, or a file). A client made with
//! the token `mstdn login` saved sends it as `Authorization: Bearer`, to
//! that server only; one without reads what the server shows to visitors.
//! A list comes a page at a time, the next page named in the `Link` header
//! (`max_id`), as Mastodon pages them. A read the server turns away for
//! being asked too often (HTTP 429), or that a busy gateway drops (502,
//! 503, 504), is tried again after the wait the server asks for
//! (`Retry-After`, else `X-RateLimit-Reset`), a few times; a write is sent
//! once, and anything else is returned at once.

mod chat;
mod compatibility;
mod deck;
pub(crate) mod profile;
pub(crate) mod types;

#[cfg(test)]
pub(crate) mod stub_tests;

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{Error, Kind, Result};
use types::{Account, Context, Found, Limits, Media, Notification, Registered, Status, Token};

/// Server used when neither `--host` nor `MSTDN_HOST` is given.
pub(crate) const DEFAULT_HOST: &str = "mastodon.social";

const USER_AGENT: &str = concat!("mstdn/", env!("CARGO_PKG_VERSION"));

/// How many times a read the server turned away is tried again.
const MAX_RETRIES: u32 = 3;

/// The longest wait before trying again, whatever the server asks: a
/// command that hangs for an hour looks broken.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// The largest answer read from the API; a page of 40 statuses is well
/// under it.
const MAX_ANSWER_BYTES: u64 = 32 * 1024 * 1024;

/// How long opening a connection may take, all of a host's addresses and
/// the TLS handshake together.
const CONNECT_WITHIN: Duration = Duration::from_secs(10);

/// How long an upload may take: a video of 99 MB on a slow link takes
/// minutes, where a call gets 30 s.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// The most statuses one call asks for: the API's limit for statuses and
/// timelines.
pub(crate) const MAX_PAGE: usize = 40;

/// Build the HTTP agent every request uses. Non-2xx statuses are returned as
/// responses so the error body can be read.
pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        // ureq gives a host's first address two thirds of the global
        // timeout; where IPv6 comes first and goes nowhere, every new
        // connection waited 20 s before it tried IPv4.
        .timeout_connect(Some(CONNECT_WITHIN))
        // The login page sends the browser back; the API itself never
        // redirects a call mstdn makes, and a token must not follow one.
        .max_redirects(0)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// The server's origin, `https://mastodon.social`, from what was typed: a
/// host (`mastodon.social`, `@mastodon.social`), or an http(s) URL with
/// nothing after the host but a slash. The host is put in lower case. A
/// path is refused rather than left to make every request go to an address
/// that does not exist.
pub(crate) fn normalize_host(typed: &str) -> Result<String> {
    let typed = typed.trim();
    let invalid = || {
        Error::usage(format!("invalid host {typed:?}")).with_hint(
            "pass a host such as mastodon.social, or an http:// or https:// URL with no path",
        )
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
        || host.contains(['/', '?', '#', '@', '\\'])
    {
        return Err(invalid());
    }
    Ok(format!("{scheme}://{}", host.to_ascii_lowercase()))
}

/// The host (and port) of an origin `normalize_host` made.
pub(crate) fn authority(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, rest)| rest)
}

/// What a server that turned a call away wrote: `{"error": "..."}`, with
/// `error_description` from the OAuth endpoints.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct ErrorBody {
    error: String,
    error_description: String,
}

/// A call the server answered with an error status.
#[derive(Debug, Clone)]
pub(crate) struct Refusal {
    pub status: u16,
    body: ErrorBody,
    /// When the server takes calls again, as `X-RateLimit-Reset` says.
    reset: Option<String>,
}

impl Refusal {
    /// The server's error and its description, or the status when it wrote
    /// neither.
    pub(crate) fn said(&self) -> String {
        let (e, d) = (self.body.error.trim(), self.body.error_description.trim());
        match (e.is_empty(), d.is_empty()) {
            (false, false) if e != d => format!("{e}: {d}"),
            (false, _) => e.to_string(),
            (true, false) => d.to_string(),
            (true, true) => format!("HTTP {}", self.status),
        }
    }

    /// The OAuth error code (`invalid_grant`, `access_denied`...).
    pub(crate) fn code(&self) -> &str {
        &self.body.error
    }
}

/// Where the next page of a list starts, as the server said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Next {
    /// The answer had no `Link` header: the next page is older than its
    /// last item.
    Unsaid,
    /// The `Link` header names no next page: this was the last.
    End,
    /// The next page is older than this ID (`max_id`).
    MaxId(String),
}

/// A page of a list and where the next starts.
#[derive(Debug, Clone)]
pub(crate) struct Page<T> {
    pub items: Vec<T>,
    pub next: Next,
}

/// What a server answered a call with.
enum Answer {
    Took {
        status: u16,
        value: Value,
        link: Option<String>,
    },
    Refused(Refusal),
}

/// Waits before a call is tried again; the tests wait without sleeping.
type Wait = Arc<dyn Fn(Duration, &str) + Send + Sync>;

/// A client of one Mastodon server. It has no `Debug`: it holds the token.
#[derive(Clone)]
pub(crate) struct Client {
    agent: ureq::Agent,
    origin: String,
    wait: Wait,
    /// The access token of the account logged in on this server.
    token: Option<String>,
}

type Made = std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>;

impl Client {
    /// A client of the server at `origin` (see [`normalize_host`]).
    pub(crate) fn new(origin: &str) -> Self {
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
    pub(crate) fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token.filter(|t| !t.is_empty());
        self
    }

    /// Replace stderr notices when the caller owns the terminal.
    pub(crate) fn with_retry_wait(mut self, wait: Wait) -> Self {
        self.wait = wait;
        self
    }

    /// Whether calls are made as an account.
    pub(crate) fn logged_in(&self) -> bool {
        self.token.is_some()
    }

    /// The server's origin.
    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    /// The server's host, as an account's `@name@host` is written.
    pub(crate) fn host(&self) -> &str {
        authority(&self.origin)
    }

    /// `req` with the token, when there is one.
    fn signed<B>(&self, req: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        match &self.token {
            Some(t) => req.header("Authorization", format!("Bearer {t}")),
            None => req,
        }
    }

    /// `GET /api/<path>` with `query`, read again when the server is busy.
    fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Page<Value>> {
        let url = format!("{}/api/{path}", self.origin);
        let answer = self.exchange(path, true, &|| {
            let mut req = self.agent.get(&url);
            for (k, v) in query {
                req = req.query(*k, v);
            }
            self.signed(req).call()
        })?;
        match answer {
            Answer::Took { value, link, .. } => Ok(Page {
                items: vec![value],
                next: next_of(link.as_deref()),
            }),
            Answer::Refused(r) => Err(self.refusal(path, r)),
        }
    }

    /// The one value `GET /api/<path>` answers.
    fn get_one(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        Ok(self
            .get(path, query)?
            .items
            .into_iter()
            .next()
            .unwrap_or(Value::Null))
    }

    /// A list `GET /api/<path>` answers, each item read as a `T`; one that
    /// cannot be read is left out rather than failing the page.
    fn get_list<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Page<T>> {
        let page = self.get(path, query)?;
        let Some(Value::Array(items)) = page.items.into_iter().next() else {
            return Err(Error::api(format!(
                "{path}: the server did not answer with a list"
            )));
        };
        Ok(Page {
            items: items
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect(),
            next: page.next,
        })
    }

    /// Make the request `make` makes and read the answer. When `retry`, a
    /// call turned away for being asked too often, or dropped by a busy
    /// gateway, is made again after a wait; a write is not: a gateway that
    /// dropped the answer may have let the write through.
    fn exchange(&self, what: &str, retry: bool, make: &dyn Fn() -> Made) -> Result<Answer> {
        let mut attempt = 0;
        loop {
            let mut resp = make().map_err(|e| transport(what, &e))?;
            let status = resp.status().as_u16();
            let header = |name: &str| {
                resp.headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v.trim().to_string())
            };
            let retry_after = header("retry-after").and_then(|v| v.parse::<u64>().ok());
            let reset = header("x-ratelimit-reset");
            let link = header("link");
            let text = resp
                .body_mut()
                .with_config()
                .limit(MAX_ANSWER_BYTES)
                .read_to_string()
                .map_err(|e| Error::api(format!("{what}: cannot read the answer: {e}")))?;
            if (200..300).contains(&status) {
                let text = if text.trim().is_empty() {
                    "null"
                } else {
                    &text
                };
                let value = serde_json::from_str(text)
                    .map_err(|e| Error::api(format!("{what}: unexpected answer: {e}")))?;
                return Ok(Answer::Took {
                    status,
                    value,
                    link,
                });
            }
            let body: ErrorBody = serde_json::from_str(&text).unwrap_or_default();
            let limited = status == 429;
            let busy = matches!(status, 502..=504);
            if retry && (limited || busy) && attempt < MAX_RETRIES {
                let wait = retry_after
                    .map(Duration::from_secs)
                    .or_else(|| reset.as_deref().and_then(until))
                    .unwrap_or_else(|| Duration::from_secs(1 << attempt))
                    .min(MAX_WAIT);
                (self.wait)(wait, self.host());
                attempt += 1;
                continue;
            }
            return Ok(Answer::Refused(Refusal {
                status,
                body,
                reset,
            }));
        }
    }

    /// Make a write, sent once: `POST /api/<path>` with the JSON `body`,
    /// and `Idempotency-Key: key` when given, so a server that got it
    /// twice makes one.
    fn post(&self, path: &str, body: &Value, key: Option<&str>) -> Result<(u16, Value)> {
        let url = format!("{}/api/{path}", self.origin);
        let sent = body.to_string();
        let answer = self.exchange(path, false, &|| {
            let mut req = self
                .agent
                .post(&url)
                .header("Content-Type", "application/json");
            if let Some(k) = key {
                req = req.header("Idempotency-Key", k);
            }
            self.signed(req).send(sent.as_str())
        })?;
        match answer {
            Answer::Took { status, value, .. } => Ok((status, value)),
            Answer::Refused(r) => Err(self.refusal(path, r)),
        }
    }

    /// The error for a call to `what` the server refused.
    fn refusal(&self, what: &str, r: Refusal) -> Error {
        if compatibility::unsupported_endpoint(what, r.status) {
            return self.unsupported(what);
        }
        let shown = crate::text::one_line(&r.said());
        let err = Error::api(format!("{what} failed: {shown}"));
        let host = self.host();
        let said = shown.to_ascii_lowercase();
        match r.status {
            429 => {
                let when = r
                    .reset
                    .as_deref()
                    .map(terminal_kit::clock::local_time)
                    .map(|t| format!(" (it takes calls again at {t})"))
                    .unwrap_or_default();
                err.with_hint(format!(
                    "{host} limits how often it is asked{when}; wait a while and try again"
                ))
            }
            401 | 403 | 422 if !self.logged_in() && said.contains("authenticated") => {
                err.with_hint(format!(
                    "{host} shows this only to accounts that are logged in; run `mstdn login --host {host}`"
                ))
            }
            401 if self.logged_in() => err.with_hint(format!(
                "{host} no longer takes the token mstdn saved; run `mstdn login --host {host}` again"
            )),
            401 => err.with_hint(format!(
                "{host} shows this only to accounts that are logged in; run `mstdn login --host {host}`"
            )),
            403 if said.contains("scope") => err.with_hint(format!(
                "the token mstdn has for {host} was not given that scope; run `mstdn login --host {host}` again"
            )),
            404 if what.starts_with("v1/accounts") || what.starts_with("v1/statuses") => err
                .with_hint(format!(
                    "check the name or the ID, and that it is on {host} (or pass --host)"
                )),
            413 => err.with_hint(format!(
                "{host} does not take a file that large; make it smaller"
            )),
            _ => err,
        }
    }

    /// `GET /api/v1/accounts/lookup`: the account `acct` (`name`, or
    /// `name@host` for one of another server) as this server knows it.
    pub(crate) fn lookup(&self, acct: &str) -> Result<Account> {
        let v = self.get_one("v1/accounts/lookup", &[("acct", acct.to_string())])?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("v1/accounts/lookup: unexpected answer: {e}")))
    }

    /// `GET /api/v2/search` for the account `acct`, asking the server to
    /// fetch it from its own when it does not know it yet (`resolve`, which
    /// takes a login): what `lookup` cannot find of another server.
    pub(crate) fn resolve_account(&self, acct: &str) -> Result<Option<Account>> {
        let found = self.search(&format!("@{acct}"), "accounts", true, 1, 0)?;
        Ok(found.accounts.into_iter().find(|a| {
            a.acct.eq_ignore_ascii_case(acct) || acct.split('@').next() == Some(a.acct.as_str())
        }))
    }

    /// `GET /api/v1/accounts/:id/statuses`: a page of an account's
    /// statuses, newest first, older than `max_id` when given.
    pub(crate) fn account_statuses(
        &self,
        id: &str,
        filter: &StatusFilter,
        limit: usize,
        max_id: Option<&str>,
    ) -> Result<Page<Status>> {
        let mut q = vec![
            ("limit", limit.clamp(1, MAX_PAGE).to_string()),
            ("exclude_replies", filter.exclude_replies.to_string()),
            ("exclude_reblogs", filter.exclude_reblogs.to_string()),
        ];
        if filter.only_media {
            q.push(("only_media", "true".into()));
        }
        if let Some(m) = max_id {
            q.push(("max_id", m.to_string()));
        }
        self.get_list(&format!("v1/accounts/{}/statuses", path_part(id)), &q)
    }

    /// `GET /api/v1/statuses/:id`: one status.
    pub(crate) fn status(&self, id: &str) -> Result<Status> {
        let path = format!("v1/statuses/{}", path_part(id));
        let v = self.get_one(&path, &[])?;
        serde_json::from_value(v).map_err(|e| Error::api(format!("{path}: unexpected answer: {e}")))
    }

    /// `GET /api/v1/statuses/:id/context`: what status `id` answers, and
    /// the replies under it.
    pub(crate) fn context(&self, id: &str) -> Result<Context> {
        let path = format!("v1/statuses/{}/context", path_part(id));
        let v = self.get_one(&path, &[])?;
        serde_json::from_value(v).map_err(|e| Error::api(format!("{path}: unexpected answer: {e}")))
    }

    /// A timeline (`v1/timelines/home`, `v1/timelines/public` with
    /// `extra` saying which part of it, `v1/timelines/tag/<tag>`): a page,
    /// newest first, older than `max_id` when given.
    pub(crate) fn timeline(
        &self,
        path: &str,
        extra: &[(&str, &str)],
        limit: usize,
        max_id: Option<&str>,
    ) -> Result<Page<Status>> {
        let mut q: Vec<(&str, String)> = vec![("limit", limit.clamp(1, MAX_PAGE).to_string())];
        q.extend(extra.iter().map(|(k, v)| (*k, v.to_string())));
        if let Some(m) = max_id {
            q.push(("max_id", m.to_string()));
        }
        self.get_list(path, &q)
    }

    /// `GET /api/v2/search`: the accounts or statuses (`kind`) that match
    /// `q`, from the `offset`th on; `resolve` asks the server to fetch what
    /// an address names from its own server.
    pub(crate) fn search(
        &self,
        q: &str,
        kind: &str,
        resolve: bool,
        limit: usize,
        offset: usize,
    ) -> Result<Found> {
        let mut query = vec![
            ("q", q.to_string()),
            ("type", kind.to_string()),
            ("limit", limit.clamp(1, MAX_PAGE).to_string()),
        ];
        if resolve {
            query.push(("resolve", "true".into()));
        }
        if offset > 0 {
            query.push(("offset", offset.to_string()));
        }
        let v = self.get_one("v2/search", &query)?;
        serde_json::from_value(v)
            .map_err(|e| Error::api(format!("v2/search: unexpected answer: {e}")))
    }

    /// `GET /api/v1/notifications`: a page of the account's notifications,
    /// newest first, older than `max_id` when given. Reading them marks
    /// nothing.
    pub(crate) fn notifications(
        &self,
        limit: usize,
        max_id: Option<&str>,
    ) -> Result<Page<Notification>> {
        let mut q = vec![("limit", limit.clamp(1, MAX_PAGE).to_string())];
        if let Some(m) = max_id {
            q.push(("max_id", m.to_string()));
        }
        self.get_list("v1/notifications", &q)
    }

    /// Read an account by its ID on this server.
    pub(crate) fn account(&self, id: &str) -> Result<Account> {
        let path = format!("v1/accounts/{}", path_part(id));
        serde_json::from_value(self.get_one(&path, &[])?)
            .map_err(|e| Error::api(format!("{path}: unexpected answer: {e}")))
    }

    /// `GET /api/v1/accounts/verify_credentials`: the account the token is of.
    pub(crate) fn me(&self) -> Result<Account> {
        let v = self.get_one("v1/accounts/verify_credentials", &[])?;
        serde_json::from_value(v).map_err(|e| {
            Error::api(format!(
                "v1/accounts/verify_credentials: unexpected answer: {e}"
            ))
        })
    }

    /// The limits of a post on this server: `GET /api/v2/instance`, else
    /// `/api/v1/instance` on a version before 4.0, else Mastodon's defaults,
    /// which `POST /api/v1/statuses` then decides.
    pub(crate) fn limits(&self) -> Limits {
        self.get_one("v2/instance", &[])
            .or_else(|_| self.get_one("v1/instance", &[]))
            .map(|v| Limits::from_instance(&v))
            .unwrap_or_default()
    }

    /// `POST /api/v1/apps`: register mstdn as an app that sends the code
    /// back to `redirect_uris`, with `scopes`.
    pub(crate) fn register_app(
        &self,
        redirect_uris: &[String],
        scopes: &str,
    ) -> Result<Registered> {
        let (_, v) = self.post(
            "v1/apps",
            &json!({
                "client_name": crate::oauth::APP_NAME,
                // One a line: what every version takes.
                "redirect_uris": redirect_uris.join("\n"),
                "scopes": scopes,
                "website": crate::oauth::WEBSITE,
            }),
            None,
        )?;
        let r: Registered = serde_json::from_value(v)
            .map_err(|e| Error::api(format!("v1/apps: unexpected answer: {e}")))?;
        if r.client_id.is_empty() || r.client_secret.is_empty() {
            return Err(Error::api(
                "v1/apps: the server registered mstdn but sent no client ID or secret",
            ));
        }
        Ok(r)
    }

    /// `POST /oauth/token` with `form`: the token, or the server's refusal
    /// (a code already used, an app it no longer knows). Sent once.
    pub(crate) fn token(
        &self,
        form: &[(&str, &str)],
    ) -> Result<std::result::Result<Token, Refusal>> {
        let url = format!("{}/oauth/token", self.origin);
        let answer = self.exchange("oauth/token", false, &|| {
            self.agent.post(&url).send_form(form.iter().copied())
        })?;
        match answer {
            Answer::Took { value, .. } => {
                let t: Token = serde_json::from_value(value)
                    .map_err(|e| Error::api(format!("oauth/token: unexpected answer: {e}")))?;
                if t.access_token.is_empty() {
                    return Err(Error::api("oauth/token: the server sent no token"));
                }
                Ok(Ok(t))
            }
            Answer::Refused(r) => Ok(Err(r)),
        }
    }

    /// `POST /api/v2/media`: upload `data` as `name`, with `description` as
    /// its alt text. The file goes without its metadata: a photo's EXIF, the
    /// place it was taken among it, and a video's (see [`crate::metadata`]).
    /// The server may answer before it has processed the file (202); the
    /// attachment is returned either way, and [`Client::media`] says when
    /// it is ready. Sent once, as every write is.
    pub(crate) fn upload(
        &self,
        name: &str,
        data: &[u8],
        description: Option<&str>,
    ) -> Result<(bool, Media)> {
        let data = crate::metadata::strip(name, data)?;
        let path = "v2/media";
        let url = format!("{}/api/{path}", self.origin);
        let boundary = random_hex("mstdn-")?;
        let body = media_body(&boundary, name, mime_of(name, &data), &data, description);
        let answer = self.exchange(path, false, &|| {
            let req = self
                .agent
                .post(&url)
                .config()
                .timeout_global(Some(UPLOAD_TIMEOUT))
                .build()
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                );
            self.signed(req).send(&body[..])
        })?;
        let (status, v) = match answer {
            Answer::Took { status, value, .. } => (status, value),
            Answer::Refused(r) => return Err(self.refusal(path, r)),
        };
        let m: Media = serde_json::from_value(v)
            .map_err(|e| Error::api(format!("{path}: unexpected answer: {e}")))?;
        if m.id.is_empty() {
            return Err(Error::api(format!(
                "{path}: the answer has no attachment in it"
            )));
        }
        Ok((status == 200 && m.url.is_some(), m))
    }

    /// `GET /api/v1/media/:id`: whether the attachment is processed (200;
    /// 206 while it is not yet).
    pub(crate) fn media(&self, id: &str) -> Result<bool> {
        let path = format!("v1/media/{}", path_part(id));
        let url = format!("{}/api/{path}", self.origin);
        match self.exchange(&path, true, &|| self.signed(self.agent.get(&url)).call())? {
            Answer::Took { status, value, .. } => {
                Ok(status == 200 && value.get("url").is_some_and(|u| !u.is_null()))
            }
            Answer::Refused(r) => Err(self.refusal(&path, r)),
        }
    }

    /// `POST /api/v1/statuses`: post a status; the status the server made.
    /// `key` goes as the `Idempotency-Key`.
    pub(crate) fn create_status(&self, body: &Value, key: &str) -> Result<Status> {
        let (_, v) = self.post("v1/statuses", body, Some(key))?;
        serde_json::from_value::<Status>(v)
            .ok()
            .filter(|s| !s.id.is_empty())
            .ok_or_else(|| Error::api("v1/statuses: the answer has no status in it"))
    }

    pub(crate) fn relationship(&self, id: &str) -> Result<types::Relationship> {
        let page: Page<types::Relationship> =
            self.get_list("v1/accounts/relationships", &[("id[]", id.into())])?;
        page.items
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| Error::api("the server returned no relationship for the account"))
    }

    pub(crate) fn relate(&self, id: &str, action: &str) -> Result<types::Relationship> {
        let path = format!("v1/accounts/{}/{action}", path_part(id));
        let (_, value) = self.post(&path, &json!({}), None)?;
        serde_json::from_value::<types::Relationship>(value)
            .ok()
            .filter(|r| r.id == id)
            .ok_or_else(|| Error::api("the server returned no relationship for the account"))
    }

    pub(crate) fn delete_status(&self, id: &str) -> Result<()> {
        let path = format!("v1/statuses/{}", path_part(id));
        let url = format!("{}/api/{path}", self.origin);
        match self.exchange(&path, false, &|| {
            self.signed(self.agent.delete(&url)).call()
        })? {
            Answer::Took { .. } => Ok(()),
            Answer::Refused(r) => Err(self.refusal(&path, r)),
        }
    }

    pub(crate) fn report(&self, account: &str, status: Option<&str>, comment: &str) -> Result<()> {
        let mut body = json!({"account_id": account, "comment": comment, "forward": false});
        if let Some(id) = status {
            body["status_ids"] = json!([id]);
        }
        self.post("v1/reports", &body, None).map(drop)
    }

    /// `POST /api/v1/statuses/:id/<action>`: `favourite`, `unfavourite`,
    /// `reblog` or `unreblog` the status `id`. Sent once, as every write is.
    pub(crate) fn act_on(&self, id: &str, action: &str) -> Result<()> {
        let path = format!("v1/statuses/{}/{action}", path_part(id));
        self.post(&path, &json!({}), None).map(drop)
    }
}

/// Which statuses of an account `account_statuses` asks for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StatusFilter {
    /// Leave out replies to others (Mastodon keeps replies to oneself).
    pub exclude_replies: bool,
    pub exclude_reblogs: bool,
    /// Only statuses with files.
    pub only_media: bool,
}

/// Where the `Link` header `link` says the next page starts.
pub(crate) fn next_of(link: Option<&str>) -> Next {
    let Some(link) = link.filter(|l| !l.trim().is_empty()) else {
        return Next::Unsaid;
    };
    for part in link.split(',') {
        let Some((target, params)) = part.trim().split_once(';') else {
            continue;
        };
        let is_next = params.split(';').any(|p| {
            p.trim()
                .strip_prefix("rel=")
                .map(|r| r.trim_matches('"'))
                .is_some_and(|r| r.split_whitespace().any(|r| r == "next"))
        });
        if !is_next {
            continue;
        }
        let url = target.trim().trim_start_matches('<').trim_end_matches('>');
        let query = url.split_once('?').map_or("", |(_, q)| q);
        let query = query.split('#').next().unwrap_or(query);
        return query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == "max_id")
            .map(|(_, v)| percent_decode(v))
            .filter(|v| !v.is_empty())
            .map_or(Next::Unsaid, Next::MaxId);
    }
    Next::End
}

/// `s` with its `%XX` escapes decoded; one that is not one is kept.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).copied().and_then(hex),
                b.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `s` as it may stand in a query or a form, every byte but the
/// unreserved ones escaped.
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// An ID as one part of a path: escaped, so `../` or `?` in what was typed
/// cannot make it another address.
fn path_part(id: &str) -> String {
    percent_encode(id)
}

/// How long until the time `reset` (an ISO 8601 time, as
/// `X-RateLimit-Reset` says it), if it is to come.
fn until(reset: &str) -> Option<Duration> {
    let t: jiff::Timestamp = reset.parse().ok()?;
    let d = t.duration_since(jiff::Timestamp::now());
    let secs = d.as_secs_f64();
    (secs > 0.0).then(|| Duration::from_secs_f64(secs.ceil()))
}

fn transport(what: &str, e: &ureq::Error) -> Error {
    Error::new(Kind::Api, format!("{what}: cannot reach the server: {e}"))
        .with_hint("check the network connection and --host")
}

/// `prefix` and 32 random hex digits: a boundary no file holds, a key, a state.
pub(crate) fn random_hex(prefix: &str) -> Result<String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| Error::io(format!("no randomness: {e}")))?;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!("{prefix}{hex}"))
}

/// The type a file is uploaded as: what its first bytes say, else what its
/// name says. The server checks it against the file.
fn mime_of(name: &str, data: &[u8]) -> &'static str {
    use crate::metadata::Format;
    match crate::metadata::format(data) {
        Some(Format::Jpeg) => return "image/jpeg",
        Some(Format::Png) => return "image/png",
        Some(Format::WebP) => return "image/webp",
        Some(Format::Gif) => return "image/gif",
        Some(Format::WebM) => return "video/webm",
        Some(Format::Mp4) => {
            return if data.get(8..12) == Some(b"qt  ") {
                "video/quicktime"
            } else {
                "video/mp4"
            };
        }
        None => {}
    }
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "avif" => "image/avif",
        "heic" => "image/heic",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        _ => "application/octet-stream",
    }
}

/// A `multipart/form-data` body: `fields` as text, then the file `data`
/// named `name`. A quote or a line break in the name would end its header
/// early, so they are written as a browser writes them.
fn multipart(
    boundary: &str,
    fields: &[(&str, &str)],
    files: &[(&str, &str, &str, &[u8])],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (k, v) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
                .as_bytes(),
        );
    }
    for (field, name, mime, data) in files {
        let shown = name
            .replace('"', "%22")
            .replace('\r', "%0D")
            .replace('\n', "%0A");
        body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{shown}\"\r\nContent-Type: {mime}\r\n\r\n"
        )
        .as_bytes(),
    );
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn media_body(
    boundary: &str,
    name: &str,
    mime: &str,
    data: &[u8],
    description: Option<&str>,
) -> Vec<u8> {
    let fields: Vec<_> = description
        .filter(|d| !d.is_empty())
        .map(|d| ("description", d))
        .into_iter()
        .collect();
    multipart(boundary, &fields, &[("file", name, mime, data)])
}

/// Same framing as upload, without allocating another copy of the file.
pub(crate) fn media_upload_size(name: &str, data: &[u8], description: Option<&str>) -> u64 {
    let framing = media_body(
        "mstdn-00000000000000000000000000000000",
        name,
        mime_of(name, data),
        &[],
        description,
    );
    framing.len() as u64 + data.len() as u64
}

pub(crate) fn retry_message(wait: Duration, host: &str) -> String {
    format!("{host} asks to wait; trying again in {} s", wait.as_secs())
}

/// Say on stderr why the command is quiet, then wait. stdout carries the
/// output, so it is not written there.
fn wait_and_say(wait: Duration, host: &str) {
    use std::io::Write;
    #[expect(
        clippy::let_underscore_must_use,
        reason = "a note on stderr while waiting; the wait and the call go on whether it showed or not"
    )]
    let _: std::io::Result<()> = writeln!(std::io::stderr(), "{}", retry_message(wait, host));
    std::thread::sleep(wait);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("mastodon.social", "https://mastodon.social")]
    #[case(" Mastodon.Social ", "https://mastodon.social")]
    #[case("@mastodon.social", "https://mastodon.social")]
    #[case("https://mastodon.social/", "https://mastodon.social")]
    #[case("HTTP://127.0.0.1:8080", "http://127.0.0.1:8080")]
    #[case("xn--r8jz45g.xn--zckzah", "https://xn--r8jz45g.xn--zckzah")]
    fn a_host_is_put_as_an_origin(#[case] typed: &str, #[case] want: &str) {
        assert_eq!(normalize_host(typed).unwrap(), want);
    }

    #[rstest]
    #[case("")]
    #[case("https://")]
    #[case("ftp://mastodon.social")]
    #[case("https://mastodon.social/api")]
    #[case("mastodon.social/@alice")]
    #[case("https://mastodon.social?x=1")]
    #[case("mas todon.social")]
    #[case("user@mastodon.social")]
    #[case("a\\b")]
    fn a_host_with_a_path_or_another_scheme_is_refused(#[case] typed: &str) {
        let e = normalize_host(typed).unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
    }

    #[test]
    fn the_authority_is_what_follows_the_scheme() {
        assert_eq!(authority("https://mastodon.social"), "mastodon.social");
        assert_eq!(authority("http://127.0.0.1:9"), "127.0.0.1:9");
    }

    #[rstest]
    #[case(None, Next::Unsaid)]
    #[case(Some(""), Next::Unsaid)]
    #[case(
        Some(r#"<https://m.test/api/v1/timelines/home?max_id=109&limit=20>; rel="next", <https://m.test/api/v1/timelines/home?min_id=120>; rel="prev""#),
        Next::MaxId("109".into())
    )]
    #[case(
        Some(r#"<https://m.test/api/v1/timelines/home?min_id=120>; rel="prev""#),
        Next::End
    )]
    #[case(
        Some(r#"<https://m.test/x?limit=2&max_id=a%2Bb>; rel=next"#),
        Next::MaxId("a+b".into())
    )]
    #[case(Some(r#"<https://m.test/x?offset=40>; rel="next""#), Next::Unsaid)]
    #[case(Some("garbage"), Next::End)]
    fn the_next_page_is_where_the_link_header_says(#[case] link: Option<&str>, #[case] want: Next) {
        assert_eq!(next_of(link), want);
    }

    #[test]
    fn an_id_cannot_leave_its_place_in_the_path() {
        assert_eq!(path_part("109"), "109");
        assert_eq!(path_part("../x?y"), "..%2Fx%3Fy");
        assert_eq!(percent_encode("read write"), "read%20write");
        assert_eq!(percent_decode("%E7%8C%AB+x%zz"), "猫 x%zz");
    }

    #[test]
    fn a_file_goes_up_as_the_type_its_bytes_say() {
        assert_eq!(mime_of("x.bin", b"\x89PNG\r\n\x1a\n...."), "image/png");
        assert_eq!(mime_of("cat.JPG", b"plain"), "application/octet-stream");
        assert_eq!(mime_of("song.mp3", b"ID3"), "audio/mpeg");
        assert_eq!(
            mime_of("a.mov", b"\0\0\0\x14ftypqt  \0\0\0\0"),
            "video/quicktime"
        );
    }

    #[test]
    fn a_reset_time_to_come_is_waited_for_and_one_gone_is_not() {
        let soon = jiff::Timestamp::now() + jiff::SignedDuration::from_secs(5);
        let d = until(&soon.to_string()).unwrap();
        assert!(
            d <= Duration::from_secs(6) && d >= Duration::from_secs(4),
            "{d:?}"
        );
        assert_eq!(until("2000-01-01T00:00:00Z"), None);
        assert_eq!(until("not a time"), None);
    }

    #[test]
    fn a_refusal_says_what_the_server_wrote() {
        let r = |e: &str, d: &str| Refusal {
            status: 400,
            body: ErrorBody {
                error: e.into(),
                error_description: d.into(),
            },
            reset: None,
        };
        assert_eq!(
            r("invalid_grant", "The code is used").said(),
            "invalid_grant: The code is used"
        );
        assert_eq!(r("Record not found", "").said(), "Record not found");
        assert_eq!(r("", "").said(), "HTTP 400");
        assert_eq!(r("same", "same").said(), "same");
    }
}
