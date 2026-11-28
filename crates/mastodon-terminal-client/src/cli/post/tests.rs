use super::*;
use crate::api::stub_tests::{Calls, reply, serve};
use crate::config::{AccountStore, Session};
use crate::error::Kind;
use rstest::rstest;
use std::sync::Arc;

/// A throwaway value in the place of a token; no server gave it.
const FAKE: &str = "fake-token-for-tests";

fn args(text: &str) -> PostArgs {
    PostArgs {
        text: text.into(),
        images: Vec::new(),
        alts: Vec::new(),
        video: None,
        reply: None,
        cw: None,
        sensitive: false,
        visibility: Visibility::Public,
        dry_run: false,
    }
}

#[test]
fn explicitly_disabled_media_is_unsupported_before_upload() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = args("picture");
    a.images.push(photo(dir.path()));
    let draft = draft(&a, &mut std::io::empty()).unwrap();
    let limits = Limits::from_instance(
        &json!({"configuration": {"statuses": {"max_media_attachments": 0}}}),
    );
    let error = too_much(&draft, &limits, "127.0.0.1:8080").unwrap_err();
    assert_eq!(error.kind(), Kind::Unsupported);
    assert_eq!(
        error.message(),
        "127.0.0.1:8080 does not support media attachments"
    );
}

fn ctx_of(origin: &str, dir: &std::path::Path, json: bool, scopes: &[&str]) -> Ctx {
    AccountStore::open(dir)
        .save(&Session {
            service: origin.into(),
            user_id: "u-me".into(),
            username: "me".into(),
            token: FAKE.into(),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
        })
        .unwrap();
    Ctx {
        origin: origin.into(),
        json,
        host_given: true,
        accounts: Ok(AccountStore::open(dir)),
        env_token: None,
    }
}

/// Time that goes by only when slept.
#[derive(Default)]
struct FakeClock(Vec<Duration>);

impl Clock for FakeClock {
    fn sleep(&mut self, d: Duration) {
        self.0.push(d);
    }
}

