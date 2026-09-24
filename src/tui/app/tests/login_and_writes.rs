use super::*;

#[test]
fn without_a_session_the_login_form_submits_all_fields() {
    let (mut app, jobs) = App::new(None, "https://bsky.social");
    assert!(jobs.is_empty());
    type_str(&mut app, "alice.test");
    app.handle_key(code(KeyCode::Enter));
    type_str(&mut app, "pw-1234");
    let jobs = app.handle_key(code(KeyCode::Enter));
    match &jobs[..] {
        [
            Job::Login {
                service,
                identifier,
                password,
            },
        ] => {
            assert_eq!(
                (service.as_str(), identifier.as_str(), password.as_str()),
                ("https://bsky.social", "alice.test", "pw-1234")
            );
        }
        other => panic!("{other:?}"),
    }
    let jobs = app.handle_event(Event::LoggedIn(Ok(session())));
    assert!(app.login.is_none());
    assert!(matches!(jobs[..], [Job::Timeline, Job::Notifications]));
}

#[test]
fn login_with_an_empty_field_does_not_send() {
    let (mut app, _) = App::new(None, "https://bsky.social");
    app.handle_key(code(KeyCode::Enter));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(jobs.is_empty());
    assert_eq!(app.login.as_ref().unwrap().focus, 1);
    assert!(app.login.as_ref().unwrap().error.is_some());
}

#[test]
fn failed_login_shows_the_server_message_and_allows_retry() {
    let (mut app, _) = App::new(None, "https://bsky.social");
    app.login.as_mut().unwrap().pending = true;
    app.pending = 1;
    app.handle_event(Event::LoggedIn(Err(Error::api("bad password"))));
    let form = app.login.as_ref().unwrap();
    assert_eq!(form.error.as_deref(), Some("bad password"));
    assert!(!form.pending);
}

#[test]
fn like_toggles_between_like_and_unlike() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('l'));
    let Job::Like { subject } = &jobs[0] else {
        panic!("{jobs:?}")
    };
    assert_eq!(subject.uri, "at://a/p/1");
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
    });
    assert_eq!(app.timeline.items[0].like_count, 3);
    let jobs = app.handle_key(key('l'));
    assert!(matches!(&jobs[0], Job::Unlike { like_uri, .. } if like_uri.ends_with("/x")));
    app.handle_event(Event::Unliked {
        post_uri: "at://a/p/1".into(),
        result: Ok(()),
    });
    assert_eq!(app.timeline.items[0].like_count, 2);
    assert!(app.timeline.items[0].like_uri().is_none());
}

// A reload sent before a like, answered after it, holds the post as it
// was: shown as is, the like would look undone and the next l would like
// the post a second time.
#[test]
fn a_reload_sent_before_a_like_does_not_undo_it() {
    let mut app = logged_in();
    let reload = press(&mut app, key('R'))[0];
    let like = press(&mut app, key('l'))[0];
    app.handle_answer(
        like,
        Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
        },
    );
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![reloaded(false, false)].into())),
    );
    let p = &app.timeline.items[0];
    assert_eq!(p.like_uri(), Some("at://did:plc:me/app.bsky.feed.like/x"));
    assert_eq!(p.like_count, 3);
    let jobs = app.handle_key(key('l'));
    assert!(matches!(&jobs[..], [Job::Unlike { .. }]), "{jobs:?}");
}

// The same for a repost: the reload's copy of the post, from before it,
// is shown reposted, so the next b removes the repost rather than
// reposting a second time.
#[test]
fn a_reload_sent_before_a_repost_does_not_undo_it() {
    let mut app = logged_in();
    let reload = press(&mut app, key('R'))[0];
    let repost = press(&mut app, key('b'))[0];
    app.handle_answer(
        repost,
        Event::Reposted {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.repost/y".into()),
        },
    );
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![reloaded(false, false)].into())),
    );
    let p = &app.timeline.items[0];
    assert_eq!(
        p.repost_uri(),
        Some("at://did:plc:me/app.bsky.feed.repost/y")
    );
    assert_eq!(p.repost_count, 1);
    let jobs = app.handle_key(key('b'));
    assert!(matches!(&jobs[..], [Job::Unrepost { .. }]), "{jobs:?}");
}

