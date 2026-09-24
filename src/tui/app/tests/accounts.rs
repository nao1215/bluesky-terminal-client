use super::*;

#[test]
fn the_account_list_switches_to_another_account_and_drops_the_first_ones_lists() {
    let mut app = two_accounts();
    let reload = press(&mut app, key('R'))[0];
    app.handle_key(key('A'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Accounts { selected: 0 })
    ));
    // Enter on the one in use changes nothing.
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.take_account_switch(), None);
    app.handle_key(key('A'));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.take_account_switch().as_deref(), Some("did:plc:work"));
    // The event loop gives the worker the session, then:
    let jobs = app.switched_to(work());
    assert!(
        matches!(
            jobs[..],
            [Job::Timeline, Job::Notifications, Job::PinnedFeeds]
        ),
        "{jobs:?}"
    );
    assert_eq!(app.session.as_ref().unwrap().did, "did:plc:work");
    assert!(
        app.timeline.items.is_empty(),
        "the first account's timeline is gone"
    );
    // A reload the first account asked for is not shown for the second.
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
    );
    assert!(app.timeline.items.is_empty());
}

#[test]
fn x_logs_an_account_out_only_after_a_y() {
    let mut app = two_accounts();
    app.handle_key(key('A'));
    app.handle_key(key('j'));
    app.handle_key(key('x'));
    app.handle_key(key('n'));
    assert_eq!(app.take_account_logout(), None);
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("still logged in")
    );
    app.handle_key(key('x'));
    app.handle_key(key('y'));
    assert_eq!(app.take_account_logout().as_deref(), Some("did:plc:work"));
    assert!(app.overlay.is_none());
    // Not the one in use: nothing else changes.
    let jobs = app.logged_out("did:plc:work", Some(session()));
    assert!(jobs.is_empty());
    assert_eq!(app.accounts, vec![Account::from(&session())]);
    assert_eq!(app.timeline.items.len(), 2);
}

#[test]
fn logging_out_the_account_in_use_moves_to_the_next_or_to_the_login_form() {
    let mut app = two_accounts();
    let jobs = app.logged_out("did:plc:me", Some(work()));
    assert!(matches!(jobs[..], [Job::Timeline, ..]), "{jobs:?}");
    assert_eq!(app.session.as_ref().unwrap().did, "did:plc:work");
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("logged out @me.test; now @work.example")
    );
    let jobs = app.logged_out("did:plc:work", None);
    assert!(jobs.is_empty());
    assert!(app.session.is_none());
    let form = app.login.as_ref().expect("login form");
    assert!(
        !form.adding,
        "the last account out: esc quits, as at the start"
    );
}

#[test]
fn another_account_is_logged_in_from_the_list_and_esc_goes_back() {
    let mut app = two_accounts();
    app.handle_key(key('A'));
    app.handle_key(key('a'));
    let form = app.login.as_ref().expect("login form");
    assert!(form.adding);
    assert_eq!(form.fields[0].text(), "https://pds.test");
    app.handle_key(code(KeyCode::Esc));
    assert!(app.login.is_none());
    assert!(!app.quit);
    // Logged in: the account joins the list, and is the one in use.
    app.handle_key(key('A'));
    app.handle_key(key('a'));
    app.handle_event(Event::LoggedIn(Ok(Session {
        did: "did:plc:third".into(),
        handle: "aaa.test".into(),
        ..session()
    })));
    let handles: Vec<_> = app.accounts.iter().map(|a| a.handle.as_str()).collect();
    assert_eq!(handles, ["aaa.test", "me.test", "work.example"]);
    assert_eq!(app.session.as_ref().unwrap().did, "did:plc:third");
}

// D asks, and the question belongs to that moment: when the session
// expires before the answer, the login form takes the keys, and the
// first y after logging in again used to delete the post.
#[test]
fn a_delete_asked_before_the_session_expired_is_called_off() {
    let mine = post("at://did:plc:me/app.bsky.feed.post/m1", "did:plc:me", false);
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(vec![mine].into())));
    app.handle_key(key('D'));
    assert!(app.confirm_delete.is_some());
    expire(&mut app);
    app.handle_event(Event::LoggedIn(Ok(session())));
    let jobs = app.handle_key(key('y'));
    assert!(jobs.is_empty(), "deleted after logging in again: {jobs:?}");
}

