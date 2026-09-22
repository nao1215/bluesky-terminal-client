//! A small blocking XRPC client for the calls bs makes.
//!
//! Every request goes to the account's PDS, which answers `com.atproto.*`
//! itself and proxies `app.bsky.*` to the AppView. When the access token
//! expires the client refreshes it once, persists the new tokens, and retries.

pub mod facets;
pub mod types;

use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{Session, SessionStore};
use crate::error::{Error, Kind, Result};
use types::*;

/// PDS used when neither `--service` nor `BS_SERVICE` is given.
pub const DEFAULT_SERVICE: &str = "https://bsky.social";

/// Longest post the AppView accepts, in grapheme clusters.
pub const MAX_POST_GRAPHEMES: usize = 300;

/// Largest avatar the PDS accepts, in bytes.
pub const MAX_AVATAR_BYTES: usize = 1_000_000;

const USER_AGENT: &str = concat!("bs/", env!("CARGO_PKG_VERSION"));

/// Build the HTTP agent every request uses. Non-2xx statuses are returned as
/// responses so the XRPC error body can be read.
/// How long a read that hit a busy server waits before trying once more.
const RETRY_AFTER: Duration = Duration::from_secs(1);

/// Whether `e` is a server being briefly unavailable rather than a real
/// answer.
fn is_transient(e: &Error) -> bool {
    let m = e.message();
    m.contains("failed: UpstreamFailure")
        || ["HTTP 502", "HTTP 503", "HTTP 504"]
            .iter()
            .any(|code| m.ends_with(code))
}

/// How long an upload may take.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(600);

pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// Validate and normalize a service URL: http(s) scheme, no trailing slash.
pub fn normalize_service(url: &str) -> Result<String> {
    let url = url.trim().trim_end_matches('/');
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(host) if !host.is_empty() && !host.contains(char::is_whitespace) => {
            Ok(url.to_string())
        }
        _ => Err(
            Error::new(Kind::Usage, format!("invalid service URL {url:?}"))
                .with_hint("pass an http:// or https:// URL, e.g. --service https://bsky.social"),
        ),
    }
}

/// Count grapheme clusters the way the AppView limits post length.
pub fn grapheme_len(text: &str) -> usize {
    text.graphemes(true).count()
}

/// The current time as the AT Protocol writes it.
pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn xrpc_url(service: &str, nsid: &str) -> String {
    format!("{service}/xrpc/{nsid}")
}

/// Turn a finished HTTP exchange into the decoded body or an [`Error`].
fn decode<T: DeserializeOwned>(
    nsid: &str,
    mut resp: ureq::http::Response<ureq::Body>,
) -> Result<T> {
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::api(format!("{nsid}: cannot read response: {e}")))?;
    if status.is_success() {
        // Some calls (updateSeen) answer 200 with no body at all; that is
        // "nothing to return", not a malformed answer.
        let body = if body.trim().is_empty() {
            "null"
        } else {
            &body
        };
        return serde_json::from_str(body)
            .map_err(|e| Error::api(format!("{nsid}: unexpected response: {e}")));
    }
    let err: XrpcError = serde_json::from_str(&body).unwrap_or_default();
    Err(XrpcFailure {
        status: status.as_u16(),
        error: err.error,
        message: err.message,
    }
    .into_error(nsid))
}

/// A non-2xx XRPC answer, before it becomes a user-facing [`Error`].
#[derive(Debug)]
struct XrpcFailure {
    status: u16,
    error: String,
    message: String,
}

impl XrpcFailure {
    fn into_error(self, nsid: &str) -> Error {
        let detail = match (self.error.is_empty(), self.message.is_empty()) {
            (false, false) => format!("{}: {}", self.error, self.message),
            (false, true) => self.error,
            (true, false) => self.message,
            (true, true) => format!("HTTP {}", self.status),
        };
        let err = Error::api(format!("{nsid} failed: {detail}"));
        if self.status == 401 {
            err.with_hint("log in again with an app password")
        } else {
            err
        }
    }
}

fn transport(nsid: &str, e: ureq::Error) -> Error {
    Error::api(format!("{nsid}: cannot reach the server: {e}"))
        .with_hint("check the network connection and the service URL")
}