// A post deleted while a reload was out does not come back with the
// reload, which was read before the delete: shown again, it could be
// deleted a second time, or answered as if it were still there.
#[test]
fn a_reload_sent_before_a_delete_does_not_bring_the_post_back() {
    let mine = || post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(vec![mine()].into())));
    let thread = press(&mut app, key('v'))[0];
    app.handle_answer(
        thread,
        Event::Thread {
            uri: "at://did:plc:me/app.bsky.feed.post/m1".into(),
            result: Ok(serde_json::from_value(json!({
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "at://did:plc:me/app.bsky.feed.post/m1", "cid": "c",
                         "author": {"did": "did:plc:me", "handle": "me.test"},
                         "record": {"text": "mine"}},
                "replies": [],
            }))
            .unwrap()),
        },
    );
    app.handle_key(code(KeyCode::Esc));
    let reload = press(&mut app, key('R'))[0];
    press(&mut app, key('D'));
    let delete = press(&mut app, key('y'))[0];
    app.handle_answer(
        delete,
        Event::PostDeleted {
            uri: "at://did:plc:me/app.bsky.feed.post/m1".into(),
            result: Ok(()),
        },
    );
    assert!(app.timeline.items.is_empty());
    app.handle_answer(reload, Event::Timeline(Ok(vec![mine()].into())));
    assert!(
        app.timeline.items.is_empty(),
        "the deleted post came back: {:?}",
        app.timeline.items
    );
}

// In an open thread, the deleted post keeps its row as a placeholder,
// so the replies under it keep their place.
#[test]
fn a_post_deleted_in_a_thread_leaves_a_placeholder() {
    let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(vec![mine.clone()].into())));
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: mine.uri.clone(),
        result: Ok(serde_json::from_value(json!({
            "$type": "app.bsky.feed.defs#threadViewPost",
            "post": {"uri": mine.uri, "cid": "c",
                     "author": {"did": "did:plc:me", "handle": "me.test"},
                     "record": {"text": "mine"}},
            "replies": [{
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "at://b/p/r", "cid": "c",
                         "author": {"did": "did:plc:bob", "handle": "bob.test"},
                         "record": {"text": "a reply"}},
                "replies": [],
            }],
        }))
        .unwrap()),
    });
    app.handle_key(key('D'));
    let jobs = app.handle_key(key('y'));
    assert!(matches!(&jobs[..], [Job::DeletePost { .. }]), "{jobs:?}");
    app.handle_event(Event::PostDeleted {
        uri: mine.uri.clone(),
        result: Ok(()),
    });
    let rows = &app.threads[0].list.items;
    assert_eq!(rows.len(), 2);
    assert!(
        matches!(&rows[0].kind, RowKind::NotFound(u) if *u == mine.uri),
        "{:?}",
        rows[0].kind
    );
    assert!(rows[1].post().is_some_and(|p| p.uri == "at://b/p/r"));
}

#[test]
fn a_reload_sent_before_an_unfollow_does_not_bring_the_account_back() {
    let mut app = logged_in();
    let reload = press(&mut app, key('R'))[0];
    let unfollow = press(&mut app, key('f'))[0];
    app.handle_answer(
        unfollow,
        Event::Unfollowed {
            did: "did:plc:alice".into(),
            result: Ok(()),
        },
    );
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ]
        .into())),
    );
    let authors: Vec<_> = app
        .timeline
        .items
        .iter()
        .map(|p| p.author.did.as_str())
        .collect();
    assert_eq!(authors, ["did:plc:bob"]);
}

// Two loads of one list in flight: the one sent last is what shows,
// whichever answers last.
#[test]
fn an_older_first_page_does_not_replace_a_newer_one() {
    let mut app = logged_in();
    let first = press(&mut app, key('R'))[0];
    let second = press(&mut app, key('R'))[0];
    app.handle_answer(
        second,
        Event::Timeline(Ok(vec![
            post("at://me/p/new", "did:plc:me", false),
            post("at://a/p/1", "did:plc:alice", true),
        ]
        .into())),
    );
    app.handle_answer(
        first,
        Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
    );
    assert_eq!(app.timeline.items[0].uri, "at://me/p/new");
}

