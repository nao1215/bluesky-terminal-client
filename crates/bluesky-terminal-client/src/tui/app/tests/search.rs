use super::*;

fn search(app: &mut App, query: &str, mode: SearchMode) -> Vec<Job> {
    app.handle_key(key('/'));
    app.handle_key(ctrl('u'));
    if app.search.mode != mode {
        app.handle_key(ctrl('t'));
    }
    type_str(app, query);
    app.handle_key(code(KeyCode::Enter))
}

#[rstest::rstest]
#[case(Tab::Timeline)]
#[case(Tab::Notifications)]
#[case(Tab::Profile)]
#[case(Tab::Columns)]
#[case(Tab::Chat)]
fn search_box_cancels_without_changing_its_list(#[case] tab: Tab) {
    let mut app = logged_in();
    app.tab = tab;
    app.timeline.selected = 1;
    app.timeline.offset = 1;
    assert!(app.handle_key(key('/')).is_empty());
    assert!(matches!(app.overlay, Some(Overlay::Search)));
    type_str(&mut app, "q l f 猫");
    app.handle_key(ctrl('t'));
    assert_eq!(app.search.mode, SearchMode::Accounts);
    assert_eq!(app.tab, tab);
    assert!(!app.quit);
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert!(app.overlay.is_none());
    assert_eq!(app.tab, tab);
    assert_eq!((app.timeline.selected, app.timeline.offset), (1, 1));
    app.handle_key(key('/'));
    app.handle_key(code(KeyCode::Home));
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('/'));
    type_str(&mut app, "!");
    assert_eq!(app.search.input.text(), "q l f 猫!");
}

#[test]
fn blank_search_keeps_the_box_open_and_sends_nothing() {
    let mut app = logged_in();
    assert!(search(&mut app, "  ", SearchMode::Posts).is_empty());
    assert!(matches!(app.overlay, Some(Overlay::Search)));
    assert!(app.search.active.is_none());
}

#[test]
fn results_page_and_escape_restores_the_exact_timeline_without_a_job() {
    let mut app = logged_in();
    app.timeline.selected = 1;
    app.timeline.offset = 1;
    app.timeline.cursor = Some("timeline-next".into());
    let before = format!("{:?}", app.timeline);
    assert!(
        matches!(&search(&mut app, "cats", SearchMode::Posts)[..], [Job::SearchPosts(q)] if q == "cats")
    );
    assert!(app.overlay.is_none());
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(page(
            vec![post("at://cats/1", "did:plc:c", false)],
            Some("cats-next"),
        )),
    });
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(app.search_mode(), Some(SearchMode::Posts));
    assert_eq!(app.shown_post().unwrap().uri, "at://cats/1");
    let jobs = app.handle_key(key('j'));
    assert!(
        matches!(&jobs[..], [Job::More { feed: Feed::SearchPosts(q), cursor }] if q == "cats" && cursor == "cats-next")
    );
    app.handle_event(Event::More {
        feed: Feed::SearchPosts("cats".into()),
        cursor: "cats-next".into(),
        result: Ok(MorePage::Posts(page(
            vec![post("at://cats/2", "did:plc:c", false)],
            None,
        ))),
    });
    app.handle_key(key('j'));
    assert_eq!(app.shown_post().unwrap().uri, "at://cats/2");
    app.handle_key(key('.'));
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.search_mode(), Some(SearchMode::Posts));
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert!(app.search_mode().is_none());
    assert_eq!(format!("{:?}", app.timeline), before);
    assert_eq!(app.shown_post().unwrap().uri, "at://b/p/2");
    app.handle_key(key('/'));
    assert!(
        matches!(&app.handle_key(code(KeyCode::Enter))[..], [Job::SearchPosts(q)] if q == "cats")
    );
}

#[rstest::rstest]
#[case(KeyCode::Enter)]
#[case(KeyCode::Char('v'))]
fn accounts_page_open_a_profile_and_return_to_results_then_timeline(#[case] open: KeyCode) {
    let mut app = logged_in();
    app.timeline.selected = 1;
    search(&mut app, "cat", SearchMode::Accounts);
    let actor = |name: &str| {
        serde_json::from_value(
            json!({"did":format!("did:plc:{name}"),"handle":format!("{name}.test")}),
        )
        .unwrap()
    };
    app.handle_event(Event::SearchActors {
        query: "cat".into(),
        result: Ok(Page {
            items: vec![actor("cat")],
            cursor: Some("more-actors".into()),
        }),
    });
    assert!(app.shown_post().is_none());
    let jobs = app.handle_key(key('j'));
    assert!(
        matches!(&jobs[..], [Job::More { feed: Feed::SearchActors(q), cursor }] if q == "cat" && cursor == "more-actors")
    );
    app.handle_event(Event::More {
        feed: Feed::SearchActors("cat".into()),
        cursor: "more-actors".into(),
        result: Ok(MorePage::Actors(vec![actor("kitty")].into())),
    });
    app.handle_key(key('j'));
    assert!(
        matches!(&app.handle_key(code(open))[..], [Job::OpenProfile(did)] if did == "did:plc:kitty")
    );
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert_eq!(app.search_mode(), Some(SearchMode::Accounts));
    assert_eq!(app.search.actors.selected, 1);
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert!(app.search_mode().is_none());
    assert_eq!(app.timeline.selected, 1);
}