/// Log in with an identifier (handle, DID, or email) and an app password.
pub fn login(service: &str, identifier: &str, password: &str) -> Result<Session> {
    let service = normalize_service(service)?;
    let nsid = "com.atproto.server.createSession";
    let resp = agent()
        .post(xrpc_url(&service, nsid))
        .header("Content-Type", "application/json")
        .send(json!({"identifier": identifier.trim(), "password": password}).to_string())
        .map_err(|e| transport(nsid, e))?;
    let tokens: SessionTokens = decode(nsid, resp)?;
    Ok(Session {
        service,
        did: tokens.did,
        handle: tokens.handle,
        access_jwt: tokens.access_jwt,
        refresh_jwt: tokens.refresh_jwt,
    })
}

/// Bluesky's video service, which takes uploaded videos (and animated
/// GIFs) and prepares them for playback.
pub const DEFAULT_VIDEO_SERVICE: &str = "https://video.bsky.app";

/// An authenticated client bound to one session.
pub struct Client {
    agent: ureq::Agent,
    session: Session,
    store: Option<SessionStore>,
    /// `did:web` of the PDS the account lives on, once looked up.
    pds_did: Option<String>,
}

#[derive(Deserialize)]
struct ServiceAuth {
    token: String,
}

/// `app.bsky.video.defs#jobStatus`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct JobStatus {
    job_id: String,
    state: String,
    blob: Option<Value>,
    error: Option<String>,
    message: Option<String>,
}

impl JobStatus {
    fn why(&self) -> String {
        match (&self.error, &self.message) {
            (Some(e), Some(m)) => format!("{e}: {m}"),
            (Some(x), None) | (None, Some(x)) => x.clone(),
            (None, None) => self.state.clone(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobStatusAnswer {
    job_status: JobStatus,
}

/// `app.bsky.video.getUploadLimits`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct UploadLimits {
    can_upload: bool,
    remaining_daily_videos: Option<u64>,
    remaining_daily_bytes: Option<u64>,
    error: Option<String>,
    message: Option<String>,
}

impl UploadLimits {
    /// Why a video of `len` bytes cannot be uploaded now, if it cannot.
    fn refusal(&self, len: usize) -> Option<Error> {
        let hint = match self.error.as_deref() {
            Some("unconfirmed_email") => {
                Some("confirm the account's email address in the Bluesky app (Settings, Account)")
            }
            _ => None,
        };
        let why = if !self.can_upload {
            Some(
                self.message
                    .clone()
                    .or_else(|| self.error.clone())
                    .unwrap_or_else(|| "no reason given".into()),
            )
        } else if self.remaining_daily_videos == Some(0) {
            Some("the daily number of videos has been reached".into())
        } else if self.remaining_daily_bytes.is_some_and(|b| b < len as u64) {
            Some("the video is larger than what is left of today's upload allowance".into())
        } else {
            None
        }?;
        let err = Error::api(format!(
            "Bluesky does not take videos from this account now: {why}"
        ));
        Some(match hint {
            Some(h) => err.with_hint(h),
            None => err,
        })
    }
}

/// The PDS endpoint in an account's DID document.
fn pds_endpoint(did_doc: &Value) -> Option<String> {
    did_doc
        .get("service")?
        .as_array()?
        .iter()
        .find(|s| {
            s.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.ends_with("#atproto_pds"))
        })?
        .get("serviceEndpoint")?
        .as_str()
        .map(str::to_string)
}

/// The `did:web` naming a service at `url` (a port is written `%3A`).
fn did_web(url: &str) -> String {
    let host = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default();
    format!("did:web:{}", host.replace(':', "%3A"))
}

enum Payload<'a> {
    None,
    Json(String),
    Bytes(&'a [u8], &'a str),
}

impl Client {
    /// A client for `session`; refreshed tokens are saved to `store`.
    pub fn new(session: Session, store: Option<SessionStore>) -> Self {
        Self {
            agent: agent(),
            session,
            store,
            pds_did: None,
        }
    }

    /// The `did:web` of the account's PDS: from its DID document, which
    /// names the host the account really lives on (not the entryway it
    /// logged in through), else the service logged in to.
    fn pds_did(&mut self) -> String {
        if let Some(d) = &self.pds_did {
            return d.clone();
        }
        let endpoint = self
            .get::<Value>("com.atproto.server.getSession", &[])
            .ok()
            .and_then(|s| s.get("didDoc").and_then(pds_endpoint));
        let did = did_web(endpoint.as_deref().unwrap_or(&self.session.service));
        self.pds_did = Some(did.clone());
        did
    }

