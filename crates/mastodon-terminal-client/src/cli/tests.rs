use super::*;
use crate::api::stub_tests::{Calls, Reply, reply, serve};
use crate::error::Kind;
use rstest::rstest;
use std::sync::{Arc, mpsc};

fn user(origin: Option<&str>, name: &str, host: Option<&str>) -> UserRef {
    UserRef {
        origin: origin.map(str::to_string),
        username: name.to_string(),
        host: host.map(str::to_string),
    }
}

#[rstest]
#[case("alice", user(None, "alice", None))]
#[case(" @alice ", user(None, "alice", None))]
#[case("@alice@Mstdn.JP", user(None, "alice", Some("mstdn.jp")))]
#[case("alice@misskey.io", user(None, "alice", Some("misskey.io")))]
#[case("@a_b-c.d", user(None, "a_b-c.d", None))]
#[case(
    "https://mastodon.social/@alice",
    user(Some("https://mastodon.social"), "alice", None)
)]
#[case(
    "https://mastodon.social/@alice@mstdn.jp/?x=1#y",
    user(Some("https://mastodon.social"), "alice", Some("mstdn.jp"))
)]
#[case(
    "https://mastodon.social/users/Gargron",
    user(Some("https://mastodon.social"), "Gargron", None)
)]
#[case(
    "http://127.0.0.1:8080/@bob",
    user(Some("http://127.0.0.1:8080"), "bob", None)
)]
#[case(
    "mastodon.social/@alice",
    user(Some("https://mastodon.social"), "alice", None)
)]
fn an_account_is_read_however_it_is_written(#[case] typed: &str, #[case] want: UserRef) {
    assert_eq!(parse_user(typed).unwrap(), want);
}

#[rstest]
#[case("")]
#[case("@")]
#[case("@@host")]
#[case("@alice@")]
#[case("@alice@host/x")]
#[case("あいう")]
#[case("alice 🐈")]
#[case("https://mastodon.social/@alice/109")]
#[case("https://mastodon.social/@alice/following")]
#[case("https://mastodon.social/")]
fn what_is_not_an_account_is_a_usage_error(#[case] typed: &str) {
    let e = parse_user(typed).unwrap_err();
    assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
}

#[rstest]
#[case("109372", None, "109372")]
#[case(" 109372 ", None, "109372")]
#[case(
    "https://mastodon.social/@Gargron/109372",
    Some("https://mastodon.social"),
    "109372"
)]
#[case(
    "https://Mastodon.Social/@bob@mstdn.jp/1093/?x=1#r",
    Some("https://mastodon.social"),
    "1093"
)]
#[case(
    "https://mastodon.social/users/Gargron/statuses/109",
    Some("https://mastodon.social"),
    "109"
)]
#[case("http://127.0.0.1:9/@a/1", Some("http://127.0.0.1:9"), "1")]
#[case("mastodon.social/@a/77", Some("https://mastodon.social"), "77")]
fn a_status_is_read_however_it_is_written(
    #[case] typed: &str,
    #[case] origin: Option<&str>,
    #[case] id: &str,
) {
    assert_eq!(
        parse_status(typed).unwrap(),
        (origin.map(str::to_string), id.to_string())
    );
}

#[rstest]
#[case("")]
#[case("../x")]
#[case("109?x")]
#[case("投稿")]
#[case("https://mastodon.social/@alice")]
#[case("https://mastodon.social/@/1")]
#[case("https://mastodon.social/@a/1/2")]
#[case("https://mastodon.social/notes/9abc")]
#[case("ftp://mastodon.social/@a/1")]
#[case("/@a/1")]
fn what_is_not_a_status_is_a_usage_error(#[case] typed: &str) {
    let e = parse_status(typed).unwrap_err();
    assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
}

fn statuses(ids: &[&str]) -> Vec<Status> {
    ids.iter()
        .map(|id| Status {
            id: id.to_string(),
            ..Default::default()
        })
        .collect()
}

fn page(ids: &[&str], next: Next) -> Page<Status> {
    Page {
        items: statuses(ids),
        next,
    }
}