/// A stand-in Mastodon that takes posts: its limits, the account, the
/// files (processed at the first look), and the status.
fn server() -> (String, Calls) {
    serve(Arc::new(|call, _| match call.path() {
        "/api/v2/instance" => reply(
            200,
            r#"{"configuration":{"statuses":{"max_characters":30,"characters_reserved_per_url":23,"max_media_attachments":4},"media_attachments":{"description_limit":10,"image_size_limit":100000}}}"#,
        ),
        "/api/v1/accounts/verify_credentials" => {
            reply(200, r#"{"id":"u-me","username":"me","acct":"me"}"#)
        }
        "/api/v1/statuses/77" => reply(200, r#"{"id":"77","account":{"acct":"bob"}}"#),
        "/api/v2/search" => reply(
            200,
            r#"{"statuses":[{"id":"5550","account":{"acct":"far@far.test"}}]}"#,
        ),
        "/api/v2/media" => reply(202, r#"{"id":"m1","type":"image","url":null}"#),
        "/api/v1/media/m1" => reply(
            200,
            r#"{"id":"m1","type":"image","url":"https://c.test/m1.png"}"#,
        ),
        "/api/v1/statuses" => reply(
            200,
            r#"{"id":"900","uri":"https://m.test/users/me/statuses/900","account":{"acct":"me"}}"#,
        ),
        _ => reply(404, r#"{"error":"Record not found"}"#),
    }))
}

fn photo(dir: &std::path::Path) -> PathBuf {
    let p = dir.join("photo.png");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/testdata/metadata/gps.png"),
        &p,
    )
    .unwrap();
    p
}

// The picture is uploaded with its alt text and waited for until the
// server has processed it, then the status is made with it, once, with an
// Idempotency-Key; the address is printed.
#[test]
fn post_uploads_waits_for_the_files_and_makes_the_status() {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write"]);
    let mut a = args("猫 https://debimate.jp/post/a/very/long/");
    a.images.push(photo(dir.path()));
    a.alts.push("黒猫 🐈‍⬛".into());
    a.cw = Some("ネタバレ".into());
    a.visibility = Visibility::Unlisted;
    a.reply = Some("77".into());
    a.sensitive = true;
    let mut out = Vec::new();
    let mut clock = FakeClock::default();
    post(&ctx, &mut out, &a, &mut "".as_bytes(), &mut clock).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), format!("{url}/@me/900\n"));
    assert_eq!(clock.0, [Duration::from_secs(1)]);
    let log = log.lock().unwrap();
    assert!(
        log.iter()
            .all(|c| c.auth.as_deref() == Some(&format!("Bearer {FAKE}")[..]))
    );
    let made: Vec<_> = log
        .iter()
        .filter(|c| c.path() == "/api/v1/statuses")
        .collect();
    assert_eq!(made.len(), 1);
    assert!(
        made[0]
            .idempotency
            .as_deref()
            .unwrap()
            .starts_with("mstdn-")
    );
    let v: Value = serde_json::from_str(&made[0].body).unwrap();
    assert_eq!(
        v,
        json!({"status": "猫 https://debimate.jp/post/a/very/long/", "visibility": "unlisted",
               "spoiler_text": "ネタバレ", "sensitive": true, "in_reply_to_id": "77", "media_ids": ["m1"]})
    );
    let up = log.iter().find(|c| c.path() == "/api/v2/media").unwrap();
    assert!(up.body.contains("name=\"description\"\r\n\r\n黒猫 🐈‍⬛\r\n"));
    assert!(up.body.contains("Content-Type: image/png"));
}

// A reply to a post given by its address on another server is found with
// a search that has the server fetch it; --json prints the new status.
#[test]
fn a_reply_to_another_servers_post_is_fetched_first() {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), true, &["read", "write"]);
    let mut a = args("返信");
    a.reply = Some("https://far.test/@far/123".into());
    let mut out = Vec::new();
    post(
        &ctx,
        &mut out,
        &a,
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v,
        json!({"action": "post", "target": null, "id": "900", "url": format!("{url}/@me/900"), "already": false, "uri": "https://m.test/users/me/statuses/900"})
    );
    let log = log.lock().unwrap();
    let s = log.iter().find(|c| c.path() == "/api/v2/search").unwrap();
    assert_eq!(s.query("q").as_deref(), Some("https://far.test/@far/123"));
    assert_eq!(s.query("resolve").as_deref(), Some("true"));
    let made = log.iter().find(|c| c.path() == "/api/v1/statuses").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&made.body).unwrap()["in_reply_to_id"],
        "5550"
    );
}

// What the server's limits rule out is a usage error, and nothing is
// uploaded or posted: the length as Mastodon counts it (the content
// warning with it), an alt text too long, a file too large.
#[rstest]
#[case::too_long(
    "0123456789012345678901234567890",
    None,
    None,
    "the post is 31 characters; "
)]
#[case::with_cw(
    "0123456789",
    Some("01234567890123456789x"),
    None,
    "the post is 31 characters with its content warning"
)]
#[case::alt_too_long("x", None, Some("01234567890"), "is 11 characters; ")]
fn what_the_server_does_not_take_is_refused_before_anything_is_sent(
    #[case] text: &str,
    #[case] cw: Option<&str>,
    #[case] alt: Option<&str>,
    #[case] said: &str,
) {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write"]);
    let mut a = args(text);
    a.cw = cw.map(str::to_string);
    if let Some(alt) = alt {
        a.images.push(photo(dir.path()));
        a.alts.push(alt.into());
    }
    let e = post(
        &ctx,
        &mut Vec::new(),
        &a,
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert!(e.message().contains(said), "{e:?}");
    let log = log.lock().unwrap();
    assert!(log.iter().all(|c| c.method == "GET"), "nothing is written");
}

// An address counts as 23 and a mention of another server's account as
// its name, so a post Mastodon takes is not refused for its links.
#[test]
fn an_address_counts_as_the_server_says() {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write"]);
    let long = format!("@bob@far.example.com https://e.test/{}", "a".repeat(200));
    post(
        &ctx,
        &mut Vec::new(),
        &args(&long),
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap();
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|c| c.path() == "/api/v1/statuses")
    );
}