    /// Whether the video service will take a video of `len` bytes now.
    fn check_upload_limits(&mut self, video_service: &str, len: usize) -> Result<()> {
        let exp = (Utc::now().timestamp() + 30 * 60).to_string();
        let lxm = "app.bsky.video.getUploadLimits";
        let auth: ServiceAuth = self.get(
            "com.atproto.server.getServiceAuth",
            &[
                ("aud", did_web(video_service).as_str()),
                ("lxm", lxm),
                ("exp", exp.as_str()),
            ],
        )?;
        let mut resp = self
            .agent
            .get(xrpc_url(video_service, lxm))
            .header("Authorization", &format!("Bearer {}", auth.token))
            .call()
            .map_err(|e| transport(lxm, e))?;
        let status = resp.status();
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        let limits: UploadLimits = serde_json::from_str(&body).unwrap_or_default();
        if let Some(e) = limits.refusal(len) {
            return Err(e);
        }
        if !status.is_success() {
            return Err(Error::api(format!(
                "{lxm} failed: HTTP {}",
                status.as_u16()
            )));
        }
        Ok(())
    }

    /// Upload a video (or an animated GIF) through the video service and
    /// wait until it is processed; returns the blob to embed. The service
    /// is authorized with a short-lived token the PDS issues for it.
    pub fn upload_video(
        &mut self,
        video_service: &str,
        bytes: &[u8],
        mime: &str,
        name: &str,
        poll_every: Duration,
    ) -> Result<Value> {
        // Asked first, as the official app does: a refusal (an unconfirmed
        // email, the daily allowance) comes with its reason, before the
        // file is sent.
        self.check_upload_limits(video_service, bytes.len())?;
        let aud = self.pds_did();
        let exp = (Utc::now().timestamp() + 30 * 60).to_string();
        let auth: ServiceAuth = self.get(
            "com.atproto.server.getServiceAuth",
            &[
                ("aud", aud.as_str()),
                ("lxm", "com.atproto.repo.uploadBlob"),
                ("exp", exp.as_str()),
            ],
        )?;
        let nsid = "app.bsky.video.uploadVideo";
        let mut resp = self
            .agent
            .post(xrpc_url(video_service, nsid))
            .config()
            .timeout_global(Some(UPLOAD_TIMEOUT))
            .build()
            .query("did", &self.session.did)
            .query("name", name)
            .header("Authorization", &format!("Bearer {}", auth.token))
            .header("Content-Type", mime)
            .send(bytes)
            .map_err(|e| transport(nsid, e))?;
        let status = resp.status();
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        // A video uploaded before is answered (409) with the job that has it.
        let mut job: JobStatus = serde_json::from_str(&body).unwrap_or_default();
        if job.job_id.is_empty() && job.blob.is_none() {
            // The video service's refusal, in its words; a 401 here is about
            // the account's standing there, not the login, so no hint to log
            // in again.
            let err: XrpcError = serde_json::from_str(&body).unwrap_or_default();
            let mut e = XrpcFailure {
                status: status.as_u16(),
                error: err.error,
                message: err.message,
            }
            .into_error(nsid);
            if status.as_u16() == 401 {
                e = Error::api(e.message().to_string());
            }
            return Err(e);
        }
        let job_id = job.job_id.clone();
        let deadline = std::time::Instant::now() + UPLOAD_TIMEOUT;
        loop {
            if let Some(blob) = job.blob.take() {
                return Ok(blob);
            }
            if job.state == "JOB_STATE_FAILED" {
                return Err(Error::api(format!(
                    "the video service could not process {name}: {}",
                    job.why()
                )));
            }
            if std::time::Instant::now() >= deadline {
                return Err(Error::api(format!(
                    "the video service did not finish {name} in time"
                )));
            }
            std::thread::sleep(poll_every);
            let nsid = "app.bsky.video.getJobStatus";
            let resp = self
                .agent
                .get(xrpc_url(video_service, nsid))
                .query("jobId", &job_id)
                .call()
                .map_err(|e| transport(nsid, e))?;
            job = decode::<JobStatusAnswer>(nsid, resp)?.job_status;
        }
    }

    /// The session in use (tokens may have been refreshed since login).
    pub fn session(&self) -> &Session {
        &self.session
    }