// Pages are asked for until -n statuses came, each older than where the
// Link header says, else than the last status of the one before, and no
// more than the API's 40 at a time.
#[test]
fn pages_go_back_until_the_limit() {
    let mut asked = Vec::new();
    let got = pages(
        100,
        false,
        |_| true,
        |n, max_id| {
            asked.push((n, max_id.map(str::to_string)));
            let start = asked.len() * 1000;
            let items: Vec<Status> = (0..n)
                .map(|i| Status {
                    id: format!("{}", start - i),
                    ..Default::default()
                })
                .collect();
            let next = if asked.len() == 1 {
                Next::MaxId("777".into())
            } else {
                Next::Unsaid
            };
            Ok(Page { items, next })
        },
    )
    .unwrap();
    assert_eq!(got.len(), 100);
    assert_eq!(
        asked,
        [
            (40, None),
            (40, Some("777".into())),
            (20, Some("1961".into()))
        ]
    );
}

// A short page is not the end (what a visitor may not see is left out);
// an empty page is, a Link header without a next page is, and so is a
// page that brings nothing new.
#[test]
fn pages_end_on_an_empty_page_the_last_one_or_one_with_nothing_new() {
    let mut calls = 0;
    let got = pages(
        10,
        false,
        |_| true,
        |_, max_id| {
            calls += 1;
            Ok(match max_id {
                None => page(&["c", "b"], Next::Unsaid),
                Some("b") => page(&["a"], Next::Unsaid),
                _ => page(&[], Next::Unsaid),
            })
        },
    )
    .unwrap();
    assert_eq!(
        got.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["c", "b", "a"]
    );
    assert_eq!(calls, 3);
    let mut calls = 0;
    pages(
        10,
        false,
        |_| true,
        |_, _| {
            calls += 1;
            Ok(page(&["b", "a"], Next::End))
        },
    )
    .unwrap();
    assert_eq!(calls, 1, "the Link header said it was the last");
    let mut calls = 0;
    pages(
        10,
        false,
        |_| true,
        |_, _| {
            calls += 1;
            Ok(page(&["b", "a"], Next::Unsaid))
        },
    )
    .unwrap();
    assert_eq!(calls, 2, "the same page twice ends it");
    let mut calls = 0;
    pages(
        10,
        false,
        |_| true,
        |_, m| {
            calls += 1;
            Ok(match m {
                None => page(&["b"], Next::MaxId("x".into())),
                _ => page(&["c"], Next::MaxId("x".into())),
            })
        },
    )
    .unwrap();
    assert_eq!(calls, 2, "a page from a cursor seen before ends it");
}

// A filter told here leaves statuses out of each page, so every page asked
// for is a full one, until enough are kept.
#[test]
fn a_filter_told_here_reads_full_pages_until_enough_are_kept() {
    let mut asked = Vec::new();
    let got = pages(
        2,
        true,
        |s: &Status| s.id.ends_with('v'),
        |n, max_id| {
            asked.push(n);
            Ok(match max_id {
                None => page(&["9x", "8v", "7x"], Next::Unsaid),
                Some("7x") => page(&["6x", "5x"], Next::Unsaid),
                Some("5x") => page(&["4v", "3v", "2x"], Next::Unsaid),
                _ => page(&[], Next::Unsaid),
            })
        },
    )
    .unwrap();
    assert_eq!(
        got.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["8v", "4v"]
    );
    assert_eq!(asked, [40, 40, 40]);
}

#[test]
fn an_error_on_a_later_page_is_returned() {
    let e = pages(
        10,
        false,
        |_| true,
        |_, m| match m {
            None => Ok(page(&["b"], Next::Unsaid)),
            Some(_) => Err(Error::api("boom")),
        },
    )
    .unwrap_err();
    assert_eq!(e.message(), "boom");
}

fn status_of(v: Value) -> Status {
    serde_json::from_value(v).unwrap()
}

#[rstest]
#[case::with_replies(AuthorFilter::WithReplies, false, false)]
#[case::no_replies(AuthorFilter::NoReplies, true, false)]
#[case::media(AuthorFilter::Media, true, true)]
#[case::video(AuthorFilter::Video, true, true)]
#[case::threads(AuthorFilter::Threads, true, false)]
fn each_filter_asks_for_what_mastodon_can_filter(
    #[case] f: AuthorFilter,
    #[case] exclude_replies: bool,
    #[case] only_media: bool,
) {
    assert_eq!(
        f.asked(true),
        StatusFilter {
            exclude_replies,
            exclude_reblogs: false,
            only_media
        }
    );
    assert!(f.asked(false).exclude_reblogs);
}