#[test]
fn a_page_asked_for_by_the_last_account_is_dropped() {
    let mut app = logged_in();
    let reload = press(&mut app, key('R'))[0];
    let mut other = session();
    other.did = "did:plc:other".into();
    let login = app.stamp(&Job::UpdateSeen(String::new()));
    app.handle_answer(login, Event::LoggedIn(Ok(other)));
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
    );
    assert!(app.timeline.items.is_empty());
}

// Reads run beside writes, so a reload can show a like or repost before
// its own answer arrives; the answer must not count it a second time.
#[test]
fn a_like_or_repost_a_reload_already_shows_is_counted_once() {
    let mut app = logged_in();
    app.handle_key(key('l'));
    app.handle_key(key('b'));
    app.handle_event(Event::Timeline(Ok(vec![reloaded(true, true)].into())));
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.like/x".into()),
    });
    app.handle_event(Event::Reposted {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.repost/y".into()),
    });
    let p = &app.timeline.items[0];
    assert_eq!((p.like_count, p.repost_count), (3, 1));

    app.handle_key(key('l'));
    app.handle_key(key('b'));
    app.handle_event(Event::Timeline(Ok(vec![reloaded(false, false)].into())));
    app.handle_event(Event::Unliked {
        post_uri: "at://a/p/1".into(),
        result: Ok(()),
    });
    app.handle_event(Event::Unreposted {
        post_uri: "at://a/p/1".into(),
        result: Ok(()),
    });
    let p = &app.timeline.items[0];
    assert_eq!((p.like_count, p.repost_count), (2, 0));
    assert!(p.like_uri().is_none());
}

#[test]
fn results_for_an_earlier_search_are_dropped() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "cats");
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('/'));
    for _ in 0..4 {
        app.handle_key(code(KeyCode::Backspace));
    }
    type_str(&mut app, "dogs");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(&jobs[..], [Job::SearchPosts(q)] if q == "dogs"),
        "{jobs:?}"
    );
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(vec![post("at://c/p/1", "did:plc:cat", false)].into()),
    });
    assert!(!app.search.posts.loaded);
    app.handle_event(Event::SearchPosts {
        query: "dogs".into(),
        result: Ok(vec![post("at://d/p/1", "did:plc:dog", false)].into()),
    });
    assert_eq!(app.search.posts.items[0].uri, "at://d/p/1");

    app.search.mode = SearchMode::Accounts;
    app.search.actors_query = "dogs".into();
    app.search.actors.begin();
    app.handle_event(Event::SearchActors {
        query: "cats".into(),
        result: Err(Error::api("late failure")),
    });
    assert!(!app.search.actors.loaded);
    assert!(app.status.is_none(), "{:?}", app.status);
}

/// Q quotes the selected post: the composer says whose post it quotes,
/// and what goes out carries that post by URI and CID.
#[test]
fn quote_sends_the_quoted_post_s_reference() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    app.handle_key(key('Q'));
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("{:?}", app.overlay)
    };
    assert_eq!(
        c.quote
            .as_ref()
            .map(|(r, h, _)| (r.uri.as_str(), h.as_str())),
        Some(("at://b/p/2", "did:plc:bob.test"))
    );
    assert!(c.reply.is_none());
    type_str(&mut app, "worth reading");
    let jobs = app.handle_key(ctrl('s'));
    match &jobs[..] {
        [
            Job::Post {
                text, reply, quote, ..
            },
        ] => {
            assert_eq!(text, "worth reading");
            assert!(reply.is_none());
            let q = quote.as_ref().expect("the quoted post");
            assert_eq!(q.uri, "at://b/p/2");
            assert_eq!(q.cid, "cid-at://b/p/2");
        }
        other => panic!("{other:?}"),
    }
}