    fn send(
        &self,
        nsid: &str,
        query: &[(&str, &str)],
        payload: &Payload<'_>,
        token: &str,
    ) -> Result<ureq::http::Response<ureq::Body>> {
        let url = xrpc_url(&self.session.service, nsid);
        let auth = format!("Bearer {token}");
        let result = match payload {
            Payload::None => self
                .agent
                .get(&url)
                .query_pairs(query.iter().copied())
                .header("Authorization", &auth)
                .call(),
            Payload::Json(body) => self
                .agent
                .post(&url)
                .query_pairs(query.iter().copied())
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send(body.as_str()),
            // A video can take minutes to upload; other calls keep the
            // agent's short timeout.
            Payload::Bytes(bytes, mime) => self
                .agent
                .post(&url)
                .config()
                .timeout_global(Some(UPLOAD_TIMEOUT))
                .build()
                .header("Authorization", &auth)
                .header("Content-Type", *mime)
                .send(*bytes),
        };
        result.map_err(|e| transport(nsid, e))
    }

    fn call<T: DeserializeOwned>(
        &mut self,
        nsid: &str,
        query: &[(&str, &str)],
        payload: Payload<'_>,
    ) -> Result<T> {
        let resp = self.send(nsid, query, &payload, &self.session.access_jwt)?;
        match decode(nsid, resp) {
            Err(err) if err.message().contains("ExpiredToken") => {
                self.refresh()?;
                let resp = self.send(nsid, query, &payload, &self.session.access_jwt)?;
                decode(nsid, resp)
            }
            other => other,
        }
    }

    /// A read. One that fails the way a busy server fails (a gateway error,
    /// Bluesky's `UpstreamFailure`) is tried once more after a moment, as it
    /// usually works the second time; a write is never repeated.
    fn get<T: DeserializeOwned>(&mut self, nsid: &str, query: &[(&str, &str)]) -> Result<T> {
        match self.call(nsid, query, Payload::None) {
            Err(e) if is_transient(&e) => {
                std::thread::sleep(RETRY_AFTER);
                self.call(nsid, query, Payload::None)
            }
            other => other,
        }
    }

    fn post<B: Serialize, T: DeserializeOwned>(&mut self, nsid: &str, body: &B) -> Result<T> {
        let body = serde_json::to_string(body).expect("request body serializes");
        self.call(nsid, &[], Payload::Json(body))
    }

    fn refresh(&mut self) -> Result<()> {
        let nsid = "com.atproto.server.refreshSession";
        let resp = self
            .agent
            .post(xrpc_url(&self.session.service, nsid))
            .header(
                "Authorization",
                &format!("Bearer {}", self.session.refresh_jwt),
            )
            .send_empty()
            .map_err(|e| transport(nsid, e))?;
        let tokens: SessionTokens = decode(nsid, resp)
            .map_err(|e| e.with_hint("the session expired; log in again with an app password"))?;
        self.session.access_jwt = tokens.access_jwt;
        self.session.refresh_jwt = tokens.refresh_jwt;
        self.session.handle = tokens.handle;
        if let Some(store) = &self.store {
            store.save(&self.session)?;
        }
        Ok(())
    }

    /// `app.bsky.feed.getTimeline`.
    pub fn timeline(&mut self, cursor: Option<&str>) -> Result<Timeline> {
        let mut q = vec![("limit", "50")];
        if let Some(c) = cursor {
            q.push(("cursor", c));
        }
        self.get("app.bsky.feed.getTimeline", &q)
    }

    /// `app.bsky.feed.getAuthorFeed` for one actor, without replies.
    pub fn author_feed(&mut self, actor: &str, cursor: Option<&str>) -> Result<AuthorFeed> {
        let mut q = vec![
            ("actor", actor),
            ("limit", "30"),
            ("filter", "posts_no_replies"),
        ];
        q.extend(cursor.map(|c| ("cursor", c)));
        self.get("app.bsky.feed.getAuthorFeed", &q)
    }

    /// `app.bsky.feed.getPostThread`: the post, the posts above it, and its
    /// replies ten levels deep.
    pub fn post_thread(&mut self, uri: &str) -> Result<ThreadNode> {
        let r: PostThread = self.get(
            "app.bsky.feed.getPostThread",
            &[("uri", uri), ("depth", "10"), ("parentHeight", "20")],
        )?;
        Ok(r.thread)
    }

    /// `app.bsky.notification.listNotifications`.
    pub fn notifications(&mut self, cursor: Option<&str>) -> Result<Notifications> {
        let mut q = vec![("limit", "30")];
        q.extend(cursor.map(|c| ("cursor", c)));
        self.get("app.bsky.notification.listNotifications", &q)
    }

    /// `app.bsky.feed.getPosts`, in batches of the 25 the endpoint allows.
    pub fn posts(&mut self, uris: &[String]) -> Result<Vec<Post>> {
        let mut out = Vec::new();
        for chunk in uris.chunks(25) {
            let q: Vec<(&str, &str)> = chunk.iter().map(|u| ("uris", u.as_str())).collect();
            let r: Posts = self.get("app.bsky.feed.getPosts", &q)?;
            out.extend(r.posts);
        }
        Ok(out)
    }