// video keeps the posts with a video (a GIF Mastodon made one too), a
// boosted one's included; no-replies leaves out the replies to oneself
// that Mastodon keeps.
#[test]
fn video_and_no_replies_are_told_from_the_posts() {
    let v = AuthorFilter::Video;
    assert!(v.keeps(&status_of(
        json!({"id": "a", "media_attachments": [{"type": "image"}, {"type": "gifv"}]})
    )));
    assert!(!v.keeps(&status_of(
        json!({"id": "a", "media_attachments": [{"type": "image"}]})
    )));
    assert!(v.keeps(&status_of(
        json!({"id": "b", "reblog": {"id": "o", "media_attachments": [{"type": "video"}]}})
    )));
    let n = AuthorFilter::NoReplies;
    assert!(n.keeps(&status_of(json!({"id": "a"}))));
    assert!(!n.keeps(&status_of(json!({"id": "a", "in_reply_to_id": "z"}))));
    assert!(AuthorFilter::Threads.keeps(&status_of(json!({"id": "a", "in_reply_to_id": "z"}))));
}

/// A stdout that refuses every write with `kind`.
struct Refusing(io::ErrorKind);

impl Write for Refusing {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(self.0, "refused"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// A reader that stopped reading (`| head`) ends the command quietly; an
// output that cannot be written (a full disk) is an error.
#[test]
fn a_closed_pipe_ends_the_command_and_a_full_disk_fails_it() {
    let gone = &mut Refusing(io::ErrorKind::BrokenPipe);
    assert_eq!(ended_by_reader(text(gone, "x")), Ok(()));
    let full = &mut Refusing(io::ErrorKind::StorageFull);
    let e = ended_by_reader(text(full, "x")).unwrap_err();
    assert_eq!(e.kind(), Kind::Io);
    assert_eq!(e.message(), "cannot write the output: refused");
}

/// A throwaway value in the place of a token; no server gave it.
const FAKE: &str = "fake-token-for-tests";

fn ctx_of(origin: &str, dir: &std::path::Path, json: bool) -> Ctx {
    Ctx {
        origin: origin.into(),
        json,
        host_given: true,
        accounts: Ok(AccountStore::open(dir)),
        env_token: None,
    }
}

fn save(dir: &std::path::Path, service: &str, name: &str) {
    AccountStore::open(dir)
        .save(&Session {
            service: service.into(),
            user_id: format!("id-{name}"),
            username: name.into(),
            token: format!("{FAKE}-{name}"),
            scopes: vec!["read".into(), "write".into()],
        })
        .unwrap();
}

fn out_of(f: impl FnOnce(&mut Vec<u8>) -> Result<()>) -> (Result<()>, String) {
    let mut out = Vec::new();
    let r = f(&mut out);
    (r, String::from_utf8(out).unwrap())
}

#[test]
fn an_empty_query_is_refused_before_anything_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of("http://127.0.0.1:9", dir.path(), false);
    let e = search(&ctx, &mut Vec::new(), "  ", 5).unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
}

// A visitor's full-text search finds nothing on Mastodon, so it is a usage
// error before anything is asked; a hashtag is read from its timeline, by
// visitors too; with a login the text is searched, page by page by offset.
#[test]
fn search_reads_a_hashtag_for_anyone_and_text_for_an_account() {
    let (url, log) = serve(Arc::new(|call, _| match call.path() {
        "/api/v2/search" => match call.query("offset").as_deref() {
            None => reply(
                200,
                r#"{"accounts":[],"statuses":[{"id":"s2","content":"<p>猫</p>"},{"id":"s1"}]}"#,
            ),
            Some("2") => reply(200, r#"{"statuses":[{"id":"s1"},{"id":"s0"}]}"#),
            _ => reply(200, r#"{"statuses":[]}"#),
        },
        _ => reply(200, r#"[{"id":"t1","content":"<p>#rust</p>"}]"#),
    }));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), true);
    let host = api::authority(&url).to_string();
    let e = search(&ctx, &mut Vec::new(), "猫 🐈", 5).unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert_eq!(
        e.message(),
        format!("{host} searches the text of posts only for an account that is logged in")
    );
    assert!(log.lock().unwrap().is_empty());
    let (r, out) = out_of(|o| search(&ctx, o, "#rust", 1));
    r.unwrap();
    assert!(out.contains("\"id\":\"t1\""), "{out}");
    save(dir.path(), &url, "me");
    let (r, out) = out_of(|o| search(&ctx, o, "猫", 5));
    r.unwrap();
    assert_eq!(out.lines().count(), 3, "{out}");
    let log = log.lock().unwrap();
    assert_eq!(log[0].path(), "/api/v1/timelines/tag/rust");
    assert_eq!(log[0].auth, None);
    assert_eq!(log[1].query("type").as_deref(), Some("statuses"));
    assert_eq!(log[1].query("q").as_deref(), Some("猫"));
    assert_eq!(log[2].query("offset").as_deref(), Some("2"));
    assert_eq!(
        log[1].auth.as_deref(),
        Some(format!("Bearer {FAKE}-me").as_str())
    );
}

#[rstest]
#[case("#rust", Some("rust"))]
#[case(" #猫_1 ", Some("猫_1"))]
#[case("#", None)]
#[case("#a b", None)]
#[case("rust", None)]
#[case("#a/../b", None)]
fn a_hashtag_is_a_word_after_a_hash(#[case] q: &str, #[case] want: Option<&str>) {
    assert_eq!(hashtag(q), want);
}

// A token is sent only to the server it was saved for: not to another
// host, nor to the same host on another scheme.
#[test]
fn a_token_goes_only_to_the_server_that_gave_it() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), "https://a.test", "alice");
    let ctx = ctx_of("https://a.test", dir.path(), false);
    assert!(ctx.client(None).unwrap().logged_in());
    assert!(!ctx.client(Some("https://b.test")).unwrap().logged_in());
    assert!(!ctx.client(Some("http://a.test")).unwrap().logged_in());
    let none = Ctx {
        accounts: Err(Error::io("no config directory")),
        ..ctx_of("https://a.test", dir.path(), false)
    };
    assert!(!none.client(None).unwrap().logged_in());
}

