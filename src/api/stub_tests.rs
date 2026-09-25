//! The client against a stand-in server: refreshing, looking handles up,
//! and pages with odd items.

use super::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

type Handler = Arc<dyn Fn(&str, &str) -> Option<(u16, String, u64)> + Send + Sync>;

/// A stub server: the handler answers (status, body, delay ms), or None to never answer.
/// The path and the Authorization of each request the server took.
type Calls = Arc<Mutex<Vec<(String, String)>>>;

fn serve(h: Handler) -> (String, Calls) {
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
                let (mut auth, mut len) = (String::new(), 0usize);
                loop {
                    let mut hd = String::new();
                    if r.read_line(&mut hd).is_err() {
                        return;
                    }
                    if hd.trim().is_empty() {
                        break;
                    }
                    let (name, value) = hd.split_once(':').unwrap_or_default();
                    match name.to_ascii_lowercase().as_str() {
                        "authorization" => auth = value.trim().to_string(),
                        "content-length" => len = value.trim().parse().unwrap(),
                        _ => {}
                    }
                }
                let _ = r.read_exact(&mut vec![0; len]);
                kept.lock().unwrap().push((path.clone(), auth.clone()));
                match h(&path, &auth) {
                    None => std::thread::sleep(Duration::from_secs(3600)),
                    Some((status, body, delay)) => {
                        std::thread::sleep(Duration::from_millis(delay));
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    }
                }
            });
        }
    });
    (url, log)
}

fn session(url: &str) -> Session {
    Session {
        service: url.to_string(),
        did: "did:plc:a".into(),
        handle: "a.test".into(),
        access_jwt: "old".into(),
        refresh_jwt: "r1".into(),
    }
}