#[rstest]
#[case("hello", 5)]
#[case("家族👨\u{200d}👩\u{200d}👧 e\u{301}", 5)]
#[case("see https://e.test/a/very/long/path?x=1.", 4 + 23 + 1)]
#[case("(https://e.test/x)", 1 + 23 + 1)]
#[case("@alice@mastodon.social hi", 6 + 3)]
#[case("@alice hi", 9)]
#[case("mail me@host.test", 17)]
#[case("xhttps://e.test", 15)]
#[case("https:// alone", 14)]
#[case("a/https://e.test/x", 18)]
#[case("_@bob@far.test", 14)]
#[case("@a_b@far.test", 4)]
#[case("@bob@localhost", 14)]
#[case("http://e.test https://f.test", 23 + 1 + 23)]
fn a_post_is_counted_as_mastodon_counts_it(#[case] text: &str, #[case] want: usize) {
    assert_eq!(counted(text, 23), want);
}

// What can be told from the command line alone is a usage error before
// the login is even read.
#[rstest]
#[case::empty("  ", &[], None, None, "the post is empty")]
#[case::empty_cw("x", &[], None, Some(" "), "--cw is empty")]
#[case::too_many_pictures("x", &["a", "b", "c", "d", "e"], None, None, "at most 4 pictures")]
#[case::pictures_and_a_video("x", &["a"], Some("v"), None, "not both")]
fn what_is_wrong_on_the_command_line_is_a_usage_error(
    #[case] text: &str,
    #[case] images: &[&str],
    #[case] video: Option<&str>,
    #[case] cw: Option<&str>,
    #[case] said: &str,
) {
    let mut a = args(text);
    a.images = images.iter().map(PathBuf::from).collect();
    a.video = video.map(PathBuf::from);
    a.cw = cw.map(str::to_string);
    let ctx = Ctx {
        origin: "http://127.0.0.1:9".into(),
        json: false,
        host_given: true,
        accounts: Err(Error::io("no config directory")),
        env_token: None,
    };
    let e = post(
        &ctx,
        &mut Vec::new(),
        &a,
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert!(e.message().contains(said), "{e:?}");
}

// The text `-` is read from stdin and trimmed; a login without the write
// scope says so before anything is sent.
#[test]
fn the_text_comes_from_stdin_and_a_login_that_may_not_write_is_refused() {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read"]);
    let e = post(
        &ctx,
        &mut Vec::new(),
        &args("x"),
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert!(
        e.message()
            .contains("does not allow mstdn to post: it lacks write:statuses"),
        "{e:?}"
    );
    assert!(log.lock().unwrap().is_empty());
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write:statuses"]);
    post(
        &ctx,
        &mut Vec::new(),
        &args("-"),
        &mut "ブログ 📝\n".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap();
    let log = log.lock().unwrap();
    let made = log.iter().find(|c| c.path() == "/api/v1/statuses").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&made.body).unwrap()["status"],
        "ブログ 📝"
    );
}

// --dry-run says what would be sent and sends nothing.
#[test]
fn a_dry_run_sends_nothing() {
    let (url, log) = server();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write"]);
    let mut a = args("下書き https://e.test/a");
    a.dry_run = true;
    a.images.push(photo(dir.path()));
    a.alts.push("alt".into());
    let mut out = Vec::new();
    post(
        &ctx,
        &mut out,
        &a,
        &mut "".as_bytes(),
        &mut FakeClock::default(),
    )
    .unwrap();
    let host = api::authority(&url);
    let out = String::from_utf8(out).unwrap();
    assert!(
        out.starts_with(&format!(
            "dry run: nothing was sent to {host}\nas @me@{host}, public\nfile: "
        )),
        "{out}"
    );
    assert!(
        out.ends_with("text (27 of 30 characters):\n下書き https://e.test/a\n"),
        "{out}"
    );
    assert!(log.lock().unwrap().iter().all(|c| c.method == "GET"));
}

// A file the server never finishes processing is given up on after five
// minutes, and nothing is posted.
#[test]
fn a_file_never_processed_is_given_up_on() {
    let (url, log) = serve(Arc::new(|call, _| match call.path() {
        "/api/v2/media" => reply(202, r#"{"id":"m1","url":null}"#),
        "/api/v1/media/m1" => reply(206, r#"{"id":"m1","url":null}"#),
        "/api/v1/accounts/verify_credentials" => reply(200, r#"{"id":"u","username":"me"}"#),
        _ => reply(404, "{}"),
    }));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false, &["read", "write"]);
    let mut a = args("x");
    a.images.push(photo(dir.path()));
    let mut clock = FakeClock::default();
    let e = post(&ctx, &mut Vec::new(), &a, &mut "".as_bytes(), &mut clock).unwrap_err();
    assert!(e.message().contains("has not finished processing"), "{e:?}");
    assert!(clock.0.iter().sum::<Duration>() >= PROCESSING_TIMEOUT);
    assert!(
        !log.lock()
            .unwrap()
            .iter()
            .any(|c| c.path() == "/api/v1/statuses")
    );
}

fn local(alt: &str, size: usize, video: bool) -> Local {
    Local {
        path: PathBuf::from("f"),
        name: "f".into(),
        data: vec![0; size],
        alt: alt.into(),
        video,
    }
}

// Text/count limits are inclusive; Mastodon and GoToSocial byte ceilings are exclusive.
#[test]
fn text_and_count_limits_are_inclusive_and_byte_limits_are_exclusive() {
    let l = Limits {
        max_characters: 5,
        characters_reserved_per_url: 23,
        max_media_attachments: 2,
        description_limit: 3,
        image_size_limit: Some(10),
        video_size_limit: Some(20),
    };
    let d = |text: &str, cw: Option<&str>, files: Vec<Local>| Draft {
        text: text.into(),
        cw: cw.map(str::to_string),
        files,
        reply: None,
    };
    assert!(too_much(&d("12345", None, vec![]), &l, "h").is_ok());
    assert!(too_much(&d("123456", None, vec![]), &l, "h").is_err());
    assert!(too_much(&d("123", Some("45"), vec![]), &l, "h").is_ok());
    assert!(too_much(&d("123", Some("456"), vec![]), &l, "h").is_err());
    assert!(
        too_much(
            &d("x", None, vec![local("", 1, false), local("", 1, false)]),
            &l,
            "h"
        )
        .is_ok()
    );
    assert!(
        too_much(
            &d("x", None, (0..3).map(|_| local("", 1, false)).collect()),
            &l,
            "h"
        )
        .is_err()
    );
    assert!(too_much(&d("x", None, vec![local("abc", 9, false)]), &l, "h").is_ok());
    assert!(too_much(&d("x", None, vec![local("abcd", 1, false)]), &l, "h").is_err());
    assert!(too_much(&d("x", None, vec![local("", 10, false)]), &l, "h").is_err());
    assert!(too_much(&d("x", None, vec![local("", 19, true)]), &l, "h").is_ok());
    assert!(too_much(&d("x", None, vec![local("", 20, true)]), &l, "h").is_err());
}

// As many --alt as files is fine; one more is a usage error.
#[test]
fn there_may_be_an_alt_for_each_file_and_no_more() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = args("x");
    a.images.push(photo(dir.path()));
    a.alts.push("one".into());
    assert!(draft(&a, &mut "".as_bytes()).is_ok());
    a.alts.push("two".into());
    let e = draft(&a, &mut "".as_bytes()).err().unwrap();
    assert_eq!(e.message(), "there is more --alt than pictures and videos");
}