    /// `app.bsky.notification.updateSeen`: notifications up to `seen_at`
    /// have been seen.
    pub fn update_seen(&mut self, seen_at: &str) -> Result<()> {
        let _: Value = self.post(
            "app.bsky.notification.updateSeen",
            &json!({"seenAt": seen_at}),
        )?;
        Ok(())
    }

    /// `app.bsky.actor.getProfile`.
    pub fn profile(&mut self, actor: &str) -> Result<Profile> {
        self.get("app.bsky.actor.getProfile", &[("actor", actor)])
    }

    /// `app.bsky.feed.searchPosts`.
    pub fn search_posts(&mut self, q: &str, cursor: Option<&str>) -> Result<SearchPosts> {
        let mut query = vec![("q", q), ("limit", "30")];
        query.extend(cursor.map(|c| ("cursor", c)));
        self.get("app.bsky.feed.searchPosts", &query)
    }

    /// `app.bsky.actor.searchActors`.
    pub fn search_actors(&mut self, q: &str, cursor: Option<&str>) -> Result<SearchActors> {
        let mut query = vec![("q", q), ("limit", "30")];
        query.extend(cursor.map(|c| ("cursor", c)));
        self.get("app.bsky.actor.searchActors", &query)
    }

    /// `com.atproto.identity.resolveHandle`.
    pub fn resolve_handle(&mut self, handle: &str) -> Result<String> {
        let r: ResolvedHandle =
            self.get("com.atproto.identity.resolveHandle", &[("handle", handle)])?;
        Ok(r.did)
    }

    fn create_record(&mut self, collection: &str, record: Value) -> Result<CreatedRecord> {
        let body = json!({"repo": self.session.did, "collection": collection, "record": record});
        self.post("com.atproto.repo.createRecord", &body)
    }

    fn delete_record(&mut self, uri: &str) -> Result<()> {
        let (collection, rkey) = split_record_uri(uri)?;
        let body = json!({"repo": self.session.did, "collection": collection, "rkey": rkey});
        let _: Value = self.post("com.atproto.repo.deleteRecord", &body)?;
        Ok(())
    }

    /// Publish a post, optionally as a reply, with link/mention/tag facets
    /// and uploaded pictures or a video. A post with media may have no text.
    pub fn create_post(
        &mut self,
        text: &str,
        reply: Option<&ReplyRef>,
        media: &PostMedia,
    ) -> Result<CreatedRecord> {
        let text = text.trim_end();
        if text.trim().is_empty() && matches!(media, PostMedia::None) {
            return Err(Error::new(Kind::Usage, "the post is empty"));
        }
        if let PostMedia::Images(images) = media
            && images.len() > crate::media::MAX_POST_IMAGES
        {
            return Err(Error::new(
                Kind::Usage,
                format!(
                    "a post can have at most {} images",
                    crate::media::MAX_POST_IMAGES
                ),
            ));
        }
        let len = grapheme_len(text);
        if len > MAX_POST_GRAPHEMES {
            return Err(Error::new(
                Kind::Usage,
                format!("the post is {len} characters; the limit is {MAX_POST_GRAPHEMES}"),
            ));
        }
        let mut facet_json = Vec::new();
        for span in facets::detect(text) {
            let did = match &span.target {
                // An unresolvable handle stays plain text rather than failing the post.
                facets::Target::Mention(handle) => self.resolve_handle(handle).ok(),
                _ => None,
            };
            if let Some(f) = facets::to_json(&span, did.as_deref()) {
                facet_json.push(f);
            }
        }
        let mut record = json!({
            "$type": "app.bsky.feed.post",
            "text": text,
            "createdAt": now(),
        });
        if !facet_json.is_empty() {
            record["facets"] = Value::Array(facet_json);
        }
        if let Some(reply) = reply {
            record["reply"] = serde_json::to_value(reply).expect("reply serializes");
        }
        match media {
            PostMedia::None => {}
            PostMedia::Images(images) => record["embed"] = images_embed(images),
            PostMedia::Video(v) => record["embed"] = video_embed(v),
        }
        self.create_record("app.bsky.feed.post", record)
    }

    /// Like a post; returns the like record's URI.
    pub fn like(&mut self, subject: &StrongRef) -> Result<String> {
        let record = json!({"$type": "app.bsky.feed.like", "subject": subject, "createdAt": now()});
        Ok(self.create_record("app.bsky.feed.like", record)?.uri)
    }

