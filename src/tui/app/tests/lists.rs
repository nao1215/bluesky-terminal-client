use super::*;

#[test]
fn nothing_more_is_fetched_on_load_or_far_from_the_end() {
    let mut app = timeline_with(MORE_AHEAD + 3, Some("c1"));
    assert_eq!(
        app.pending, 2,
        "only the notifications and the pinned feeds, in the background"
    );
    assert!(app.handle_key(key('j')).is_empty());
    assert!(app.handle_key(key('j')).is_empty());
}

#[test]
fn nearing_the_end_fetches_the_next_page_once() {
    let mut app = timeline_with(MORE_AHEAD + 2, Some("c1"));
    assert!(app.handle_key(key('j')).is_empty());
    let jobs = app.handle_key(key('j')); // MORE_AHEAD from the end
    assert!(matches!(&jobs[..], [Job::More { feed: Feed::Timeline, cursor }] if cursor == "c1"));
    // While it is on its way, moving on asks for nothing more.
    assert!(app.handle_key(key('j')).is_empty());
    app.handle_event(Event::More {
        feed: Feed::Timeline,
        cursor: "c1".into(),
        result: Ok(MorePage::Posts(page(
            vec![
                post(&format!("at://p/{}", MORE_AHEAD + 1), "did:plc:a", true),
                post("at://p/new", "did:plc:a", true),
            ],
            Some("c2"),
        ))),
    });
    // The first was already there: only one is new; the selection did not move.
    assert_eq!(app.timeline.items.len(), MORE_AHEAD + 3);
    assert_eq!(app.timeline.selected, 3);
    assert_eq!(app.timeline.cursor.as_deref(), Some("c2"));
}

#[rstest::rstest]
#[case::no_cursor(None)]
#[case::same_cursor(Some("c1"))]
fn the_list_ends_when_the_cursor_stops_moving(#[case] next: Option<&str>) {
    let mut app = timeline_with(3, Some("c1"));
    let jobs = app.handle_key(key('j'));
    assert_eq!(jobs.len(), 1);
    app.handle_event(Event::More {
        feed: Feed::Timeline,
        cursor: "c1".into(),
        result: Ok(MorePage::Posts(page(vec![], next))),
    });
    assert_eq!(app.timeline.cursor, None);
    assert!(app.handle_key(key('j')).is_empty());
    assert!(app.handle_key(key('G')).is_empty());
}

#[test]
fn a_page_for_a_refreshed_list_is_dropped() {
    let mut app = timeline_with(3, Some("c1"));
    app.handle_key(key('j'));
    // R replaced the list before the old page came back.
    app.handle_event(Event::Timeline(Ok(page(
        vec![post("at://fresh", "did:plc:a", true)],
        Some("new"),
    ))));
    app.handle_event(Event::More {
        feed: Feed::Timeline,
        cursor: "c1".into(),
        result: Ok(MorePage::Posts(page(
            vec![post("at://old", "did:plc:a", true)],
            None,
        ))),
    });
    let uris: Vec<&str> = app.timeline.items.iter().map(|p| p.uri.as_str()).collect();
    assert_eq!(uris, ["at://fresh"]);
    assert_eq!(app.timeline.cursor.as_deref(), Some("new"));
}

#[test]
fn a_failed_page_can_be_tried_again() {
    let mut app = timeline_with(2, Some("c1"));
    assert_eq!(app.handle_key(key('j')).len(), 1);
    app.handle_event(Event::More {
        feed: Feed::Timeline,
        cursor: "c1".into(),
        result: Err(Error::api("boom")),
    });
    assert!(app.status.as_ref().unwrap().error);
    assert_eq!(app.handle_key(key('k')).len(), 1, "the next move retries");
}

#[test]
fn a_page_for_an_old_query_is_dropped() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "old");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::SearchPosts {
        query: "old".into(),
        result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], Some("c1"))),
    });
    assert_eq!(app.handle_key(key('j')).len(), 1);
    // A new search starts before the page arrives.
    app.handle_key(key('/'));
    app.handle_key(ctrl('u'));
    type_str(&mut app, "new");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::More {
        feed: Feed::SearchPosts("old".into()),
        cursor: "c1".into(),
        result: Ok(MorePage::Posts(page(
            vec![post("at://s/2", "did:plc:x", false)],
            None,
        ))),
    });
    assert_eq!(app.search.posts.items.len(), 1);
}

#[test]
fn repost_toggles_and_waits_for_the_answer() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('b'));
    let [Job::Repost { subject }] = &jobs[..] else {
        panic!("{jobs:?}")
    };
    assert_eq!(subject.uri, "at://a/p/1");
    assert!(app.handle_key(key('b')).is_empty(), "second press waits");
    app.handle_event(Event::Reposted {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.repost/r1".into()),
    });
    assert_eq!(app.timeline.items[0].repost_count, 1);
    let jobs = app.handle_key(key('b'));
    assert!(matches!(&jobs[..], [Job::Unrepost { repost_uri, .. }] if repost_uri.ends_with("/r1")));
    app.handle_event(Event::Unreposted {
        post_uri: "at://a/p/1".into(),
        result: Ok(()),
    });
    assert_eq!(app.timeline.items[0].repost_count, 0);
    assert!(app.timeline.items[0].repost_uri().is_none());
}