#[test]
fn failed_search_preserves_content_query_mode_and_paging_cursor() {
    let mut app = logged_in();
    let before = format!("{:?}", app.timeline);
    search(&mut app, "broken", SearchMode::Posts);
    app.handle_event(Event::SearchPosts {
        query: "broken".into(),
        result: Err(Error::api("unavailable")),
    });
    assert_eq!(format!("{:?}", app.timeline), before);
    assert!(app.search.active.is_none());
    search(&mut app, "cats", SearchMode::Posts);
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(page(
            vec![post("at://cats/1", "did:plc:c", false)],
            Some("next"),
        )),
    });
    let before = format!("{:?}", app.search.posts);
    search(&mut app, "dogs", SearchMode::Accounts);
    app.handle_event(Event::SearchActors {
        query: "dogs".into(),
        result: Err(Error::api("unavailable")),
    });
    assert_eq!(app.search_mode(), Some(SearchMode::Posts));
    assert_eq!(app.search_query(), "cats");
    assert_eq!(format!("{:?}", app.search.posts), before);
    let status = app.status.as_ref().unwrap();
    assert!(status.error);
    assert!(!status.popup);
    assert_eq!(status.text, "unavailable");
    app.handle_key(key('/'));
    app.handle_key(ctrl('t'));
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(format!("{:?}", app.search.posts), before);
    // R reloads the visible results, not the cancelled draft.
    assert!(matches!(&app.handle_key(key('R'))[..], [Job::SearchPosts(q)] if q == "cats"));
}

#[test]
fn late_answers_cannot_reopen_search_or_replace_a_newer_request() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "cats");
    let old = press(&mut app, code(KeyCode::Enter))[0];
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('/'));
    let new = press(&mut app, code(KeyCode::Enter))[0];
    app.handle_answer(
        old,
        Event::SearchPosts {
            query: "cats".into(),
            result: Ok(Vec::new().into()),
        },
    );
    assert!(app.search.active.is_none());
    app.handle_answer(
        new,
        Event::SearchPosts {
            query: "cats".into(),
            result: Ok(Vec::new().into()),
        },
    );
    assert_eq!(app.search_mode(), Some(SearchMode::Posts));
    app.handle_key(key('R'));
    app.handle_key(key('3'));
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(Vec::new().into()),
    });
    assert_eq!(app.tab, Tab::Notifications);
    assert!(app.search.active.is_none());
}

#[test]
fn search_returns_to_the_same_column_and_selection() {
    let mut app = columns_with(
        &[
            columns::Source::Following,
            columns::Source::Search {
                query: "column".into(),
            },
        ],
        vec![post("at://c/1", "did:plc:c", false)],
    );
    app.columns.focus = 1;
    let before = format!("{:?}", app.columns);
    search(&mut app, "cats", SearchMode::Posts);
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(Vec::new().into()),
    });
    assert_eq!(app.search_mode(), Some(SearchMode::Posts));
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert_eq!(app.tab, Tab::Columns);
    assert_eq!(format!("{:?}", app.columns), before);
}

#[test]
fn numbered_tabs_have_no_search_or_gap() {
    let mut app = logged_in();
    for (digit, tab) in [
        ('1', Tab::Timeline),
        ('2', Tab::Chat),
        ('3', Tab::Notifications),
        ('4', Tab::Profile),
    ] {
        app.handle_key(key(digit));
        assert_eq!(app.tab, tab);
        assert!(app.overlay.is_none());
    }
    assert_eq!(Tab::ALL.len(), 4);
}

#[test]
fn a_failed_stamped_search_does_not_supersede_the_previous_results_next_page() {
    let mut app = logged_in();
    let jobs = search(&mut app, "cats", SearchMode::Posts);
    let first = app.stamp(&jobs[0]);
    app.handle_answer(
        first,
        Event::SearchPosts {
            query: "cats".into(),
            result: Ok(page(
                vec![post("at://cats/1", "did:plc:c", false)],
                Some("next"),
            )),
        },
    );
    let more = press(&mut app, key('j'))[0];
    let jobs = search(&mut app, "dogs", SearchMode::Posts);
    let failed = app.stamp(&jobs[0]);
    app.handle_answer(
        failed,
        Event::SearchPosts {
            query: "dogs".into(),
            result: Err(Error::api("unavailable")),
        },
    );
    app.handle_answer(
        more,
        Event::More {
            feed: Feed::SearchPosts("cats".into()),
            cursor: "next".into(),
            result: Ok(MorePage::Posts(
                vec![post("at://cats/2", "did:plc:c", false)].into(),
            )),
        },
    );
    assert_eq!(app.search.posts.items.len(), 2);
    assert!(!app.search.posts.more_pending);
    assert_eq!(app.search_query(), "cats");
}
