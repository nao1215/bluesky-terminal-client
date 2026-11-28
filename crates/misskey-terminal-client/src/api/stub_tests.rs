//! The client against a stand-in server: what it sends, how it waits and
//! tries again when the server turns it away, and saving files.

use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Mutex;

/// What the stand-in answers: status, extra headers, body.
pub(crate) struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

pub(crate) fn reply(status: u16, body: &str) -> Reply {
    Reply {
        status,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    }
}

/// The handler gets the request's path and body and the number of the
/// request (from 0).
type Handler = Arc<dyn Fn(&str, &str, usize) -> Reply + Send + Sync>;

/// The path and the body of each request the server took.
pub(crate) type Calls = Arc<Mutex<Vec<(String, String)>>>;

pub(crate) fn serve(h: Handler) -> (String, Calls) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let log: Calls = Arc::default();
    let kept = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let h = Arc::clone(&h);
            let kept = Arc::clone(&kept);
            std::thread::spawn(move || {
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if r.read_line(&mut line).is_err() {
                    return;
                }
                let path = line.split(' ').nth(1).unwrap_or("").to_string();
                let mut len = 0usize;
                loop {
                    let mut hd = String::new();
                    if r.read_line(&mut hd).is_err() {
                        return;
                    }
                    if hd.trim().is_empty() {
                        break;
                    }
                    let (name, value) = hd.split_once(':').unwrap_or_default();
                    if name.eq_ignore_ascii_case("content-length") {
                        len = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; len];
                let _ = r.read_exact(&mut body);
                let body = String::from_utf8_lossy(&body).into_owned();
                let n = {
                    let mut log = kept.lock().unwrap();
                    log.push((path.clone(), body.clone()));
                    log.len() - 1
                };
                let answer = h(&path, &body, n);
                let mut head = format!(
                    "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nConnection: close\r\n",
                    answer.status
                );
                let mut declared = false;
                for (k, v) in &answer.headers {
                    declared |= k.eq_ignore_ascii_case("content-length");
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                if !declared {
                    head.push_str(&format!("Content-Length: {}\r\n", answer.body.len()));
                }
                head.push_str("\r\n");
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&answer.body);
            });
        }
    });
    (url, log)
}

/// A client of `url` whose waits are written down instead of slept.
fn client(url: &str) -> (Client, Arc<Mutex<Vec<Duration>>>) {
    let waits: Arc<Mutex<Vec<Duration>>> = Arc::default();
    let kept = Arc::clone(&waits);
    let c = Client::with_wait(url, Arc::new(move |d, _| kept.lock().unwrap().push(d)));
    (c, waits)
}

const LIMITED: &str = r#"{"error":{"message":"Rate limit exceeded. Please try again later.","code":"RATE_LIMIT_EXCEEDED","id":"d5826d14-3982-4d2e-8011-b9e9f02499ef"}}"#;

// A search turned away for being asked too often waits the time the
// server names in Retry-After and is tried again; the notes then come.
#[test]
fn a_rate_limited_call_waits_as_asked_and_is_tried_again() {
    let (url, log) = serve(Arc::new(|_, _, n| {
        if n < 2 {
            Reply {
                status: 429,
                headers: vec![("Retry-After", "7".into())],
                body: LIMITED.as_bytes().to_vec(),
            }
        } else {
            reply(200, r#"[{"id":"a1","text":"見つけた 🐈‍⬛"}]"#)
        }
    }));
    let (c, waits) = client(&url);
    let notes = c.search("猫", 10, None).unwrap();
    assert_eq!(notes[0].text.as_deref(), Some("見つけた 🐈‍⬛"));
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(7); 2]);
    assert_eq!(log.lock().unwrap().len(), 3);
}

// Without Retry-After the waits double; after three tries again the call
// fails with the server's words and what to do.
#[test]
fn a_call_still_limited_after_three_tries_again_fails_with_a_hint() {
    let (url, log) = serve(Arc::new(|_, _, _| reply(429, LIMITED)));
    let (c, waits) = client(&url);
    let e = c.search("x", 10, None).unwrap_err();
    assert_eq!(
        e.message(),
        "notes/search failed: RATE_LIMIT_EXCEEDED: Rate limit exceeded. Please try again later."
    );
    assert!(e.hint().unwrap().contains("limits how often"), "{e:?}");
    assert_eq!(*waits.lock().unwrap(), [1, 2, 4].map(Duration::from_secs));
    assert_eq!(log.lock().unwrap().len(), 4);
}

// A server that asks for an hour is waited for a minute: a command that
// hangs for an hour looks broken.
#[test]
fn a_long_retry_after_is_cut_to_a_minute() {
    let (url, _) = serve(Arc::new(|_, _, n| match n {
        0 => Reply {
            status: 429,
            headers: vec![("Retry-After", "3600".into())],
            body: Vec::new(),
        },
        _ => reply(200, "[]"),
    }));
    let (c, waits) = client(&url);
    assert!(c.search("x", 10, None).unwrap().is_empty());
    assert_eq!(*waits.lock().unwrap(), [MAX_WAIT]);
}

