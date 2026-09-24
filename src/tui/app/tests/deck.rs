use super::*;

#[test]
fn a_column_is_added_from_the_list_kept_and_loaded() {
    let mut app = logged_in();
    app.handle_key(key('1'));
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
    // The first column comes beside the timeline, which becomes a column
    // of its own: the Timeline tab shows both.
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(
            &jobs[..],
            [
                Job::Column {
                    feed: Feed::Timeline,
                    cursor: None,
                    ..
                },
                Job::Column {
                    feed: Feed::Notifications,
                    cursor: None,
                    ..
                }
            ]
        ),
        "{jobs:?}"
    );
    assert_eq!(app.tab, Tab::Columns);
    assert_eq!(app.tab.title(), "Timeline");
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(
        saved.columns["did:plc:me"],
        [columns::Source::Following, columns::Source::Notifications]
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
    assert_eq!(app.columns.focus, 2);
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

// A profile opened from a column says Esc goes back to the columns, which
// is where it goes.
#[test]
fn a_profile_opened_from_a_column_says_esc_goes_back_to_the_columns() {
    let mut app = columns_with(
        &[columns::Source::Following],
        vec![post("at://a/p/1", "did:plc:alice", true)],
    );
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.tab, Tab::Profile);
    let hints = crate::tui::keys::hints(&app);
    assert!(hints.contains(&("esc", "back to timeline")), "{hints:?}");
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.tab, Tab::Columns);
}

// The pinned feeds come while the + list is open: the choice selected stays
// the one Enter adds, not the one that moved into its place.
#[test]
fn feeds_that_come_while_the_add_list_is_open_do_not_change_the_choice() {
    let mut app = logged_in();
    app.handle_key(key('1'));
    app.handle_key(key('+'));
    app.handle_key(key('j'));
    app.handle_event(Event::PinnedFeeds(Ok(vec![feed_info("cats 🐈")])));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(
            &jobs[..],
            [
                Job::Column {
                    feed: Feed::Timeline,
                    ..
                },
                Job::Column {
                    feed: Feed::Notifications,
                    ..
                }
            ]
        ),
        "{jobs:?}"
    );
}

fn column_uris(app: &App, i: usize) -> Vec<String> {
    let Rows::Posts(l) = &app.columns.items[i].rows else {
        panic!("column {i} holds no posts")
    };
    l.items.iter().map(|p| p.uri.clone()).collect()
}

// The Timeline tab shows the Following column once there are columns: an
// unfollowed account's posts leave it as they leave the timeline, whether
// the unfollow answers now or a page loaded before it lands later.
#[test]
fn an_unfollowed_account_leaves_the_following_column() {
    let posts = vec![
        post("at://a/p/1", "did:plc:alice", true),
        post("at://b/p/2", "did:plc:bob", true),
    ];
    let mut app = columns_with(&[columns::Source::Following], posts.clone());
    app.handle_event(Event::Unfollowed {
        did: "did:plc:alice".into(),
        result: Ok(()),
    });
    assert_eq!(column_uris(&app, 0), ["at://b/p/2"]);

    let mut app = columns_with(&[columns::Source::Following], posts);
    app.apply(&Written::Follow {
        did: "did:plc:bob".into(),
        uri: None,
    });
    assert_eq!(column_uris(&app, 0), ["at://a/p/1"]);
}

// A new post of yours is loaded where the Timeline tab shows it: in the
// Following column and in a column of your own posts.
#[test]
fn a_new_post_loads_the_columns_that_show_your_posts() {
    let mut app = columns_with(
        &[
            columns::Source::Following,
            columns::Source::Notifications,
            columns::Source::Author {
                did: "did:plc:me".into(),
                handle: "me.test".into(),
            },
        ],
        vec![post("at://a/p/1", "did:plc:alice", true)],
    );
    let jobs = app.handle_event(Event::Posted {
        reply_to: None,
        result: Ok(()),
    });
    assert!(
        matches!(
            &jobs[..],
            [
                Job::Timeline,
                Job::Column {
                    feed: Feed::Timeline,
                    cursor: None,
                    ..
                },
                Job::Column {
                    feed: Feed::Author(_),
                    cursor: None,
                    ..
                }
            ]
        ),
        "{jobs:?}"
    );
}
