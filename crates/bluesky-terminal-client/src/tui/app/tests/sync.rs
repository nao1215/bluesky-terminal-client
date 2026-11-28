//! Lists and counts that stay in step after writes, reloads and logins.

use super::*;

// A reload sent after the like but before its answer can have been read
// before the like was written: the like must still show.
#[test]
fn a_reload_sent_before_a_likes_answer_keeps_the_like() {
    let mut app = logged_in();
    let like = press(&mut app, key('l'))[0];
    let reload = press(&mut app, key('R'))[0];
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
}

#[test]
fn a_reload_sent_before_a_deletes_answer_keeps_the_post_gone() {
    let mine = || post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(vec![mine()].into())));
    press(&mut app, key('D'));
    let delete = press(&mut app, key('y'))[0];
    let reload = press(&mut app, key('R'))[0];
    app.handle_answer(
        delete,
        Event::PostDeleted {
            uri: "at://did:plc:me/app.bsky.feed.post/m1".into(),
            result: Ok(()),
        },
    );
    app.handle_answer(reload, Event::Timeline(Ok(vec![mine()].into())));
    assert!(
        app.timeline.items.is_empty(),
        "the deleted post came back: {:?}",
        app.timeline
            .items
            .iter()
            .map(|p| &p.uri)
            .collect::<Vec<_>>()
    );
}

// Posting from your own profile: the profile's posts are read again so the
// new post is there, as the timeline and a column of your posts are.
#[test]
fn a_post_sent_from_your_profile_loads_it_again() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('5'));
    assert!(matches!(&jobs[..], [Job::OpenProfile(_)]), "{jobs:?}");
    app.handle_key(key('n'));
    type_str(&mut app, "hello");
    app.handle_key(ctrl('s'));
    let jobs = app.handle_event(Event::Posted {
        reply_to: None,
        result: Ok(()),
    });
    assert!(
        jobs.iter().any(|j| matches!(
            j,
            Job::OpenProfile(_)
                | Job::More {
                    feed: Feed::Author(_),
                    ..
                }
        )),
        "the own profile is not reloaded: {jobs:?}"
    );
}

// Replying inside an open thread: the thread is read again so the reply
// shows under the post it answers.
#[test]
fn a_reply_in_an_open_thread_loads_the_thread_again() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &[])),
    });
    app.handle_key(key('r'));
    type_str(&mut app, "reply");
    app.handle_key(ctrl('s'));
    let jobs = app.handle_event(Event::Posted {
        reply_to: Some("at://a/p/1".into()),
        result: Ok(()),
    });
    assert!(
        jobs.iter().any(|j| matches!(j, Job::Thread(_))),
        "the open thread is not reloaded: {jobs:?}"
    );
}

// A download started before switching accounts still says where it was saved.
#[test]
fn a_download_started_before_a_switch_still_says_where_it_went() {
    let mut app = two_accounts();
    viewing_a_picture(&mut app);
    let dl = press(&mut app, key('d'))[0];
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('A'));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    app.take_account_switch();
    app.switched_to(work());
    app.handle_answer(dl, Event::Downloaded(Ok(PathBuf::from("/tmp/x.jpg"))));
    let s = app
        .status
        .as_ref()
        .map(|s| s.text.clone())
        .unwrap_or_default();
    assert!(s.contains("saved"), "status: {s:?}");
}

// A column whose load failed because the session expired is loaded again
// once logged in again: it is what the Timeline tab shows.
#[test]
fn the_columns_load_again_after_logging_in_again() {
    let mut app = columns_with(
        &[columns::Source::Following, columns::Source::Notifications],
        vec![post("at://a/p/1", "did:plc:alice", true)],
    );
    assert_eq!(app.tab, Tab::Columns);
    let jobs = app.handle_key(key('R'));
    let Some(Job::Column { id, generation, .. }) = jobs.into_iter().next() else {
        panic!()
    };
    app.handle_event(Event::Column {
        id,
        generation,
        cursor: None,
        result: Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        )),
    });
    assert!(app.login.is_some());
    let jobs = app.handle_event(Event::LoggedIn(Ok(session())));
    let err = match &app.columns.focused().unwrap().rows {
        Rows::Posts(l) => l.error.clone(),
        Rows::Notifications(l) => l.error.clone(),
    };
    assert!(
        jobs.iter().any(|j| matches!(j, Job::Column { .. })),
        "jobs after logging in again: {jobs:?}; the column still says {err:?}"
    );
}

// Following someone from their profile counts them in its followers.
#[test]
fn a_follow_on_a_profile_counts_in_its_followers() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    let mut p: Profile = serde_json::from_value(json!({
        "did": "did:plc:bob", "handle": "did:plc:bob.test", "followersCount": 10
    }))
    .unwrap();
    p.viewer = None;
    app.handle_event(Event::Profile(Ok((p, Vec::new().into()))));
    let jobs = app.handle_key(key('f'));
    assert!(matches!(&jobs[..], [Job::Follow { .. }]), "{jobs:?}");
    app.handle_event(Event::Followed {
        did: "did:plc:bob".into(),
        result: Ok("at://did:plc:me/app.bsky.graph.follow/x".into()),
    });
    let p = app.profile.profile.as_ref().unwrap();
    assert!(p.viewer.as_ref().unwrap().following.is_some());
    assert_eq!(p.followers_count, Some(11));
}