// The commands of an account say so before anything is asked when there
// is none; local and public are read as a visitor, local with local=true.
#[test]
fn the_commands_of_an_account_need_one_and_the_others_do_not() {
    let (url, log) = serve(Arc::new(|_, _| reply(200, "[]")));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false);
    let host = api::authority(&url).to_string();
    for e in [
        timeline(&ctx, &mut Vec::new(), TimelineKind::Home, 5, false).unwrap_err(),
        notifications(&ctx, &mut Vec::new(), 5).unwrap_err(),
        whoami(&ctx, &mut Vec::new()).unwrap_err(),
    ] {
        assert_eq!(e.kind(), Kind::Usage);
        assert_eq!(e.message(), format!("not logged in on {host}"));
        assert_eq!(
            e.hint(),
            Some(format!("run `mstdn login --host {host}`").as_str())
        );
    }
    assert!(log.lock().unwrap().is_empty());
    timeline(&ctx, &mut Vec::new(), TimelineKind::Local, 5, true).unwrap();
    timeline(&ctx, &mut Vec::new(), TimelineKind::Public, 5, false).unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log[0].path(), "/api/v1/timelines/public");
    assert_eq!(log[0].query("local").as_deref(), Some("true"));
    assert_eq!(log[0].query("limit").as_deref(), Some("5"));
    assert_eq!(log[1].query("local"), None);
    assert_eq!(
        log[1].query("limit").as_deref(),
        Some("40"),
        "boosts left out read full pages"
    );
}

// The home timeline leaves the boosts out unless --reposts keeps them.
#[test]
fn boosts_are_kept_only_with_reposts() {
    let (url, _) = serve(Arc::new(|call, _| match call.query("max_id") {
        None => reply(
            200,
            r#"[{"id":"3","content":"<p>mine</p>"},{"id":"2","reblog":{"id":"o","content":"<p>boosted</p>"}},{"id":"1"}]"#,
        ),
        _ => reply(200, "[]"),
    }));
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), &url, "me");
    let ctx = ctx_of(&url, dir.path(), true);
    let ids = |reposts| {
        let (r, out) = out_of(|o| timeline(&ctx, o, TimelineKind::Home, 5, reposts));
        r.unwrap();
        out.lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(false), ["3", "1"]);
    assert_eq!(ids(true), ["3", "2", "1"]);
}