    /// Remove a like by its record URI.
    pub fn unlike(&mut self, like_uri: &str) -> Result<()> {
        self.delete_record(like_uri)
    }

    /// Repost a post; returns the repost record's URI.
    pub fn repost(&mut self, subject: &StrongRef) -> Result<String> {
        let record =
            json!({"$type": "app.bsky.feed.repost", "subject": subject, "createdAt": now()});
        Ok(self.create_record("app.bsky.feed.repost", record)?.uri)
    }

    /// Remove a repost by its record URI.
    pub fn unrepost(&mut self, repost_uri: &str) -> Result<()> {
        self.delete_record(repost_uri)
    }

    /// Follow an account; returns the follow record's URI.
    pub fn follow(&mut self, did: &str) -> Result<String> {
        let record = json!({"$type": "app.bsky.graph.follow", "subject": did, "createdAt": now()});
        Ok(self.create_record("app.bsky.graph.follow", record)?.uri)
    }

    /// Remove a follow by its record URI.
    pub fn unfollow(&mut self, follow_uri: &str) -> Result<()> {
        self.delete_record(follow_uri)
    }

    /// The account's own `app.bsky.actor.profile` record, or `None` when the
    /// account has never saved one.
    pub fn own_profile_record(&mut self) -> Result<Option<Record>> {
        let did = self.session.did.clone();
        let result = self.get(
            "com.atproto.repo.getRecord",
            &[
                ("repo", did.as_str()),
                ("collection", "app.bsky.actor.profile"),
                ("rkey", "self"),
            ],
        );
        match result {
            Ok(r) => Ok(Some(r)),
            Err(e) if e.message().contains("RecordNotFound") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Upload an image blob.
    pub fn upload_blob(&mut self, bytes: &[u8], mime: &str) -> Result<Value> {
        let r: UploadedBlob = self.call(
            "com.atproto.repo.uploadBlob",
            &[],
            Payload::Bytes(bytes, mime),
        )?;
        Ok(r.blob)
    }

    /// Replace the display name and description (and optionally the avatar)
    /// of `base`, the profile record the edit started from, keeping every
    /// other field. `None` means the account had no profile record yet.
    ///
    /// The write is guarded by `base`'s CID: if another client changed the
    /// profile after the editor read it, the PDS refuses the write instead of
    /// the edit silently discarding that change.
    pub fn update_profile(&mut self, base: Option<&Record>, edit: &ProfileEdit) -> Result<()> {
        let (mut value, swap) = match base {
            Some(r) => (r.value.clone(), r.cid.clone()),
            None => (json!({"$type": "app.bsky.actor.profile"}), None),
        };
        if !value.is_object() {
            value = json!({"$type": "app.bsky.actor.profile"});
        }
        set_or_remove(&mut value, "displayName", &edit.display_name);
        set_or_remove(&mut value, "description", &edit.description);
        if let Some((bytes, mime)) = &edit.avatar {
            value["avatar"] = self.upload_blob(bytes, mime)?;
        }
        let did = self.session.did.clone();
        let mut body = json!({
            "repo": did,
            "collection": "app.bsky.actor.profile",
            "rkey": "self",
            "record": value,
        });
        // Refuse to overwrite a profile another client changed meanwhile. For
        // an account that had no profile, null asks that it still has none.
        match (base.is_some(), swap) {
            (_, Some(cid)) => body["swapRecord"] = Value::String(cid),
            (false, None) => body["swapRecord"] = Value::Null,
            (true, None) => {}
        }
        let _: Value = self.post("com.atproto.repo.putRecord", &body)?;
        Ok(())
    }
}

/// The fields the profile editor changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileEdit {
    pub display_name: String,
    pub description: String,
    /// New avatar bytes and MIME type; `None` keeps the current avatar.
    pub avatar: Option<(Vec<u8>, String)>,
}

fn set_or_remove(obj: &mut Value, key: &str, text: &str) {
    let map = obj.as_object_mut().expect("profile record is an object");
    let text = text.trim();
    if text.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), Value::String(text.to_string()));
    }
}

/// Split `at://<did>/<collection>/<rkey>` into collection and rkey.
fn split_record_uri(uri: &str) -> Result<(&str, &str)> {
    let rest = uri
        .strip_prefix("at://")
        .ok_or_else(|| Error::api(format!("not an AT-URI: {uri}")))?;
    let mut parts = rest.splitn(3, '/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(c), Some(k)) if !c.is_empty() && !k.is_empty() && !k.contains('/') => {
            Ok((c, k))
        }
        _ => Err(Error::api(format!("not a record AT-URI: {uri}"))),
    }
}

