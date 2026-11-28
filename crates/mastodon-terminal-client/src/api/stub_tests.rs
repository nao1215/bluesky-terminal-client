//! The client against a stand-in server: what it sends, how it pages and
//! waits and tries again when the server turns it away, and saving files.

use super::*;
use std::sync::Mutex;

#[test]
fn unsupported_reads_discovery_without_sending_the_token() {
    let (url, calls) = serve(Arc::new(|call, _| match call.path() {
        "/api/v1/timelines/public" => reply(404, r#"{"error":"Not Found"}"#),
        "/api/v2/instance" => reply(
            200,
            r#"{"version":"4.2.0 (compatible; GoToSocial 0.19.0)"}"#,
        ),
        "/.well-known/nodeinfo" => reply(
            200,
            r#"{"links":[{"rel":"http://nodeinfo.diaspora.software/ns/schema/2.0","href":"https://outside.invalid/nodeinfo/2.0"}]}"#,
        ),
        _ => reply(500, "{}"),
    }));
    let client = Client::new(&url).with_token(Some("private-test-token".into()));
    let error = client
        .timeline("v1/timelines/public", &[], 1, None)
        .unwrap_err();
    assert_eq!(error.kind(), Kind::Unsupported);
    assert!(error.message().contains(client.host()));
    assert!(error.message().contains("v1/timelines/public"));
    assert!(error.message().contains("GoToSocial 0.19.0"));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls[0].auth.is_some());
    assert!(calls[1..].iter().all(|c| c.auth.is_none()));
}

pub(crate) use terminal_kit::test_support::http::{Response as Reply, reply};

/// A request the stand-in took.
#[derive(Debug, Clone, Default)]
pub(crate) struct Call {
    pub method: String,
    /// The path with its query, as sent.
    pub target: String,
    pub auth: Option<String>,
    pub idempotency: Option<String>,
    pub content_type: Option<String>,
    pub body: String,
}

impl Call {
    pub(crate) fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }

    /// The value of `name` in the query, decoded.
    pub(crate) fn query(&self, name: &str) -> Option<String> {
        let q = self.target.split_once('?')?.1;
        q.split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| percent_decode(v))
    }
}

/// The handler gets the request and its number (from 0).
pub(crate) type Handler = Arc<dyn Fn(&Call, usize) -> Reply + Send + Sync>;

/// Each request the server took.
pub(crate) type Calls = Arc<Mutex<Vec<Call>>>;

pub(crate) fn serve(h: Handler) -> (String, Calls) {
    let log: Calls = Arc::default();
    let kept = Arc::clone(&log);
    let server = terminal_kit::test_support::http::serve_http(move |request, _| {
        let call = Call {
            method: request.method.clone(),
            target: request.target.clone(),
            auth: request.header("authorization").map(str::to_owned),
            idempotency: request.header("idempotency-key").map(str::to_owned),
            content_type: request.header("content-type").map(str::to_owned),
            body: String::from_utf8_lossy(&request.body).into_owned(),
        };
        let n = {
            let mut log = kept.lock().unwrap();
            log.push(call.clone());
            log.len() - 1
        };
        Some(h(&call, n))
    })
    .unwrap();
    (server.url().to_owned(), log)
}

/// A client of `url` whose waits are written down instead of slept.
pub(crate) fn client(url: &str) -> (Client, Arc<Mutex<Vec<Duration>>>) {
    let waits: Arc<Mutex<Vec<Duration>>> = Arc::default();
    let kept = Arc::clone(&waits);
    let c = Client::with_wait(url, Arc::new(move |d, _| kept.lock().unwrap().push(d)));
    (c, waits)
}

const LIMITED: &str = r#"{"error":"Too many requests"}"#;

// A read turned away for being asked too often waits the time the server
// names in Retry-After and is tried again; the statuses then come.
#[test]
fn a_rate_limited_read_waits_as_asked_and_is_tried_again() {
    let (url, log) = serve(Arc::new(|_, n| {
        if n < 2 {
            Reply {
                status: 429,
                headers: vec![("Retry-After", "7".into())],
                body: LIMITED.as_bytes().to_vec(),
            }
        } else {
            reply(200, r#"[{"id":"1","content":"<p>見つけた 🐈‍⬛</p>"}]"#)
        }
    }));
    let (c, waits) = client(&url);
    let page = c.timeline("v1/timelines/public", &[], 10, None).unwrap();
    assert_eq!(page.items[0].content, "<p>見つけた 🐈‍⬛</p>");
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(7); 2]);
    assert_eq!(log.lock().unwrap().len(), 3);
}