#[test]
fn selection_is_clamped_to_the_list() {
    let mut app = logged_in();
    for _ in 0..5 {
        app.handle_key(key('j'));
    }
    assert_eq!(app.timeline.selected, 1);
    app.handle_key(key('g'));
    assert_eq!(app.timeline.selected, 0);
    app.handle_key(key('G'));
    assert_eq!(app.timeline.selected, 1);
}

#[test]
fn each_search_list_pages_with_its_own_query() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "a");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::SearchPosts {
        query: "a".into(),
        result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], Some("pa"))),
    });
    // Accounts for "b", then back to the posts for "a" without searching.
    app.handle_key(key('/'));
    app.handle_key(ctrl('t'));
    app.handle_key(ctrl('u'));
    type_str(&mut app, "b");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::SearchActors {
        query: "b".into(),
        result: Ok(Vec::new().into()),
    });
    app.handle_key(key('/'));
    app.handle_key(ctrl('t'));
    app.handle_key(code(KeyCode::Esc));
    let jobs = app.handle_key(key('j'));
    assert!(
        matches!(&jobs[..], [Job::More { feed: Feed::SearchPosts(q), cursor }] if q == "a" && cursor == "pa"),
        "{jobs:?}"
    );
    app.handle_event(Event::More {
        feed: Feed::SearchPosts("a".into()),
        cursor: "pa".into(),
        result: Ok(MorePage::Posts(page(
            vec![post("at://s/2", "did:plc:x", false)],
            None,
        ))),
    });
    assert_eq!(app.search.posts.items.len(), 2);
}

#[test]
fn logging_in_as_someone_else_forgets_the_last_account() {
    let mut app = notifications_tab();
    app.handle_key(key('v'));
    app.fail(&Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken",
    ));
    let other = Session {
        did: "did:plc:other".into(),
        handle: "other.test".into(),
        ..session()
    };
    let jobs = app.handle_event(Event::LoggedIn(Ok(other)));
    assert!(matches!(
        &jobs[..],
        [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
    ));
    assert!(app.notifications.items.is_empty());
    assert!(!app.notifications.loaded);
    assert_eq!(app.unread, 0);
    assert!(app.threads.is_empty());
    assert!(app.timeline.items.is_empty());
    assert_eq!(app.tab, Tab::Timeline);
    // The same account again keeps what was loaded.
    let mut app = notifications_tab();
    app.handle_event(Event::LoggedIn(Ok(session())));
    assert_eq!(app.notifications.items.len(), 3);
}

#[test]
fn a_tab_still_loading_is_not_asked_for_again() {
    let mut app = logged_in();
    // The notifications are on their way since the start.
    assert!(app.handle_key(key('3')).is_empty());
    app.handle_key(key('1'));
    assert!(app.handle_key(key('3')).is_empty());
    assert_eq!(app.handle_key(key('4')).len(), 1);
    app.handle_key(key('1'));
    assert!(app.handle_key(key('4')).is_empty());
}

#[test]
fn search_keys_do_not_reach_behind_a_thread() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "q");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::SearchPosts {
        query: "q".into(),
        result: Ok(page(vec![post("at://s/1", "did:plc:x", false)], None)),
    });
    app.handle_key(key('v'));
    app.handle_key(key('i'));
    assert!(!app.search.editing);
    assert!(app.handle_key(key('t')).is_empty());
    assert_eq!(app.search.mode, SearchMode::Posts);
}

// A reload of the timeline (R, or after posting) brings its first page. The
// posts loaded further down stay, and so does the selection: the next l
// likes the post that was selected, not the first one.
#[test]
fn reloading_the_timeline_keeps_the_pages_loaded_and_the_selection() {
    let mut app = timeline_with(50, Some("c1"));
    for _ in 0..45 {
        app.handle_key(key('j'));
    }
    let more: Vec<Post> = (50..60)
        .map(|i| post(&format!("at://p/{i}"), "did:plc:alice", true))
        .collect();
    app.handle_event(Event::More {
        feed: Feed::Timeline,
        cursor: "c1".into(),
        result: Ok(MorePage::Posts(page(more, Some("c2")))),
    });
    for _ in 0..10 {
        app.handle_key(key('j'));
    }
    let chosen = app.timeline.current().unwrap().uri.clone();
    assert_eq!(chosen, "at://p/55");
    app.handle_key(key('R'));
    let first: Vec<Post> = (0..50)
        .map(|i| post(&format!("at://p/{i}"), "did:plc:alice", true))
        .collect();
    app.handle_event(Event::Timeline(Ok(page(first, Some("c1b")))));
    assert_eq!(app.timeline.items.len(), 60);
    assert_eq!(app.timeline.current().unwrap().uri, chosen);
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == chosen),
        "{jobs:?}"
    );
}