// author looks the account up, then pages its statuses as the filter
// asks; one of another server the server does not know is fetched with a
// search when logged in, and is a hint to log in when not.
#[test]
fn author_looks_the_account_up_and_pages_its_statuses() {
    let (url, log) = serve(Arc::new(|call, _| {
        match call.path() {
        "/api/v1/accounts/lookup" if call.query("acct").as_deref() == Some("alice") => {
            reply(200, r#"{"id":"7","username":"alice","acct":"alice"}"#)
        }
        "/api/v1/accounts/lookup" => reply(404, r#"{"error":"Record not found"}"#),
        "/api/v2/search" => reply(200, r#"{"accounts":[{"id":"9","username":"bob","acct":"bob@far.test"}]}"#),
        _ => Reply {
            status: 200,
            headers: vec![("Link", "<http://x/?max_id=5>; rel=\"prev\"".into())],
            body: r#"[{"id":"6","account":{"username":"alice","acct":"alice"},"content":"<p>hello 🐈</p>","created_at":"2026-11-20T10:00:00Z"}]"#.as_bytes().to_vec(),
        },
    }
    }));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false);
    let (r, out) = out_of(|o| author(&ctx, o, "@alice", 3, AuthorFilter::NoReplies, false, None));
    r.unwrap();
    assert!(out.contains("hello 🐈\n♡ 0  ⟳ 0  ↩ 0\n"), "{out}");
    let e = author(
        &ctx,
        &mut Vec::new(),
        "bob@far.test",
        3,
        AuthorFilter::WithReplies,
        false,
        None,
    )
    .unwrap_err();
    assert!(
        e.hint()
            .unwrap()
            .contains("does not know @bob@far.test yet; log in"),
        "{e:?}"
    );
    save(dir.path(), &url, "me");
    author(
        &ctx,
        &mut Vec::new(),
        "bob@far.test",
        3,
        AuthorFilter::WithReplies,
        true,
        None,
    )
    .unwrap();
    let log = log.lock().unwrap();
    assert_eq!(log[1].path(), "/api/v1/accounts/7/statuses");
    assert_eq!(log[1].query("exclude_replies").as_deref(), Some("true"));
    assert_eq!(log[1].query("limit").as_deref(), Some("40"));
    assert_eq!(
        log.iter()
            .filter(|c| c.path() == "/api/v1/accounts/7/statuses")
            .count(),
        1,
        "the Link header said it was the last"
    );
    let search = log.iter().find(|c| c.path() == "/api/v2/search").unwrap();
    assert_eq!(search.query("q").as_deref(), Some("@bob@far.test"));
    assert_eq!(search.query("resolve").as_deref(), Some("true"));
    let last = log.last().unwrap();
    assert_eq!(last.path(), "/api/v1/accounts/9/statuses");
    assert_eq!(last.query("exclude_reblogs").as_deref(), Some("false"));
}

// download saves each file of the post (of the boosted one, for a boost)
// as <status>-<n>.<ext>, and a post with none is a usage error.
#[test]
fn download_saves_the_files_of_a_post() {
    let (url, _) = serve(Arc::new(|call, _| match call.path() {
        "/api/v1/statuses/1" => reply(200, r#"{"id":"1","account":{"acct":"a"}}"#),
        p if p.starts_with("/files/") => Reply {
            status: 200,
            headers: vec![("Content-Type", "image/webp".into())],
            body: b"RIFF....WEBP".to_vec(),
        },
        _ => reply(404, "{}"),
    }));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), true);
    let e = download_status(&ctx, &mut Vec::new(), "1", dir.path()).unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert!(e.message().ends_with("/@a/1 has no files"), "{e:?}");
    let boosted: Status = serde_json::from_value(json!({
        "id": "2", "account": {"acct": "a"},
        "reblog": {"id": "88", "account": {"acct": "b@far.test"}, "media_attachments": [
            {"id": "m1", "type": "image", "url": format!("{url}/files/x")},
            {"id": "m2", "type": "image", "url": null, "remote_url": format!("{url}/files/y")}
        ]}
    }))
    .unwrap();
    let out_dir = dir.path().join("out");
    let (r, out) = out_of(|o| save_files(&ctx, o, &api::agent(), &boosted, &out_dir));
    r.unwrap();
    let lines: Vec<Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["postId"], "88");
    assert_eq!(lines[0]["type"], "image");
    assert!(
        crate::json_keys::missing(
            crate::json_keys::SAVED,
            &lines[0]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
        )
        .is_empty()
    );
    assert!(lines[0]["path"].as_str().unwrap().ends_with("88-1.webp"));
    assert!(lines[1]["path"].as_str().unwrap().ends_with("88-2.webp"));
    assert_eq!(
        std::fs::read(out_dir.join("88-2.webp")).unwrap(),
        b"RIFF....WEBP"
    );
}