// A busy gateway is tried again too; an answer that is a real refusal is
// returned at once, with the server's code and message.
#[test]
fn a_busy_gateway_is_tried_again_and_a_refusal_is_not() {
    let (url, log) = serve(Arc::new(|path, _, n| match (path, n) {
        ("/api/notes/show", 0) => reply(502, "<html>Bad Gateway</html>"),
        ("/api/notes/show", _) => reply(200, r#"{"id":"n1"}"#),
        _ => reply(
            404,
            r#"{"error":{"message":"No such user.","code":"NO_SUCH_USER","id":"4362f8dc"}}"#,
        ),
    }));
    let (c, waits) = client(&url);
    assert_eq!(c.note("n1").unwrap().id, "n1");
    assert_eq!(waits.lock().unwrap().len(), 1);
    let e = c.user("nobody", None).unwrap_err();
    assert_eq!(
        e.message(),
        "users/show failed: NO_SUCH_USER: No such user."
    );
    assert_eq!(e.kind(), Kind::Api);
    assert!(e.hint().unwrap().contains("--host"));
    assert_eq!(
        log.lock().unwrap().len(),
        3,
        "the refusal is not tried again"
    );
}

// Every call is a POST of JSON to /api/<endpoint>; the account's server is
// sent as written, null for one of the server asked, and the page asks for
// what the filter says, older than untilId.
#[test]
fn what_each_call_sends() {
    let (url, log) = serve(Arc::new(|path, _, _| match path {
        "/api/users/show" => reply(200, r#"{"id":"u1","username":"alice","host":null}"#),
        _ => reply(200, "[]"),
    }));
    let (c, _) = client(&url);
    c.user("alice", None).unwrap();
    c.user("bob", Some("other.example")).unwrap();
    let filter = NoteFilter {
        replies: false,
        renotes: true,
        files: true,
    };
    c.user_notes("u1", &filter, 500, Some("9z")).unwrap();
    c.search("猫 🐈‍⬛ \"quoted\"", 0, None).unwrap();
    let log = log.lock().unwrap();
    let body = |i: usize| serde_json::from_str::<Value>(&log[i].1).unwrap();
    assert_eq!(log[0].0, "/api/users/show");
    assert_eq!(body(0), json!({"username": "alice", "host": null}));
    assert_eq!(body(1), json!({"username": "bob", "host": "other.example"}));
    assert_eq!(log[2].0, "/api/users/notes");
    assert_eq!(
        body(2),
        json!({"userId": "u1", "limit": 100, "withReplies": false, "withRenotes": true,
               "withFiles": true, "untilId": "9z"})
    );
    assert_eq!(body(3), json!({"query": "猫 🐈‍⬛ \"quoted\"", "limit": 1}));
}

#[test]
fn an_answer_that_is_not_a_list_of_notes_is_an_error() {
    let (url, _) = serve(Arc::new(|_, _, _| reply(200, r#"{"notes":[]}"#)));
    let (c, _) = client(&url);
    let e = c.search("x", 1, None).unwrap_err();
    assert_eq!(e.kind(), Kind::Api);
}

#[test]
fn a_server_that_cannot_be_reached_says_so() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let (c, waits) = client(&url);
    let e = c.note("x").unwrap_err();
    assert!(
        e.message()
            .starts_with("notes/show: cannot reach the server"),
        "{e:?}"
    );
    assert!(waits.lock().unwrap().is_empty());
}

fn png() -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend(std::iter::repeat_n(7u8, 100_000));
    b
}

// A file is saved byte for byte under its note's name, and a second save
// of it takes the next name rather than writing over the first.
#[test]
fn a_file_is_saved_whole_and_never_over_another() {
    let (url, _) = serve(Arc::new(|_, _, _| Reply {
        status: 200,
        headers: Vec::new(),
        body: png(),
    }));
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("写真");
    let f = types::DriveFile {
        url: Some(format!("{url}/files/a.png")),
        name: "猫.png".into(),
        mime: "image/png".into(),
        size: 100_008,
        ..Default::default()
    };
    let name = crate::download::file_name("9abc", 1, &f);
    let first = crate::download::save(&agent(), &f, &out, &name).unwrap();
    let second = crate::download::save(&agent(), &f, &out, &name).unwrap();
    assert_eq!(first, out.join("9abc-1.png"));
    assert_eq!(second, out.join("9abc-1 (1).png"));
    assert_eq!(std::fs::read(&first).unwrap(), png());
    assert_eq!(std::fs::read(&second).unwrap(), png());
}

// A file the server answers with an error, or says is over the limit,
// leaves nothing behind, not even its folder.
#[test]
fn a_file_that_cannot_be_downloaded_leaves_nothing() {
    let (url, _) = serve(Arc::new(|path, _, _| match path {
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
        let f = types::DriveFile {
            url: Some(format!("{url}/files/{file}")),
            mime: "image/png".into(),
            ..Default::default()
        };
        let e = crate::download::save(&agent(), &f, &out, "n-1.png").unwrap_err();
        assert!(e.message().contains(said), "{e:?}");
        assert!(!out.exists());
    }
}

/// A throwaway value in the place of a token; no server gave it.
const FAKE: &str = "fake-token-for-tests";

// A client with a token sends it as the body's `i`, beside what the call
// sends; one without sends no `i` at all.
#[test]
fn the_token_goes_in_the_body_only_when_there_is_one() {
    let (url, log) = serve(Arc::new(|_, _, _| reply(200, "[]")));
    let (c, _) = client(&url);
    c.search("猫", 5, None).unwrap();
    let c = c.with_token(Some(FAKE.into()));
    assert!(c.logged_in());
    c.search("猫", 5, None).unwrap();
    let log = log.lock().unwrap();
    let body = |i: usize| serde_json::from_str::<Value>(&log[i].1).unwrap();
    assert_eq!(body(0), json!({"query": "猫", "limit": 5}));
    assert_eq!(body(1), json!({"query": "猫", "limit": 5, "i": FAKE}));
    let (c, _) = client(&url);
    assert!(!c.with_token(Some(String::new())).logged_in());
}

// A timeline asks for what is wanted, renotes or not, older than untilId;
// notifications are asked for left unread, so reading them changes nothing.
#[test]
fn what_the_timeline_and_the_notifications_send() {
    let (url, log) = serve(Arc::new(|path, _, _| match path {
        "/api/i" => reply(200, r#"{"id":"u1","username":"me","notesCount":3}"#),
        _ => reply(200, r#"[{"id":"x1","type":"follow"}]"#),
    }));
    let (c, _) = client(&url);
    let c = c.with_token(Some(FAKE.into()));
    c.timeline("notes/hybrid-timeline", false, 500, Some("9z"))
        .unwrap();
    let got = c.notifications(0, None).unwrap();
    assert_eq!(got[0].kind, "follow");
    let me = c.me().unwrap();
    assert_eq!((me.id.as_str(), me.notes_count), ("u1", Some(3)));
    let log = log.lock().unwrap();
    let body = |i: usize| serde_json::from_str::<Value>(&log[i].1).unwrap();
    assert_eq!(log[0].0, "/api/notes/hybrid-timeline");
    assert_eq!(
        body(0),
        json!({"limit": 100, "withRenotes": false, "untilId": "9z", "i": FAKE})
    );
    assert_eq!(log[1].0, "/api/i/notifications");
    assert_eq!(body(1), json!({"limit": 1, "markAsRead": false, "i": FAKE}));
    assert_eq!(log[2].0, "/api/i");
    assert_eq!(body(2), json!({"i": FAKE}));
}

// A token the server no longer takes says how to get a new one, and the
// error does not quote it.
#[test]
fn a_token_the_server_turns_down_says_to_log_in_again() {
    let (url, _) = serve(Arc::new(|_, _, _| {
        reply(
            401,
            r#"{"error":{"message":"Authentication failed. Please ensure your token is correct.","code":"AUTHENTICATION_FAILED","id":"b0a7f5f8-dc2f-4171-b91f-de88ad238e14"}}"#,
        )
    }));
    let (c, waits) = client(&url);
    let e = c.with_token(Some(FAKE.into())).me().unwrap_err();
    assert_eq!(e.kind(), Kind::Api);
    assert!(e.hint().unwrap().contains("msky login --host"), "{e:?}");
    assert!(!format!("{e:?}").contains(FAKE));
    assert!(waits.lock().unwrap().is_empty());
}

// A check the server answers is returned as it is; a refusal is returned
// as one, to be told from a call that failed on the way.
#[test]
fn a_miauth_check_tells_a_refusal_from_an_answer() {
    let (url, log) = serve(Arc::new(|path, _, _| match path {
        "/api/miauth/s1/check" => reply(200, r#"{"ok":false}"#),
        _ => reply(
            403,
            r#"{"error":{"code":"ACCESS_DENIED","message":"denied"}}"#,
        ),
    }));
    let (c, _) = client(&url);
    assert_eq!(c.miauth_check("s1").unwrap().unwrap(), json!({"ok": false}));
    let r = c.miauth_check("s2").unwrap().unwrap_err();
    assert_eq!((r.status, r.said()), (403, "ACCESS_DENIED: denied".into()));
    assert_eq!(log.lock().unwrap()[0].1, "{}");
}