/// An uploaded image to attach to a post.
#[derive(Debug, Clone)]
pub struct PostImage {
    /// The blob reference `uploadBlob` returned.
    pub blob: Value,
    pub alt: String,
    pub width: u32,
    pub height: u32,
}

/// What a post carries besides its text.
#[derive(Debug, Clone)]
pub enum PostMedia {
    None,
    Images(Vec<PostImage>),
    Video(PostVideo),
}

/// An uploaded video to attach to a post.
#[derive(Debug, Clone)]
pub struct PostVideo {
    pub blob: Value,
    pub alt: String,
    /// Width and height, when the file's header gave them.
    pub dims: Option<(u32, u32)>,
}

/// The `app.bsky.embed.video` embed: the blob, its alt text when there is
/// one, and its shape when it is known.
fn video_embed(v: &PostVideo) -> Value {
    let mut e = json!({"$type": "app.bsky.embed.video", "video": v.blob});
    if !v.alt.is_empty() {
        e["alt"] = json!(v.alt);
    }
    if let Some((w, h)) = v.dims {
        e["aspectRatio"] = json!({"width": w, "height": h});
    }
    e
}

/// The `app.bsky.embed.images` embed for `images`, each with its alt text and
/// its shape, so clients lay it out before it downloads.
fn images_embed(images: &[PostImage]) -> Value {
    let images: Vec<Value> = images
        .iter()
        .map(|i| {
            json!({
                "image": i.blob,
                "alt": i.alt,
                "aspectRatio": {"width": i.width, "height": i.height},
            })
        })
        .collect();
    json!({"$type": "app.bsky.embed.images", "images": images})
}