// Mastodon says when the limit is lifted in X-RateLimit-Reset: that is
// waited for, and once it has come, the limit says when in its hint.
#[test]
fn a_rate_limit_without_retry_after_waits_until_the_reset() {
    let reset = (jiff::Timestamp::now() + jiff::SignedDuration::from_secs(3)).to_string();
    let (url, log) = serve(Arc::new(move |_, _| Reply {
        status: 429,
        headers: vec![("X-RateLimit-Reset", reset.clone())],
        body: LIMITED.as_bytes().to_vec(),
    }));
    let (c, waits) = client(&url);
    let e = c.status("1").unwrap_err();
    assert_eq!(e.message(), "v1/statuses/1 failed: Too many requests");
    assert!(
        e.hint()
            .unwrap()
            .contains("limits how often it is asked (it takes calls again at"),
        "{e:?}"
    );
    let waits = waits.lock().unwrap();
    assert_eq!(waits.len(), 3);
    assert!(
        waits
            .iter()
            .all(|w| *w <= Duration::from_secs(4) && *w >= Duration::from_secs(1)),
        "{waits:?}"
    );
    assert_eq!(log.lock().unwrap().len(), 4);
}

// Without either header the waits double; after three tries again the call
// fails with the server's words and what to do.
#[test]
fn a_read_still_limited_after_three_tries_again_fails_with_a_hint() {
    let (url, log) = serve(Arc::new(|_, _| reply(429, LIMITED)));
    let (c, waits) = client(&url);
    let e = c.status("1").unwrap_err();
    assert!(e.hint().unwrap().contains("limits how often"), "{e:?}");
    assert_eq!(*waits.lock().unwrap(), [1, 2, 4].map(Duration::from_secs));
    assert_eq!(log.lock().unwrap().len(), 4);
}

// A write is never tried again: a server that dropped the answer may have
// made the status.
#[test]
fn a_write_is_sent_once_whatever_the_server_says() {
    let (url, log) = serve(Arc::new(|_, _| reply(503, "")));
    let (c, waits) = client(&url);
    let c = c.with_token(Some(FAKE.into()));
    let e = c.create_status(&json!({"status": "x"}), "k1").unwrap_err();
    assert_eq!(e.message(), "v1/statuses failed: HTTP 503");
    assert!(waits.lock().unwrap().is_empty());
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].idempotency.as_deref(), Some("k1"));
}