// A theme being previewed is not chosen: when the picker is closed by
// the session expiring, the theme goes back to the one in use, as Esc
// would. It used to stay on the previewed one, which settings.json did
// not hold.
#[test]
fn a_theme_previewed_when_the_session_expired_is_not_kept() {
    let mut app = logged_in();
    app.handle_key(key('T'));
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    assert_eq!(app.theme_index, 2);
    expire(&mut app);
    assert!(app.overlay.is_none());
    assert_eq!(app.theme_index, 0);
    assert_eq!(app.theme.name, THEMES[0].name);
}

// The actions list is about the post it was opened on. When that post
// leaves the list meanwhile (a reload without it), the selection moves
// to another post, and enter used to act on that one.
#[test]
fn the_actions_list_does_not_act_on_a_post_it_was_not_opened_on() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    app.handle_key(key('.'));
    assert!(matches!(app.overlay, Some(Overlay::Actions { .. })));
    // A reload that no longer has the second post.
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://a/p/1",
        "did:plc:alice",
        true,
    )]
    .into())));
    // Enter on "reply to it", and l, would act on at://a/p/1.
    let jobs = app.handle_key(key('l'));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert!(app.overlay.is_none(), "{:?}", app.overlay);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("no longer")),
        "{:?}",
        app.status
    );
    // Opened again, it is about the post selected now, and works.
    app.handle_key(key('.'));
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://a/p/1"),
        "{jobs:?}"
    );
}

// The help, the theme picker, and the viewer hold nothing to keep, and a
// playing video must not go on behind the login form.
#[test]
fn an_expired_session_closes_the_help_over_it() {
    let mut app = logged_in();
    app.overlay = Some(Overlay::Help { scroll: 0 });
    app.handle_event(Event::Timeline(Err(Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
    ))));
    assert!(app.login.is_some());
    assert!(app.overlay.is_none());
}

#[test]
fn a_thread_reloading_under_another_still_takes_its_answer() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    app.handle_key(key('R'));
    app.handle_key(key('j'));
    app.handle_key(key('v'));
    assert_eq!(app.threads.len(), 2);
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    assert!(app.threads[0].list.loaded);
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.threads[0].list.items.len(), 3);
}

// A like, a follow, or a notifications "seen" the first account made lands
// after the switch; it is that account's, not the second's, so none of the
// second account's lists show it: the post it liked is not liked for the
// second account, and its l still likes.
#[test]
fn a_write_the_first_account_made_changes_nothing_of_the_second_ones() {
    let mut app = two_accounts();
    let like = press(&mut app, key('l'))[0];
    app.handle_key(key('A'));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.take_account_switch().as_deref(), Some("did:plc:work"));
    let jobs = app.switched_to(work());
    let seqs: Vec<u64> = jobs.iter().map(|j| app.stamp(j)).collect();
    app.handle_answer(
        like,
        Event::Liked {
            post_uri: "at://a/p/1".into(),
            result: Ok("at://did:plc:me/app.bsky.feed.like/l1".into()),
        },
    );
    app.handle_answer(
        seqs[0],
        Event::Timeline(Ok(vec![post("at://a/p/1", "did:plc:alice", true)].into())),
    );
    let p = app
        .timeline
        .items
        .first()
        .expect("the second account's timeline");
    assert!(p.like_uri().is_none(), "liked for the second account");
    assert!(matches!(app.handle_key(key('l'))[..], [Job::Like { .. }]));
}

// The settings screen has the accounts too: its last row opens the list A
// opens, esc goes back to the settings, and a switch closes both.
#[test]
fn the_settings_screen_switches_accounts_as_a_does() {
    let mut app = two_accounts();
    app.handle_key(key('5'));
    app.handle_key(key('s'));
    app.handle_key(key('k'));
    let row = app.settings_rows().pop().unwrap();
    assert_eq!((row.name, row.value.as_str()), ("Account", "@me.test"));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Accounts { selected: 0 })
    ));
    app.handle_key(code(KeyCode::Esc));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 6, .. })
    ));
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.overlay.is_none());
    assert_eq!(app.take_account_switch().as_deref(), Some("did:plc:work"));
    // Opened with A, esc closes it and nothing else comes back.
    app.handle_key(key('A'));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
}
