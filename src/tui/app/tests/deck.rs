use super::*;

#[test]
fn a_column_is_added_from_the_list_kept_and_loaded() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    assert!(app.columns.items.is_empty());
    app.handle_key(key('+'));
    let choices = app.column_choices();
    let at = choices
        .iter()
        .position(|c| *c == columns::Source::Notifications)
        .unwrap();
    for _ in 0..at {
        app.handle_key(key('j'));
    }
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(
            &jobs[..],
            [Job::Column {
                feed: Feed::Notifications,
                cursor: None,
                ..
            }]
        ),
        "{jobs:?}"
    );
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(
        saved.columns["did:plc:me"],
        [columns::Source::Notifications]
    );
    // A search column asks for its query first.
    app.handle_key(key('+'));
    app.handle_key(key('k'));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(
        app.overlay,
        Some(Overlay::AddColumn { query: Some(_), .. })
    ));
    type_str(&mut app, "猫🐈‍⬛ 1️⃣");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(&jobs[..], [Job::Column { feed: Feed::SearchPosts(q), .. }] if q == "猫🐈‍⬛ 1️⃣"),
        "{jobs:?}"
    );
    assert_eq!(app.columns.focus, 1);
    assert!(app.overlay.is_none());
}

#[test]
fn the_keys_of_a_post_act_on_the_focused_columns_selection() {
    let mut app = columns_with(
        &[
            columns::Source::Following,
            columns::Source::Search { query: "x".into() },
        ],
        vec![
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ],
    );
    assert_eq!(app.columns.focus, 0);
    app.handle_key(code(KeyCode::Right));
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://b/p/2"),
        "{jobs:?}"
    );
    app.handle_event(Event::Liked {
        post_uri: "at://b/p/2".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.like/n".into()),
    });
    // Liked in both columns and on the Timeline tab.
    for c in &app.columns.items {
        let Rows::Posts(l) = &c.rows else { panic!() };
        assert!(l.items[1].like_uri().is_some(), "{:?}", c.source);
    }
    assert!(app.timeline.items[1].like_uri().is_some());
    // H goes back left; the first column's selection is where it was.
    app.handle_key(key('H'));
    assert_eq!(app.columns.focus, 0);
    assert_eq!(app.shown_post().unwrap().uri, "at://a/p/1");
}

#[test]
fn a_page_lands_only_in_its_column_and_not_after_a_reload_or_a_removal() {
    let mut app = columns_with(
        &[columns::Source::Following, columns::Source::Following],
        vec![post("at://a/p/1", "did:plc:alice", true)],
    );
    let first = app.columns.items[0].id;
    let generation = app.columns.items[0].generation;
    // R asks again; the answer to the earlier ask is dropped.
    let jobs = app.handle_key(key('R'));
    assert!(
        matches!(&jobs[..], [Job::Column { id, .. }] if *id == first),
        "{jobs:?}"
    );
    app.handle_event(Event::Column {
        id: first,
        generation,
        cursor: None,
        result: Ok(MorePage::Posts(
            vec![post("at://z/p/9", "did:plc:z", true)].into(),
        )),
    });
    let Rows::Posts(l) = &app.columns.items[0].rows else {
        panic!()
    };
    assert!(!l.loaded, "still waiting for its own answer");
    let Rows::Posts(other) = &app.columns.items[1].rows else {
        panic!()
    };
    assert_eq!(other.items.len(), 1, "the other column is untouched");
    // Removed after x and y: its answer goes nowhere.
    app.handle_key(key('x'));
    app.handle_key(key('n'));
    assert_eq!(app.columns.items.len(), 2, "any key but y keeps it");
    app.handle_key(key('x'));
    app.handle_key(key('y'));
    assert_eq!(app.columns.items.len(), 1);
    app.handle_event(Event::Column {
        id: first,
        generation: generation + 1,
        cursor: None,
        result: Ok(MorePage::Posts(Vec::new().into())),
    });
    assert_eq!(app.columns.items.len(), 1);
    assert_ne!(app.columns.items[0].id, first);
}

#[test]
fn a_deleted_post_leaves_every_column_and_each_account_has_its_own_columns() {
    let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
    let mut app = columns_with(&[columns::Source::Following], vec![mine.clone()]);
    app.handle_key(key('D'));
    app.handle_key(key('y'));
    app.handle_event(Event::PostDeleted {
        uri: mine.uri.clone(),
        result: Ok(()),
    });
    let Rows::Posts(l) = &app.columns.items[0].rows else {
        panic!()
    };
    assert!(l.items.is_empty());
    // Another account has none of these columns.
    app.accounts = vec![Account::from(&session()), Account::from(&work())];
    app.switched_to(work());
    assert!(app.columns.items.is_empty());
    app.switched_to(session());
    assert_eq!(app.columns.sources(), [columns::Source::Following]);
}
