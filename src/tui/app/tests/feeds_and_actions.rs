use super::*;

/// The Timeline tab shows the following timeline alone: [ and ] do
/// nothing, and the pinned feeds are what + offers, as columns.
#[test]
fn the_pinned_feeds_are_offered_as_columns_not_switched_to() {
    let mut app = logged_in();
    app.handle_event(Event::PinnedFeeds(Ok(vec![
        feed_info("discover"),
        feed_info("science 🔬"),
    ])));
    assert!(app.handle_key(key(']')).is_empty());
    assert!(app.handle_key(key('[')).is_empty());
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(app.timeline.current().unwrap().uri, "at://a/p/1");
    let choices = app.column_choices();
    for name in ["discover", "science 🔬"] {
        assert!(
            choices
                .iter()
                .any(|c| matches!(c, columns::Source::Feed { name: n, .. } if n == name)),
            "{choices:?}"
        );
    }
}

/// The actions list is a way to reach the keys of a post without
/// remembering them: what it runs is exactly what the key runs.
#[test]
fn the_actions_list_runs_the_key_it_names() {
    let mut app = logged_in();
    assert!(app.handle_key(key('.')).is_empty());
    let Some(Overlay::Actions { selected: 0, .. }) = app.overlay else {
        panic!("{:?}", app.overlay)
    };
    // The entries say what each key would do to this post now.
    let entries = crate::tui::keys::actions(&app);
    assert_eq!(entries[0], ("r", "reply to it"));
    assert_eq!(entries[1], ("l", "like it"));
    // Moving to the like and choosing it sends the like the key sends.
    app.handle_key(key('j'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://a/p/1"));
    assert!(app.overlay.is_none(), "the list closes when it has run");
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.like/l".into()),
    });
    // With the post liked, the list says what l would do now.
    app.handle_key(key('.'));
    assert_eq!(
        crate::tui::keys::actions(&app)[1],
        ("l", "remove your like")
    );
    // A key of the list works from inside it, and Esc leaves everything
    // as it was.
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://did:plc:bob/app.bsky.feed.post/p9",
        "did:plc:bob",
        true,
    )]
    .into())));
    app.handle_key(key('.'));
    app.handle_key(key('c'));
    assert!(app.overlay.is_none());
    assert_eq!(
        app.take_copy().as_deref(),
        Some("https://bsky.app/profile/did:plc:bob/post/p9")
    );
    app.handle_key(key('.'));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert!(app.status.is_none() || !app.status.as_ref().unwrap().error);
}

/// Nothing selected, nothing to offer: `.` does not open an empty box.
#[test]
fn the_actions_list_does_not_open_on_an_empty_list() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(Vec::new().into())));
    assert!(app.handle_key(key('.')).is_empty());
    assert!(app.overlay.is_none(), "{:?}", app.overlay);
}

/// c puts the post's address where anything else can paste it.    /// c puts the post's address where anything else can paste it. The
/// address is the one o opens, so the two keys agree.
#[test]
fn c_copies_the_post_s_address() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://did:plc:bob/app.bsky.feed.post/theirs",
        "did:plc:bob",
        true,
    )]
    .into())));
    assert!(app.handle_key(key('c')).is_empty());
    assert_eq!(
        app.take_copy().as_deref(),
        Some("https://bsky.app/profile/did:plc:bob/post/theirs")
    );
    // Taken once: the loop does not write it again on the next frame.
    assert!(app.take_copy().is_none());
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("copied https://bsky.app/profile")),
        "{:?}",
        app.status
    );
}

/// Deleting cannot be undone, so it takes two keys: D asks, y sends,
/// and one delete leaves the server. The post goes from every list it
/// is in once the server confirms it.
#[test]
fn deleting_your_own_post_asks_first_and_sends_one_delete() {
    let mut app = logged_in();
    let mine = "at://did:plc:me/app.bsky.feed.post/mine";
    app.handle_event(Event::Timeline(Ok(vec![
        post(mine, "did:plc:me", false),
        post("at://b/p/2", "did:plc:bob", true),
    ]
    .into())));
    // Asking sends nothing.
    let jobs = app.handle_key(key('D'));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("press y to delete")),
        "{:?}",
        app.status
    );
    let jobs = app.handle_key(key('y'));
    assert!(
        matches!(&jobs[..], [Job::DeletePost { uri }] if uri == mine),
        "{jobs:?}"
    );
    // A second D while the first is still out sends nothing more.
    assert!(app.handle_key(key('D')).is_empty());
    assert!(app.handle_key(key('y')).is_empty());
    app.handle_event(Event::PostDeleted {
        uri: mine.to_string(),
        result: Ok(()),
    });
    assert_eq!(app.timeline.items.len(), 1);
    assert_eq!(app.timeline.items[0].uri, "at://b/p/2");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("deleted")),
        "{:?}",
        app.status
    );
}

/// Another account's post is not deletable, and any key but y calls the
/// question off without doing what that key usually does.
#[test]
fn only_your_own_post_is_deleted_and_any_other_key_calls_it_off() {
    let mut app = logged_in();
    let mine = "at://did:plc:me/app.bsky.feed.post/mine";
    app.handle_event(Event::Timeline(Ok(vec![
        post("at://b/p/2", "did:plc:bob", true),
        post(mine, "did:plc:me", false),
    ]
    .into())));
    // On Bob's post: nothing is asked and nothing is sent.
    let jobs = app.handle_key(key('D'));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("only delete your own posts")),
        "{:?}",
        app.status
    );
    // On my own post, j calls the question off and does not move the
    // selection: the key that answers is y and nothing else.
    app.handle_key(key('j'));
    let before = app.timeline.selected;
    assert!(app.handle_key(key('D')).is_empty());
    let jobs = app.handle_key(key('j'));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert_eq!(app.timeline.selected, before);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("not deleted")),
        "{:?}",
        app.status
    );
    // The next j moves as usual.
    app.handle_key(key('k'));
    assert_ne!(app.timeline.selected, before);
}

#[test]
fn a_failed_post_keeps_the_draft() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    type_str(&mut app, "draft");
    app.handle_key(ctrl('s'));
    app.handle_event(Event::Posted {
        reply_to: None,
        result: Err(Error::api("boom")),
    });
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!()
    };
    assert_eq!(c.input.text(), "draft");
    assert!(!c.sending);
}

// The actions list offers only what its key does there: no "open the
// profile" on the Profile tab, where Enter opens nothing, and no follow on
// your own post or profile, which f refuses.
#[test]
fn the_actions_list_offers_nothing_its_key_refuses() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://did:plc:me/app.bsky.feed.post/1",
        "did:plc:me",
        false,
    )]
    .into())));
    let keys: Vec<&str> = crate::tui::keys::actions(&app)
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert!(!keys.contains(&"f"), "{keys:?}");
    app.handle_key(key('5'));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:me", "handle": "me.test"})).unwrap(),
        Vec::new().into(),
    ))));
    let keys: Vec<&str> = crate::tui::keys::actions(&app)
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert!(!keys.contains(&"enter"), "{keys:?}");
    assert!(!keys.contains(&"f"), "{keys:?}");
}