/// Guess an image MIME type from its first bytes.
#[cfg(test)]
pub fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xff, 0xd8, 0xff, ..] => Some("image/jpeg"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://bsky.social/", "https://bsky.social")]
    #[case(" http://127.0.0.1:8080 ", "http://127.0.0.1:8080")]
    fn service_urls_are_normalized(#[case] input: &str, #[case] want: &str) {
        assert_eq!(normalize_service(input).unwrap(), want);
    }

    #[rstest]
    #[case("bsky.social")]
    #[case("ftp://x")]
    #[case("https://")]
    #[case("https://a b")]
    fn bad_service_urls_are_usage_errors(#[case] input: &str) {
        assert_eq!(normalize_service(input).unwrap_err().kind(), Kind::Usage);
    }

    #[test]
    fn record_uri_splits_into_collection_and_rkey() {
        assert_eq!(
            split_record_uri("at://did:plc:a/app.bsky.feed.like/3k").unwrap(),
            ("app.bsky.feed.like", "3k")
        );
        assert!(split_record_uri("at://did:plc:a/app.bsky.feed.like").is_err());
        assert!(split_record_uri("https://x/y/z").is_err());
    }

    #[test]
    fn graphemes_count_emoji_as_one() {
        assert_eq!(grapheme_len("👍🏽a"), 2);
        assert_eq!(grapheme_len("日本語"), 3);
    }

    #[test]
    fn xrpc_failures_read_as_one_line() {
        let f = XrpcFailure {
            status: 401,
            error: "AuthenticationRequired".into(),
            message: "Invalid identifier or password".into(),
        };
        let e = f.into_error("com.atproto.server.createSession");
        assert_eq!(
            e.message(),
            "com.atproto.server.createSession failed: AuthenticationRequired: Invalid identifier or password"
        );
        assert!(e.to_string().contains("\nhint: log in again"), "{e}");
        let bare = XrpcFailure {
            status: 502,
            error: String::new(),
            message: String::new(),
        };
        assert!(bare.into_error("x").message().ends_with("HTTP 502"));
    }

    #[test]
    fn profile_fields_are_set_or_removed() {
        let mut v =
            json!({"$type": "app.bsky.actor.profile", "displayName": "old", "banner": {"x": 1}});
        set_or_remove(&mut v, "displayName", "  ");
        set_or_remove(&mut v, "description", " hello ");
        assert_eq!(
            v,
            json!({"$type": "app.bsky.actor.profile", "description": "hello", "banner": {"x": 1}})
        );
    }

    #[rstest]
    #[case(
        "app.bsky.feed.getTimeline failed: UpstreamFailure: Upstream Failure",
        true
    )]
    #[case("app.bsky.feed.getTimeline failed: HTTP 502", true)]
    #[case("app.bsky.feed.getTimeline failed: HTTP 504", true)]
    #[case("app.bsky.feed.getTimeline failed: InternalServerError: boom", false)]
    #[case("app.bsky.feed.getTimeline failed: HTTP 500", false)]
    #[case("app.bsky.feed.getTimeline failed: InvalidRequest: bad cursor", false)]
    fn only_a_busy_server_is_tried_again(#[case] message: &str, #[case] want: bool) {
        assert_eq!(is_transient(&Error::api(message)), want);
    }

    #[test]
    fn upload_limits_say_why_a_video_is_refused() {
        let limits = |v: Value| serde_json::from_value::<UploadLimits>(v).unwrap();
        let unconfirmed = limits(json!({
            "canUpload": false, "error": "unconfirmed_email",
            "message": "Confirm your email address to upload videos"
        }))
        .refusal(10)
        .unwrap();
        assert!(
            unconfirmed
                .message()
                .ends_with("Confirm your email address to upload videos")
        );
        assert!(
            unconfirmed
                .to_string()
                .contains("confirm the account's email")
        );
        let ok = json!({"canUpload": true, "remainingDailyVideos": 3, "remainingDailyBytes": 100});
        assert!(limits(ok.clone()).refusal(100).is_none());
        assert!(
            limits(ok)
                .refusal(101)
                .unwrap()
                .message()
                .contains("allowance")
        );
        let none_left = json!({"canUpload": true, "remainingDailyVideos": 0});
        assert!(
            limits(none_left)
                .refusal(1)
                .unwrap()
                .message()
                .contains("daily number")
        );
    }

    #[test]
    fn the_pds_is_named_by_its_did_web() {
        let doc = json!({"service": [
            {"id": "#other", "serviceEndpoint": "https://x.test"},
            {"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://morel.us-east.host.bsky.network"}
        ]});
        let endpoint = pds_endpoint(&doc).unwrap();
        assert_eq!(
            did_web(&endpoint),
            "did:web:morel.us-east.host.bsky.network"
        );
        assert_eq!(
            did_web("http://127.0.0.1:8080/"),
            "did:web:127.0.0.1%3A8080"
        );
        assert_eq!(pds_endpoint(&json!({})), None);
    }

    #[test]
    fn video_embed_leaves_out_what_is_not_known() {
        let blob = json!({"$type": "blob", "ref": {"$link": "v1"}, "mimeType": "video/mp4"});
        let v = video_embed(&PostVideo {
            blob: blob.clone(),
            alt: "a dog".into(),
            dims: Some((1080, 1920)),
        });
        assert_eq!(v["$type"], "app.bsky.embed.video");
        assert_eq!(v["video"], blob);
        assert_eq!(v["alt"], "a dog");
        assert_eq!(v["aspectRatio"], json!({"width": 1080, "height": 1920}));
        let v = video_embed(&PostVideo {
            blob,
            alt: String::new(),
            dims: None,
        });
        assert!(
            v.get("alt").is_none() && v.get("aspectRatio").is_none(),
            "{v}"
        );
    }

    #[test]
    fn images_embed_carries_alt_text_and_shape_in_order() {
        let img = |cid: &str, alt: &str| PostImage {
            blob: json!({"$type": "blob", "ref": {"$link": cid}}),
            alt: alt.into(),
            width: 4,
            height: 3,
        };
        let v = images_embed(&[img("b1", "a cat"), img("b2", "")]);
        assert_eq!(v["$type"], "app.bsky.embed.images");
        assert_eq!(v["images"][0]["image"]["ref"]["$link"], "b1");
        assert_eq!(v["images"][0]["alt"], "a cat");
        assert_eq!(
            v["images"][1]["alt"], "",
            "alt is required, even when empty"
        );
        assert_eq!(
            v["images"][1]["aspectRatio"],
            json!({"width": 4, "height": 3})
        );
    }

    #[rstest]
    #[case(b"\x89PNG\r\n".as_slice(), Some("image/png"))]
    #[case(b"\xff\xd8\xff\xe0".as_slice(), Some("image/jpeg"))]
    #[case(b"RIFF\0\0\0\0WEBPVP8 ".as_slice(), Some("image/webp"))]
    #[case(b"hello".as_slice(), None)]
    fn image_mime_is_sniffed(#[case] bytes: &[u8], #[case] want: Option<&str>) {
        assert_eq!(sniff_image_mime(bytes), want);
    }
}