// thread prints what the post answers, the post, and its replies.
#[test]
fn thread_prints_the_ancestors_the_post_and_the_replies() {
    let (url, _) = serve(Arc::new(|call, _| match call.path() {
        "/api/v1/statuses/5" => reply(200, r#"{"id":"5"}"#),
        "/api/v1/statuses/5/context" => reply(
            200,
            r#"{"ancestors":[{"id":"3"},{"id":"4"}],"descendants":[{"id":"6"}]}"#,
        ),
        _ => reply(404, "{}"),
    }));
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), true);
    let (r, out) = out_of(|o| thread(&ctx, o, "5"));
    r.unwrap();
    let ids: Vec<String> = out
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(ids, ["3", "4", "5", "6"]);
}

// logout takes the account in use, or the one on --host, or all of them,
// and says who; with nothing saved it says so.
#[test]
fn logout_says_who_was_logged_out() {
    let dir = tempfile::tempdir().unwrap();
    save(dir.path(), "https://a.test", "alice");
    save(dir.path(), "https://b.test", "bob");
    let run = |ctx: &Ctx, all: bool| out_of(|o| logout(ctx, o, all)).1;
    let in_use = Ctx {
        host_given: false,
        ..ctx_of("https://b.test", dir.path(), false)
    };
    assert_eq!(run(&in_use, false), "logged out @bob@b.test\n");
    let other = ctx_of("https://c.test", dir.path(), true);
    assert_eq!(run(&other, false), "{\"loggedOut\":[]}\n");
    save(dir.path(), "https://b.test", "bob");
    assert_eq!(
        run(&other, true),
        "{\"loggedOut\":[{\"host\":\"a.test\",\"id\":\"id-alice\",\"username\":\"alice\"},{\"host\":\"b.test\",\"id\":\"id-bob\",\"username\":\"bob\"}]}\n"
    );
    assert_eq!(run(&in_use, false), "not logged in\n");
}

// accounts lists the logins as msky's does, the one in use marked; --use
// makes another the one in use, and one not logged in is a usage error
// that names those that are.
#[test]
fn accounts_lists_the_logins_and_use_switches_between_them() {
    let dir = tempfile::tempdir().unwrap();
    let run = |ctx: &Ctx, use_host: Option<&str>| out_of(|o| accounts(ctx, o, use_host));
    let ctx = ctx_of("https://a.test", dir.path(), false);
    assert_eq!(run(&ctx, None).1, "not logged in\n");
    save(dir.path(), "https://a.test", "alice");
    save(dir.path(), "http://127.0.0.1:8080", "bob");
    assert_eq!(
        run(&ctx, None).1,
        "* @bob@127.0.0.1:8080  id-bob\n  @alice@a.test  id-alice\n"
    );
    let (r, out) = run(&ctx, Some("https://A.test/"));
    r.unwrap();
    assert_eq!(
        out,
        "  @bob@127.0.0.1:8080  id-bob\n* @alice@a.test  id-alice\n"
    );
    let e = run(&ctx, Some("c.test")).0.unwrap_err();
    assert_eq!(e.message(), "no account is logged in on c.test");
    assert_eq!(e.hint(), Some("logged in on: 127.0.0.1:8080, a.test"));
    let json_ctx = ctx_of("https://a.test", dir.path(), true);
    let lines: Vec<Value> = run(&json_ctx, None)
        .1
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        lines[1],
        json!({"id": "id-alice", "username": "alice", "host": "a.test", "service": "https://a.test", "current": true})
    );
}

/// A stand-in Mastodon for the login: it registers the app, trades `code`
/// for the token, and says whose it is.
fn oauth_server(token_status: u16, token_body: &'static str) -> (String, Calls) {
    serve(Arc::new(move |call, _| match call.path() {
        "/api/v1/apps" => reply(
            200,
            r#"{"id":"1","name":"mstdn","client_id":"cid","client_secret":"csecret"}"#,
        ),
        "/oauth/token" => reply(token_status, token_body),
        "/api/v1/accounts/verify_credentials" => reply(
            200,
            r#"{"id":"u1","username":"alice","acct":"alice","display_name":"アリス 🐈"}"#,
        ),
        _ => reply(404, "{}"),
    }))
}

