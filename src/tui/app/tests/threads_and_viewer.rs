use super::*;

// o always leads somewhere: the post's link where it has one, and the
// post itself on bsky.app where it has none, which is where its
// replies and its author's other posts are.
#[test]
fn o_opens_the_link_or_the_post_itself() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p1",
            json!({"$type": "app.bsky.embed.external#view",
                   "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
        ),
        post(
            "at://did:plc:bob/app.bsky.feed.post/p2",
            "did:plc:bob",
            true,
        ),
    ]
    .into())));
    let jobs = app.handle_key(key('o'));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
        "{jobs:?}"
    );
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('o'));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:bob/post/p2"),
        "{jobs:?}"
    );
    // Space still opens the viewer, and says so when there is nothing
    // to view: it does not send the reader to a browser.
    let jobs = app.handle_key(key(' '));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("no pictures, video, or link")),
        "{:?}",
        app.status
    );
}

// A terminal that cannot show pictures has no viewer: space opens the
// post on bsky.app, where they can be seen, and a link as before.
#[test]
fn without_pictures_space_opens_the_post_in_the_browser() {
    let mut app = logged_in();
    app.without_pictures();
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("cannot show pictures")),
        "{:?}",
        app.status
    );
    app.handle_event(Event::Timeline(Ok(vec![
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p1",
            json!({"$type": "app.bsky.embed.images#view", "images": [{"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}]}),
        ),
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p2",
            json!({"$type": "app.bsky.embed.external#view", "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
        ),
    ]
    .into())));
    let jobs = app.handle_key(key(' '));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:alice/post/p1"),
        "{jobs:?}"
    );
    assert!(app.overlay.is_none());
    app.handle_key(key('j'));
    let jobs = app.handle_key(key(' '));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
        "{jobs:?}"
    );
}

#[test]
fn without_pictures_the_keys_say_what_space_does() {
    let mut app = logged_in();
    assert!(crate::tui::keys::hints(&app).contains(&("space", "view")));
    app.without_pictures();
    let hints = crate::tui::keys::hints(&app);
    assert!(hints.contains(&("space", "open in browser")), "{hints:?}");
    assert!(!hints.contains(&("space", "view")));
    let help = crate::tui::keys::help(false);
    assert!(help.iter().all(|(title, _)| *title != "Viewer"));
    let posts = &help.iter().find(|(t, _)| *t == "Posts").unwrap().1;
    assert!(
        posts
            .iter()
            .any(|(k, d)| *k == "space" && d.contains("web browser"))
    );
    assert!(
        crate::tui::keys::help(true)
            .iter()
            .any(|(t, _)| *t == "Viewer")
    );
}

#[test]
fn v_opens_the_thread_and_esc_closes_it() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    let th = app.threads.last().unwrap();
    assert_eq!(th.list.items.len(), 4);
    assert_eq!(
        th.list.selected, 1,
        "the opened post is selected, below its parent"
    );
    // Keys act on the thread: j moves to the first reply and l likes it.
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('l'));
    assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r1"));
    app.handle_event(Event::Liked {
        post_uri: "at://r1".into(),
        result: Ok("at://like".into()),
    });
    let liked = app.threads.last().unwrap().list.items[2]
        .post()
        .unwrap()
        .clone();
    assert_eq!(liked.like_count, 1);
    app.handle_key(code(KeyCode::Esc));
    assert!(app.threads.is_empty());
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(app.timeline.selected, 0, "the timeline is where it was");
}

#[test]
fn a_thread_inside_a_thread_stacks() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://r1"));
    assert_eq!(app.threads.len(), 2);
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.threads.len(), 1);
}

#[test]
fn a_late_thread_answer_is_dropped_and_a_tab_switch_closes_threads() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_key(code(KeyCode::Esc));
    // The answer for the thread already closed changes nothing.
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &[])),
    });
    assert!(app.threads.is_empty());
    app.handle_key(key('v'));
    app.handle_key(key('2'));
    assert!(app.threads.is_empty());
}

// was: R set the selection back to the focused post, so the next key
// acted on it instead of the reply that was picked.
#[test]
fn reloading_a_thread_keeps_the_selected_reply() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    assert_eq!(app.threads[0].list.selected, 3, "the second reply");
    app.handle_key(key('R'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    assert_eq!(app.threads[0].list.selected, 3);
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r2"),
        "{jobs:?}"
    );
}

#[test]
fn reloading_a_thread_whose_selected_reply_is_gone_goes_back_to_the_post() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    app.handle_key(key('R'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    assert_eq!(
        app.threads[0].list.selected, 1,
        "the opened post, below its parent"
    );
}

#[test]
fn a_failed_thread_says_why_and_r_retries() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Err(Error::api("NotFound: Post not found")),
    });
    assert_eq!(
        app.threads[0].error.as_deref(),
        Some("NotFound: Post not found")
    );
    let jobs = app.handle_key(key('R'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
}

#[test]
fn an_unreadable_settings_file_is_never_overwritten() {
    let mut app = logged_in();
    app.apply_settings(
        Settings::default(),
        ColorDepth::TrueColor,
        Some("settings.json is not valid and was ignored".into()),
    );
    app.handle_key(key('T'));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.take_settings_save().is_none());
    assert_eq!(app.theme_index, 1, "used for this session");
    let status = app.status.as_ref().unwrap();
    assert!(
        status.error && status.text.contains("not overwritten"),
        "{status:?}"
    );
}