// A busy gateway is tried again too; a real refusal is returned at once,
// with the server's words and what to do.
#[test]
fn a_busy_gateway_is_tried_again_and_a_refusal_is_not() {
    let (url, log) = serve(Arc::new(|call, n| match (call.path(), n) {
        ("/api/v1/statuses/1", 0) => reply(502, "<html>Bad Gateway</html>"),
        ("/api/v1/statuses/1", _) => reply(200, r#"{"id":"1"}"#),
        _ => reply(404, r#"{"error":"Record not found"}"#),
    }));
    let (c, waits) = client(&url);
    assert_eq!(c.status("1").unwrap().id, "1");
    assert_eq!(waits.lock().unwrap().len(), 1);
    let e = c.lookup("nobody").unwrap_err();
    assert_eq!(e.message(), "v1/accounts/lookup failed: Record not found");
    assert_eq!(e.kind(), Kind::Api);
    assert!(e.hint().unwrap().contains("--host"));
    assert_eq!(
        log.lock().unwrap().len(),
        3,
        "the refusal is not tried again"
    );
}

// Each read is a GET with what it asks for in the query; the page asks for
// at most the API's 40, older than max_id, and the Link header names the
// next.
#[test]
fn what_each_read_sends_and_where_the_next_page_starts() {
    let (url, log) = serve(Arc::new(|call, _| match call.path() {
        "/api/v1/accounts/lookup" => reply(200, r#"{"id":"7","username":"alice","acct":"alice"}"#),
        _ => Reply {
            status: 200,
            headers: vec![(
                "Link",
                "<http://x/api/v1/accounts/7/statuses?max_id=99>; rel=\"next\"".into(),
            )],
            body: b"[{\"id\":\"100\"}]".to_vec(),
        },
    }));
    let (c, _) = client(&url);
    c.lookup("bob@other.example").unwrap();
    let filter = StatusFilter {
        exclude_replies: true,
        exclude_reblogs: false,
        only_media: true,
    };
    let page = c.account_statuses("7", &filter, 500, Some("120")).unwrap();
    assert_eq!(page.next, Next::MaxId("99".into()));
    c.search("猫 🐈‍⬛ \"quoted\" &x=1", "statuses", false, 0, 40)
        .unwrap_err();
    let log = log.lock().unwrap();
    assert!(log.iter().all(|c| c.method == "GET" && c.auth.is_none()));
    assert_eq!(log[0].query("acct").as_deref(), Some("bob@other.example"));
    assert_eq!(log[1].path(), "/api/v1/accounts/7/statuses");
    assert_eq!(
        [
            "limit",
            "exclude_replies",
            "exclude_reblogs",
            "only_media",
            "max_id"
        ]
        .map(|k| log[1].query(k)),
        [
            Some("40"),
            Some("true"),
            Some("false"),
            Some("true"),
            Some("120")
        ]
        .map(|v| v.map(String::from))
    );
    assert_eq!(log[2].query("q").as_deref(), Some("猫 🐈‍⬛ \"quoted\" &x=1"));
    assert_eq!(log[2].query("limit").as_deref(), Some("1"));
    assert_eq!(log[2].query("offset").as_deref(), Some("40"));
}

#[test]
fn an_answer_that_is_not_a_list_is_an_error() {
    let (url, _) = serve(Arc::new(|_, _| reply(200, r#"{"statuses":[]}"#)));
    let (c, _) = client(&url);
    let e = c.timeline("v1/timelines/public", &[], 1, None).unwrap_err();
    assert_eq!(e.kind(), Kind::Api);
}

#[test]
fn a_server_that_cannot_be_reached_says_so() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let (c, waits) = client(&url);
    let e = c.status("x").unwrap_err();
    assert!(
        e.message()
            .starts_with("v1/statuses/x: cannot reach the server"),
        "{e:?}"
    );
    assert!(waits.lock().unwrap().is_empty());
}

fn png() -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend(std::iter::repeat_n(7u8, 100_000));
    b
}

// A file is saved byte for byte under its status's name, with the
// extension of the type it is served as, and a second save of it takes the
// next name rather than writing over the first.
#[test]
fn a_file_is_saved_whole_and_never_over_another() {
    let (url, _) = serve(Arc::new(|_, _| Reply {
        status: 200,
        headers: vec![("Content-Type", "image/png".into())],
        body: png(),
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("写真");
    let m = types::Media {
        url: Some(format!("{url}/files/a.bin")),
        kind: "image".into(),
        ..Default::default()
    };
    let first = crate::download::save(&agent(), &m, &out, "109-1").unwrap();
    let second = crate::download::save(&agent(), &m, &out, "109-1").unwrap();
    assert_eq!(first, out.join("109-1.png"));
    assert_eq!(second, out.join("109-1 (1).png"));
    assert_eq!(std::fs::read(&first).unwrap(), png());
    assert_eq!(std::fs::read(&second).unwrap(), png());
}

// A file the server answers with an error, or says is over the limit,
// leaves nothing behind, not even its folder.
#[test]
fn a_file_that_cannot_be_downloaded_leaves_nothing() {
    let (url, _) = serve(Arc::new(|call, _| match call.path() {
        "/files/gone.png" => reply(404, "not found"),
        _ => Reply {
            status: 200,
            headers: vec![("Content-Length", (300u64 * 1024 * 1024).to_string())],
            body: Vec::new(),
        },
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    for (file, said) in [("gone.png", "HTTP 404"), ("huge.mp4", "larger than 200 MB")] {
        let m = types::Media {
            url: Some(format!("{url}/files/{file}")),
            kind: "image".into(),
            ..Default::default()
        };
        let e = crate::download::save(&agent(), &m, &out, "1-1").unwrap_err();
        assert!(e.message().contains(said), "{e:?}");
        assert!(!out.exists());
    }
}

/// A throwaway value in the place of a token; no server gave it.
const FAKE: &str = "fake-token-for-tests";

// A client with a token sends it as a Bearer header; one without sends no
// header at all.
#[test]
fn the_token_goes_in_the_header_only_when_there_is_one() {
    let (url, log) = serve(Arc::new(|_, _| {
        reply(200, r#"{"id":"1","username":"me","acct":"me"}"#)
    }));
    let (c, _) = client(&url);
    c.status("1").unwrap();
    let c = c.with_token(Some(FAKE.into()));
    assert!(c.logged_in());
    c.me().unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log[0].auth, None);
    assert_eq!(
        log[1].auth.as_deref(),
        Some(format!("Bearer {FAKE}").as_str())
    );
    assert_eq!(log[1].path(), "/api/v1/accounts/verify_credentials");
    let (c, _) = client(&url);
    assert!(!c.with_token(Some(String::new())).logged_in());
}

// A token the server no longer takes says how to get a new one, and the
// error does not quote it; a visitor asked to log in is told to.
#[test]
fn a_token_the_server_turns_down_says_to_log_in_again() {
    let (url, _) = serve(Arc::new(|_, _| {
        reply(401, r#"{"error":"The access token is invalid"}"#)
    }));
    let (c, waits) = client(&url);
    let e = c.clone().with_token(Some(FAKE.into())).me().unwrap_err();
    assert_eq!(e.kind(), Kind::Api);
    assert!(
        e.hint().unwrap().contains("no longer takes the token"),
        "{e:?}"
    );
    assert!(!format!("{e:?}").contains(FAKE));
    assert!(waits.lock().unwrap().is_empty());
    let (url, _) = serve(Arc::new(|_, _| {
        reply(
            422,
            r#"{"error":"This method requires an authenticated user"}"#,
        )
    }));
    let (c, _) = client(&url);
    let e = c
        .timeline("v1/timelines/public", &[("local", "true")], 5, None)
        .unwrap_err();
    assert!(
        e.hint()
            .unwrap()
            .contains("shows this only to accounts that are logged in"),
        "{e:?}"
    );
}

// A picture goes up as multipart with its type and alt text, its metadata
// taken off; an answer of 202 is a file still being processed.
#[test]
fn an_upload_sends_the_file_and_says_whether_it_is_ready() {
    let (url, log) = serve(Arc::new(|_, n| match n {
        0 => reply(
            200,
            r#"{"id":"m1","type":"image","url":"https://c.test/m1.png"}"#,
        ),
        _ => reply(202, r#"{"id":"m2","type":"video","url":null}"#),
    }));
    let (c, _) = client(&url);
    let c = c.with_token(Some(FAKE.into()));
    let jpg = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/testdata/metadata/gps.jpg"
    ))
    .unwrap();
    let (ready, m) = c.upload("猫 \"1\".jpg", &jpg, Some("黒猫 🐈‍⬛")).unwrap();
    assert!(ready);
    assert_eq!(m.id, "m1");
    let (ready, m) = c.upload("song.mp3", b"ID3 not much", None).unwrap();
    assert!(!ready);
    assert_eq!(m.id, "m2");
    let log = log.lock().unwrap();
    assert!(log[1].body.contains("Content-Type: audio/mpeg\r\n"));
    assert_eq!(
        log[1].body.len() as u64,
        media_upload_size("song.mp3", b"ID3 not much", None)
    );
    assert!(!log[1].body.contains("name=\"description\""));
    let first = &log[0];
    assert_eq!(first.path(), "/api/v2/media");
    assert!(
        first
            .content_type
            .as_deref()
            .unwrap()
            .starts_with("multipart/form-data; boundary=mstdn-")
    );
    assert!(
        first
            .body
            .contains("name=\"description\"\r\n\r\n黒猫 🐈‍⬛\r\n"),
        "{}",
        &first.body[..300]
    );
    assert!(
        first
            .body
            .contains("filename=\"猫 %221%22.jpg\"\r\nContent-Type: image/jpeg\r\n")
    );
    assert!(
        !first.body.contains("SecretCam"),
        "the metadata is taken off"
    );
}

#[test]
fn quote_capabilities_come_from_readable_fields_not_software_versions() {
    for (field, expected) in [(true, None), (false, Some(Kind::Unsupported))] {
        let (url, _) = serve(Arc::new(move |call, _| match call.path() {
            "/api/v1/statuses/10" => reply(
                200,
                if field {
                    r#"{"id":"10","quote_approval":{"current_user":"automatic"}}"#
                } else {
                    r#"{"id":"10"}"#
                },
            ),
            "/api/v1/accounts/verify_credentials" => reply(
                200,
                if field {
                    r#"{"id":"1","source":{"quote_policy":"public"}}"#
                } else {
                    r#"{"id":"1","source":{}}"#
                },
            ),
            "/api/v2/instance" => reply(200, r#"{"version":"99.0.0 (compatible; example)"}"#),
            _ => reply(404, "{}"),
        }));
        let client = Client::new(&url);
        for id in [Some("10"), None] {
            assert_eq!(client.require_quotes(id).err().map(|e| e.kind()), expected);
        }
    }
    for status in [401, 403, 404, 500] {
        let (url, _) = serve(Arc::new(move |_, _| reply(status, r#"{"error":"denied"}"#)));
        assert_eq!(
            Client::new(&url)
                .require_quotes(Some("10"))
                .unwrap_err()
                .kind(),
            Kind::Api
        );
    }
}
