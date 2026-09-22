//! A small blocking XRPC client for the calls bs makes.
//!
//! Every request goes to the account's PDS, which answers `com.atproto.*`
//! itself and proxies `app.bsky.*` to the AppView. When the access token
//! expires the client refreshes it once, persists the new tokens, and retries.

pub mod facets;
pub mod types;

use std::time::Duration;

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;
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

fn now() -> String {
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
        return serde_json::from_str(&body)
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

/// An authenticated client bound to one session.
pub struct Client {
    agent: ureq::Agent,
    session: Session,
    store: Option<SessionStore>,
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
            Payload::Bytes(bytes, mime) => self
                .agent
                .post(&url)
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

    fn get<T: DeserializeOwned>(&mut self, nsid: &str, query: &[(&str, &str)]) -> Result<T> {
        self.call(nsid, query, Payload::None)
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
    pub fn author_feed(&mut self, actor: &str) -> Result<AuthorFeed> {
        self.get(
            "app.bsky.feed.getAuthorFeed",
            &[
                ("actor", actor),
                ("limit", "30"),
                ("filter", "posts_no_replies"),
            ],
        )
    }

    /// `app.bsky.actor.getProfile`.
    pub fn profile(&mut self, actor: &str) -> Result<Profile> {
        self.get("app.bsky.actor.getProfile", &[("actor", actor)])
    }

    /// `app.bsky.feed.searchPosts`.
    pub fn search_posts(&mut self, q: &str) -> Result<SearchPosts> {
        self.get("app.bsky.feed.searchPosts", &[("q", q), ("limit", "30")])
    }

    /// `app.bsky.actor.searchActors`.
    pub fn search_actors(&mut self, q: &str) -> Result<SearchActors> {
        self.get("app.bsky.actor.searchActors", &[("q", q), ("limit", "30")])
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

    /// Publish a post, optionally as a reply, with link/mention/tag facets.
    pub fn create_post(&mut self, text: &str, reply: Option<&ReplyRef>) -> Result<CreatedRecord> {
        let text = text.trim_end();
        if text.trim().is_empty() {
            return Err(Error::new(Kind::Usage, "the post is empty"));
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

/// Guess an image MIME type from its first bytes.
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
    #[case(b"\x89PNG\r\n".as_slice(), Some("image/png"))]
    #[case(b"\xff\xd8\xff\xe0".as_slice(), Some("image/jpeg"))]
    #[case(b"RIFF\0\0\0\0WEBPVP8 ".as_slice(), Some("image/webp"))]
    #[case(b"hello".as_slice(), None)]
    fn image_mime_is_sniffed(#[case] bytes: &[u8], #[case] want: Option<&str>) {
        assert_eq!(sniff_image_mime(bytes), want);
    }
}