// A handle mentioned many times is looked up once: each lookup holds up
// the writes behind the post.
#[test]
fn a_handle_mentioned_many_times_is_resolved_once() {
    let (url, log) = serve(Arc::new(|path, _| {
        if path.contains("resolveHandle") {
            Some((200, r#"{"did":"did:plc:x"}"#.into(), 200))
        } else {
            Some((
                200,
                r#"{"uri":"at://did:plc:a/app.bsky.feed.post/1","cid":"c"}"#.into(),
                0,
            ))
        }
    }));
    let c = Client::new(session(&url), None);
    let text = "@same.test ".repeat(20);
    let t = Instant::now();
    c.create_post(&text, None, None, &PostMedia::None, crate::i18n::Lang::En)
        .unwrap();
    let n = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(p, _)| p.contains("resolveHandle"))
        .count();
    println!(
        "20 mentions of one handle: {n} resolveHandle calls, {:?}",
        t.elapsed()
    );
    assert_eq!(n, 1, "one handle should be resolved once");
}

// Reads that find the token expired together refresh it once.
#[test]
fn loads_that_find_the_token_expired_together_refresh_it_once() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let r2 = Arc::clone(&refreshes);
    let (url, _log) = serve(Arc::new(move |path, auth| {
        if path.contains("refreshSession") {
            r2.fetch_add(1, Ordering::SeqCst);
            return Some((
                200,
                r#"{"did":"did:plc:a","handle":"a.test","accessJwt":"new","refreshJwt":"r2"}"#
                    .into(),
                300,
            ));
        }
        if auth == "Bearer old" {
            return Some((
                400,
                r#"{"error":"ExpiredToken","message":"Token has expired"}"#.into(),
                0,
            ));
        }
        Some((200, r#"{"feed":[]}"#.into(), 0))
    }));
    let c = Client::new(session(&url), None);
    let hs: Vec<_> = (0..8)
        .map(|_| {
            let c = c.clone();
            std::thread::spawn(move || c.timeline(None).map(|_| ()))
        })
        .collect();
    for h in hs {
        h.join().unwrap().unwrap();
    }
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}

/// A JWT that expired in 2023 and one that expires in 2100.
const EXPIRED_JWT: &str = "eyJhbGciOiJFUzI1NksiLCJ0eXAiOiJhdCtqd3QifQ.eyJleHAiOjE3MDAwMDAwMDB9.sig";
const FRESH_JWT: &str = "eyJhbGciOiJFUzI1NksiLCJ0eXAiOiJhdCtqd3QifQ.eyJleHAiOjQxMDI0NDQ4MDB9.sig";

// The access token saved when bsky last ran has usually expired. Sent
// anyway, it cost a round trip only to be refused before the refresh.
#[test]
fn an_access_token_past_its_expiry_is_refreshed_before_it_is_sent() {
    let (url, log) = serve(Arc::new(move |path, auth| {
        if path.contains("refreshSession") {
            return Some((
                200,
                format!(
                    r#"{{"did":"did:plc:a","handle":"a.test","accessJwt":"{FRESH_JWT}","refreshJwt":"r2"}}"#
                ),
                0,
            ));
        }
        if auth == format!("Bearer {EXPIRED_JWT}") {
            return Some((400, r#"{"error":"ExpiredToken"}"#.into(), 0));
        }
        Some((200, r#"{"feed":[]}"#.into(), 0))
    }));
    let mut s = session(&url);
    s.access_jwt = EXPIRED_JWT.into();
    let c = Client::new(s, None);
    c.timeline(None).unwrap();
    c.timeline(None).unwrap();
    let calls: Vec<(String, String)> = log.lock().unwrap().clone();
    let paths: Vec<&str> = calls
        .iter()
        .map(|(p, _)| p.split('?').next().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "/xrpc/com.atproto.server.refreshSession",
            "/xrpc/app.bsky.feed.getTimeline",
            "/xrpc/app.bsky.feed.getTimeline",
        ]
    );
    assert_eq!(calls[1].1, format!("Bearer {FRESH_JWT}"));
}

// A computer whose clock runs ahead of the server's sees every new token as
// expired already; it refreshes once, and then leaves expiry to the server
// instead of refreshing before every request.
#[test]
fn a_clock_that_runs_ahead_refreshes_once_not_before_every_request() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let r2 = Arc::clone(&refreshes);
    let (url, _log) = serve(Arc::new(move |path, _| {
        if path.contains("refreshSession") {
            let n = r2.fetch_add(1, Ordering::SeqCst);
            // Valid for the server; past its expiry by this computer's clock.
            return Some((
                200,
                format!(
                    r#"{{"did":"did:plc:a","handle":"a.test","accessJwt":"{EXPIRED_JWT}{n}","refreshJwt":"r{n}"}}"#
                ),
                0,
            ));
        }
        Some((200, r#"{"feed":[]}"#.into(), 0))
    }));
    let mut s = session(&url);
    s.access_jwt = EXPIRED_JWT.into();
    let c = Client::new(s, None);
    for _ in 0..3 {
        c.timeline(None).unwrap();
    }
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}

// An error body with `"message": null` did not read at all, so its
// ExpiredToken was lost: the token was not refreshed and the read failed.
#[test]
fn an_expired_token_is_refreshed_whatever_the_message_holds() {
    let (url, _log) = serve(Arc::new(move |path, auth| {
        if path.contains("refreshSession") {
            return Some((
                200,
                r#"{"did":"did:plc:a","handle":"a.test","accessJwt":"new","refreshJwt":"r2"}"#
                    .into(),
                0,
            ));
        }
        if auth == "Bearer old" {
            return Some((400, r#"{"error":"ExpiredToken","message":null}"#.into(), 0));
        }
        Some((200, r#"{"feed":[]}"#.into(), 0))
    }));
    let c = Client::new(session(&url), None);
    c.timeline(None).unwrap();
}

// A refresh answered with another account's tokens is not taken as this
// one's: not saved, not sent.
#[test]
fn a_refresh_answered_for_another_account_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (url, log) = serve(Arc::new(move |path, auth| {
        if path.contains("refreshSession") {
            return Some((200, r#"{"did":"did:plc:OTHER","handle":"other.test","accessJwt":"other-access","refreshJwt":"other-refresh"}"#.into(), 0));
        }
        if auth == "Bearer old" {
            return Some((400, r#"{"error":"ExpiredToken"}"#.into(), 0));
        }
        Some((
            200,
            r#"{"uri":"at://x/app.bsky.feed.like/1","cid":"c"}"#.into(),
            0,
        ))
    }));
    let accounts = crate::config::AccountStore::open(dir.path());
    accounts.save(&session(&url)).unwrap();
    let c = Client::new(session(&url), Some(accounts.store_for("did:plc:a")));
    let subject = StrongRef {
        uri: "at://did:plc:z/app.bsky.feed.post/1".into(),
        cid: "c".into(),
    };
    let r = c.like(&subject);
    let saved = accounts.find("did:plc:a").unwrap().unwrap();
    println!("like result: {:?}", r.map(|_| ()));
    println!(
        "saved for did:plc:a: did={} handle={} access={}",
        saved.did, saved.handle, saved.access_jwt
    );
    for (p, a) in log.lock().unwrap().iter() {
        println!("  {p}  {a}");
    }
    assert_eq!(
        saved.access_jwt, "old",
        "tokens of another account must not be saved as did:plc:a's"
    );
}

// One feed the server describes oddly does not fail the others.
#[test]
fn one_odd_feed_does_not_fail_the_others() {
    let body = r#"{"feeds":[{"uri":"at://a/app.bsky.feed.generator/good","displayName":"Good"},{"displayName":"no uri"}]}"#;
    let r: std::result::Result<FeedGenerators, _> = serde_json::from_str(body);
    println!(
        "{:?}",
        r.as_ref()
            .map(|f| f.feeds.len())
            .map_err(std::string::ToString::to_string)
    );
    let body = r#"{"feeds":[{"uri":"at://a/app.bsky.feed.generator/good","displayName":"Good"},{"uri":"at://b/app.bsky.feed.generator/x","displayName":null}]}"#;
    let r2: std::result::Result<FeedGenerators, _> = serde_json::from_str(body);
    println!(
        "{:?}",
        r2.as_ref()
            .map(|f| f.feeds.len())
            .map_err(std::string::ToString::to_string)
    );
    assert!(r.is_ok() && r2.is_ok());
}

/// The TUI and a `bsky` command share one session file. The command
/// refreshes (the refresh token rotates); the TUI's own refresh later still
/// sends the spent one from memory instead of the file's.
#[test]
fn tokens_another_bsky_rotated_are_taken_from_the_file() {
    let dir = tempfile::tempdir().unwrap();
    // valid access tokens: acc2, acc3; valid refresh: whichever is current.
    let current_refresh = Arc::new(Mutex::new("r1".to_string()));
    let cr = Arc::clone(&current_refresh);
    let n = Arc::new(AtomicUsize::new(1));
    let (url, log) = serve(Arc::new(move |path, auth| {
        if path.contains("refreshSession") {
            let mut cur = cr.lock().unwrap();
            if auth != format!("Bearer {}", *cur) {
                return Some((
                    400,
                    r#"{"error":"ExpiredToken","message":"Token has been revoked"}"#.into(),
                    0,
                ));
            }
            let k = n.fetch_add(1, Ordering::SeqCst) + 1;
            *cur = format!("r{k}");
            return Some((
                200,
                format!(
                    r#"{{"did":"did:plc:a","handle":"a.test","accessJwt":"acc{k}","refreshJwt":"r{k}"}}"#
                ),
                0,
            ));
        }
        if auth == "Bearer acc2" || auth == "Bearer acc3" {
            return Some((200, r#"{"feed":[]}"#.into(), 0));
        }
        Some((
            400,
            r#"{"error":"ExpiredToken","message":"Token has expired"}"#.into(),
            0,
        ))
    }));
    let accounts = crate::config::AccountStore::open(dir.path());
    accounts.save(&session(&url)).unwrap();
    let tui = Client::new(
        accounts.find("did:plc:a").unwrap().unwrap(),
        Some(accounts.store_for("did:plc:a")),
    );
    // A `bsky timeline` run in another terminal: it loads the file and refreshes.
    let cli = Client::new(
        accounts.find("did:plc:a").unwrap().unwrap(),
        Some(accounts.store_for("did:plc:a")),
    );
    cli.timeline(None).unwrap();
    let file = accounts.find("did:plc:a").unwrap().unwrap();
    println!(
        "file after the command: access={} refresh={}",
        file.access_jwt, file.refresh_jwt
    );
    // Now the TUI reads.
    let r = tui.timeline(None);
    for (p, a) in log.lock().unwrap().iter() {
        println!("  {p}  {a}");
    }
    println!(
        "tui: {:?}",
        r.as_ref().map(|_| ()).map_err(|e| e.message().to_string())
    );
    assert!(r.is_ok(), "the file held valid tokens");
}