const TOKEN: &str = r#"{"access_token":"fake-token-for-tests","token_type":"Bearer","scope":"read write","created_at":1}"#;

/// stderr for a login running on another thread: what is said reaches the
/// test as it is said.
struct Said(mpsc::Sender<String>);

impl Write for Said {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let _ = self.0.send(String::from_utf8_lossy(b).into_owned());
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The value of `name` in the query of `url`, decoded.
fn query_of(url: &str, name: &str) -> String {
    crate::api::stub_tests::Call {
        target: url.split_once("/oauth/authorize").unwrap().1.to_string(),
        ..Default::default()
    }
    .query(name)
    .unwrap()
}

// The login registers mstdn, prints the page to open, takes the code the
// browser brings back to the listener with the state it was sent with,
// trades it with the PKCE verifier, and saves the token for the server;
// the next login uses the app again.
#[test]
fn login_takes_the_code_on_the_listener_and_saves_the_token() {
    let (url, log) = oauth_server(200, TOKEN);
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false);
    for round in 0..2 {
        if round == 1 {
            // The app is used again on the port it was registered with
            // when that port is free, and registered again when it is not.
            // The port was seen still taken right after the first login on
            // a macOS runner, so wait until it is free: this checks the
            // reuse, not the runner's sockets.
            let app = crate::config::AccountStore::open(dir.path())
                .app(api::authority(&url))
                .unwrap();
            let port: u16 = app.redirect_uris[0]
                .trim_start_matches("http://127.0.0.1:")
                .trim_end_matches("/callback")
                .parse()
                .unwrap();
            let start = std::time::Instant::now();
            while std::net::TcpListener::bind(("127.0.0.1", port)).is_err() {
                assert!(
                    start.elapsed() < Duration::from_secs(30),
                    "port {port} stays taken"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let (tx, rx) = mpsc::channel();
        let browser = std::thread::spawn(move || {
            let said: String = rx
                .iter()
                .take_while(|s: &String| !s.starts_with("Waiting"))
                .collect();
            let page = said
                .lines()
                .find(|l| l.contains("/oauth/authorize?"))
                .unwrap()
                .to_string();
            let back = query_of(&page, "redirect_uri");
            let state = query_of(&page, "state");
            let port: u16 = back
                .trim_start_matches("http://127.0.0.1:")
                .trim_end_matches("/callback")
                .parse()
                .unwrap();
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(s, "GET /callback?code=c0de&state={state} HTTP/1.1\r\n\r\n").unwrap();
            let mut got = String::new();
            std::io::Read::read_to_string(&mut s, &mut got).unwrap();
            (said, page, got)
        });
        let mut out = Vec::new();
        login(
            &ctx,
            &mut out,
            &mut Said(tx),
            &mut "".as_bytes(),
            Duration::from_secs(10),
            false,
            false,
        )
        .unwrap();
        let (said, page, got) = browser.join().unwrap();
        let host = api::authority(&url);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("logged in as @alice@{host}\n")
        );
        assert!(
            said.starts_with(&format!("To log in on {host}, open this address")),
            "{said}"
        );
        assert!(page.starts_with(&format!("{url}/oauth/authorize?response_type=code&client_id=cid&redirect_uri=http%3A%2F%2F127.0.0.1%3A")), "{page}");
        assert!(
            page.contains("&scope=read%20write&state=")
                && page.ends_with("&code_challenge_method=S256")
        );
        assert!(
            got.starts_with("HTTP/1.1 200") && got.contains("close this tab"),
            "{got}"
        );
        assert!(!said.contains(FAKE) && !said.contains("csecret"));
        let log = log.lock().unwrap();
        assert_eq!(
            log.iter().filter(|c| c.path() == "/api/v1/apps").count(),
            1,
            "round {round}: the app is registered once"
        );
        let apps = log.iter().find(|c| c.path() == "/api/v1/apps").unwrap();
        let v: Value = serde_json::from_str(&apps.body).unwrap();
        assert_eq!(v["scopes"], "read write");
        assert_eq!(v["client_name"], "mstdn");
        assert!(
            v["redirect_uris"]
                .as_str()
                .unwrap()
                .ends_with("/callback\nurn:ietf:wg:oauth:2.0:oob")
        );
        let token = log.iter().rfind(|c| c.path() == "/oauth/token").unwrap();
        assert_eq!(token.auth, None);
        let form: std::collections::HashMap<String, String> = token
            .body
            .split('&')
            .map(|kv| {
                let (k, v) = kv.split_once('=').unwrap();
                (
                    k.to_string(),
                    crate::api::stub_tests::Call {
                        target: format!("?x={v}"),
                        ..Default::default()
                    }
                    .query("x")
                    .unwrap(),
                )
            })
            .collect();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "c0de");
        assert_eq!(form["client_secret"], "csecret");
        assert_eq!(form["redirect_uri"], query_of(&page, "redirect_uri"));
        assert_eq!(
            oauth_challenge(&form["code_verifier"]),
            query_of(&page, "code_challenge")
        );
        let me = log
            .iter()
            .rfind(|c| c.path() == "/api/v1/accounts/verify_credentials")
            .unwrap();
        assert_eq!(me.auth.as_deref(), Some(format!("Bearer {FAKE}").as_str()));
    }
    let saved = AccountStore::open(dir.path())
        .find(api::authority(&url))
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            saved.service.as_str(),
            saved.user_id.as_str(),
            saved.username.as_str(),
            saved.token.as_str()
        ),
        (url.as_str(), "u1", "alice", FAKE)
    );
    assert_eq!(saved.scopes, ["read", "write"]);
}

