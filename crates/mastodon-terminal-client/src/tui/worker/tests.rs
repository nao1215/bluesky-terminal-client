use super::*;
use crate::api::stub_tests::{Reply, reply, serve};
use std::time::Instant;

fn wait(w: &Worker) -> (u64, Event) {
    let start = Instant::now();
    loop {
        if let Some(a) = w.try_recv() {
            return a;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "no answer");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn client(url: &str) -> Client {
    Client::new(url).with_token(Some("fake-token-for-tests".into()))
}

// A thread is what the status answers, the status, and its replies, read
// side by side.
#[test]
fn a_thread_is_its_ancestors_the_status_and_its_descendants() {
    let (url, log) = serve(Arc::new(|call, _| match call.path() {
        "/api/v1/statuses/2" => reply(200, r#"{"id":"2","content":"<p>本文 👨‍👩‍👧</p>"}"#),
        "/api/v1/statuses/2/context" => reply(
            200,
            r#"{"ancestors":[{"id":"1"}],"descendants":[{"id":"3","in_reply_to_id":"2"}]}"#,
        ),
        _ => reply(404, "{}"),
    }));
    let w = Worker::spawn(client(&url)).unwrap();
    w.send(7, Job::Thread { id: "2".into() });
    let (seq, ev) = wait(&w);
    assert_eq!(seq, 7);
    let Event::Thread { id, result } = ev else {
        panic!("{ev:?}");
    };
    assert_eq!(id, "2");
    let th = result.unwrap();
    assert_eq!(th.ancestors[0].id, "1");
    assert_eq!(th.status.id, "2");
    assert_eq!(th.descendants[0].id, "3");
    assert_eq!(log.lock().unwrap().len(), 2);
}

// A thread whose status is gone says so.
#[test]
fn a_thread_of_a_status_gone_fails() {
    let (url, _) = serve(Arc::new(|_, _| {
        reply(404, r#"{"error":"Record not found"}"#)
    }));
    let w = Worker::spawn(client(&url)).unwrap();
    w.send(1, Job::Thread { id: "9".into() });
    let (_, ev) = wait(&w);
    let Event::Thread { result, .. } = ev else {
        panic!("{ev:?}");
    };
    assert!(result.unwrap_err().message().contains("Record not found"));
}

// A favourite and a boost, and taking each back, are the four calls on
// the status, sent with the token.
#[test]
fn favourites_and_boosts_are_their_calls_on_the_status() {
    let (url, log) = serve(Arc::new(|_, _| reply(200, r#"{"id":"5"}"#)));
    let w = Worker::spawn(client(&url)).unwrap();
    let jobs = [
        Job::Favourite {
            id: "5".into(),
            on: true,
        },
        Job::Favourite {
            id: "5".into(),
            on: false,
        },
        Job::Boost {
            id: "5".into(),
            on: true,
        },
        Job::Boost {
            id: "5".into(),
            on: false,
        },
    ];
    for (i, j) in jobs.into_iter().enumerate() {
        w.send(i as u64, j);
        let (_, ev) = wait(&w);
        assert!(
            matches!(
                ev,
                Event::Favourited { result: Ok(()), .. } | Event::Boosted { result: Ok(()), .. }
            ),
            "{ev:?}"
        );
    }
    let log = log.lock().unwrap();
    let paths: Vec<&str> = log.iter().map(|c| c.path()).collect();
    assert_eq!(
        paths,
        [
            "/api/v1/statuses/5/favourite",
            "/api/v1/statuses/5/unfavourite",
            "/api/v1/statuses/5/reblog",
            "/api/v1/statuses/5/unreblog",
        ]
    );
    for c in log.iter() {
        assert_eq!(c.method, "POST");
        assert_eq!(c.auth.as_deref(), Some("Bearer fake-token-for-tests"));
    }
}

// A post goes once, with an Idempotency-Key, its warning and its reply
// only when it has them.
#[test]
fn a_post_sends_the_draft_once_with_a_key() {
    let (url, log) = serve(Arc::new(|_, _| reply(200, r#"{"id":"900"}"#)));
    let w = Worker::spawn(client(&url)).unwrap();
    for (cw, reply_to) in [("ネタバレ", Some("100")), ("  ", None)] {
        w.send(
            1,
            Job::Post(Box::new(Draft {
                quote: None,
                quote_policy: None,
                files: Vec::new(),
                text: "こんにちは 🇯🇵".into(),
                cw: cw.into(),
                visibility: Visibility::Unlisted,
                reply: reply_to.map(Into::into),
            })),
        );
        let (_, ev) = wait(&w);
        assert_eq!(
            match ev {
                Event::Posted(r) => r.unwrap().id,
                ev => panic!("{ev:?}"),
            },
            "900"
        );
    }
    let log = log.lock().unwrap();
    let writes: Vec<_> = log.iter().filter(|c| c.method == "POST").collect();
    assert_eq!(writes.len(), 2);
    let first: serde_json::Value = serde_json::from_str(&writes[0].body).unwrap();
    assert_eq!(first["status"], "こんにちは 🇯🇵");
    assert_eq!(first["spoiler_text"], "ネタバレ");
    assert_eq!(first["visibility"], "unlisted");
    assert_eq!(first["in_reply_to_id"], "100");
    let second: serde_json::Value = serde_json::from_str(&writes[1].body).unwrap();
    assert!(second.get("spoiler_text").is_none(), "{second}");
    assert!(second.get("in_reply_to_id").is_none(), "{second}");
    let keys: Vec<&str> = log
        .iter()
        .filter_map(|c| c.idempotency.as_deref())
        .collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1], "a new key for each post");
}

// A timeline page and where the next starts, as the Link header says.
#[test]
fn a_timeline_page_is_asked_for_with_its_query() {
    let (url, log) = serve(Arc::new(|_, _| Reply {
        status: 200,
        headers: vec![(
            "Link",
            "<http://x.test/api/v1/timelines/public?max_id=8>; rel=\"next\"".into(),
        )],
        body: br#"[{"id":"9"}]"#.to_vec(),
    }));
    let w = Worker::spawn(client(&url)).unwrap();
    w.send(
        1,
        Job::Timeline {
            kind: TimelineKind::Local,
            max_id: Some("10".into()),
        },
    );
    let (_, ev) = wait(&w);
    let Event::Timeline { result, .. } = ev else {
        panic!("{ev:?}");
    };
    let page = result.unwrap();
    assert_eq!(next_of(&page, |s: &Status| &s.id).as_deref(), Some("8"));
    let log = log.lock().unwrap();
    assert_eq!(log[0].path(), "/api/v1/timelines/public");
    assert_eq!(log[0].query("local").as_deref(), Some("true"));
    assert_eq!(log[0].query("max_id").as_deref(), Some("10"));
    assert_eq!(log[0].query("limit").as_deref(), Some("30"));
}

// q waits for a write on its way, and only as long as it takes.
#[test]
fn a_write_on_its_way_is_finished_before_quitting() {
    let (url, log) = serve(Arc::new(|_, _| {
        std::thread::sleep(Duration::from_millis(300));
        reply(200, r#"{"id":"5"}"#)
    }));
    let w = Worker::spawn(client(&url)).unwrap();
    let start = Instant::now();
    w.finish_writes(Duration::from_secs(3));
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "nothing to wait for"
    );
    w.send(
        1,
        Job::Favourite {
            id: "5".into(),
            on: true,
        },
    );
    // A read is not waited for.
    w.send(2, Job::Thread { id: "5".into() });
    w.finish_writes(Duration::from_secs(3));
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|c| c.path() == "/api/v1/statuses/5/favourite"),
        "the favourite reached the server"
    );
}

// Only writes are waited for when quitting.
#[test]
fn only_writes_change_the_server() {
    assert!(
        Job::Post(Box::new(Draft {
            quote: None,
            quote_policy: None,
            files: Vec::new(),
            text: String::new(),
            cw: String::new(),
            visibility: Visibility::Public,
            reply: None,
        }))
        .changes_server()
    );
    assert!(
        Job::Boost {
            id: "1".into(),
            on: false
        }
        .changes_server()
    );
    assert!(!Job::Thread { id: "1".into() }.changes_server());
    assert!(!Job::Limits.changes_server());
    assert!(!Job::Identity.changes_server());
    assert!(!Job::Relationship { id: "2".into() }.changes_server());
    for action in [
        RelationAction::Follow,
        RelationAction::Mute,
        RelationAction::Block,
    ] {
        for on in [false, true] {
            assert!(
                Job::Relate {
                    id: "2".into(),
                    action,
                    on
                }
                .changes_server()
            );
        }
    }
    assert!(Job::Delete { id: "1".into() }.changes_server());
    assert!(
        Job::Report {
            account: "2".into(),
            status: None,
            comment: String::new()
        }
        .changes_server()
    );
    assert!(!Job::OpenLink { url: String::new() }.changes_server());
}

// A job whose work panics is answered with its own failure.
#[test]
fn a_job_that_panics_is_answered_with_its_failure() {
    let ev = guarded(Job::Thread { id: "1".into() }, |_| panic!("boom"));
    let Event::Thread { result, .. } = ev else {
        panic!("{ev:?}");
    };
    assert_eq!(result.unwrap_err().message(), "internal error: boom");
}