fn oauth_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier))
}

// With --oob the code is pasted; the redirect is the one that has the
// server show it, and --json says who logged in.
#[test]
fn login_with_oob_takes_the_pasted_code() {
    let (url, log) = oauth_server(200, TOKEN);
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), true);
    let mut out = Vec::new();
    let mut err = Vec::new();
    login(
        &ctx,
        &mut out,
        &mut err,
        &mut " pasted-code \n".as_bytes(),
        Duration::from_secs(1),
        true,
        false,
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v,
        json!({"id": "u1", "username": "alice", "host": api::authority(&url), "service": url})
    );
    let err = String::from_utf8(err).unwrap();
    assert!(
        err.contains("redirect_uri=urn%3Aietf%3Awg%3Aoauth%3A2.0%3Aoob&"),
        "{err}"
    );
    assert!(
        err.ends_with("Paste the code the page shows, then press Enter: "),
        "{err}"
    );
    let log = log.lock().unwrap();
    let token = log.iter().find(|c| c.path() == "/oauth/token").unwrap();
    assert!(token.body.contains("code=pasted-code&"), "{}", token.body);
    let apps: Value = serde_json::from_str(&log[0].body).unwrap();
    assert_eq!(apps["redirect_uris"], "urn:ietf:wg:oauth:2.0:oob");
}

// A code the server does not take saves nothing; an app it no longer
// knows is forgotten, so the next login registers again; nothing pasted
// is a usage error before anything is traded.
#[rstest]
#[case::code_refused(400, r#"{"error":"invalid_grant","error_description":"The provided authorization grant is invalid"}"#, "did not take the code: invalid_grant: The provided authorization grant is invalid", true)]
#[case::app_gone(
    401,
    r#"{"error":"invalid_client","error_description":"Client authentication failed"}"#,
    "no longer knows the app mstdn registered",
    false
)]
fn a_code_not_taken_saves_nothing(
    #[case] status: u16,
    #[case] body: &'static str,
    #[case] said: &str,
    #[case] app_kept: bool,
) {
    let (url, _) = oauth_server(status, body);
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_of(&url, dir.path(), false);
    let e = login(
        &ctx,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut "c\n".as_bytes(),
        Duration::from_secs(1),
        true,
        false,
    )
    .unwrap_err();
    assert_eq!(e.kind(), Kind::Usage);
    assert!(e.message().contains(said), "{e:?}");
    assert!(e.hint().unwrap().contains("nothing was saved"));
    assert!(!dir.path().join("accounts").exists());
    assert_eq!(
        AccountStore::open(dir.path())
            .app(api::authority(&url))
            .is_some(),
        app_kept
    );
    let e = login(
        &ctx,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut "\n".as_bytes(),
        Duration::from_secs(1),
        true,
        false,
    )
    .unwrap_err();
    assert_eq!(e.message(), "no code was given");
}
