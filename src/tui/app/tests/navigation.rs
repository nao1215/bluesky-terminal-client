use super::*;

#[test]
fn unfollow_removes_the_author_from_the_timeline() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('f'));
    let Job::Unfollow { did, .. } = &jobs[0] else {
        panic!("{jobs:?}")
    };
    assert_eq!(did, "did:plc:alice");
    app.handle_event(Event::Unfollowed {
        did: "did:plc:alice".into(),
        result: Ok(()),
    });
    assert_eq!(app.timeline.items.len(), 1);
    assert_eq!(app.timeline.items[0].author.did, "did:plc:bob");
}

#[test]
fn search_accounts_and_follow() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    app.handle_key(ctrl('t'));
    type_str(&mut app, "carol");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::SearchActors(q)] if q == "carol"));
    let carol: Profile =
        serde_json::from_value(json!({"did": "did:plc:carol", "handle": "carol.test"})).unwrap();
    app.handle_event(Event::SearchActors {
        query: "carol".into(),
        result: Ok(vec![carol].into()),
    });
    let jobs = app.handle_key(key('f'));
    assert!(matches!(&jobs[..], [Job::Follow { did }] if did == "did:plc:carol"));
    app.handle_event(Event::Followed {
        did: "did:plc:carol".into(),
        result: Ok("at://f".into()),
    });
    assert_eq!(app.search.actors.items[0].following_uri(), Some("at://f"));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:carol"));
    assert_eq!(app.tab, Tab::Profile);
}

#[test]
fn typing_in_the_search_box_does_not_trigger_commands() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "q l f");
    assert!(!app.quit);
    assert_eq!(app.search.input.text(), "q l f");
}

#[test]
fn following_yourself_is_refused() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('5'));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
    let me: Profile =
        serde_json::from_value(json!({"did": "did:plc:me", "handle": "me.test"})).unwrap();
    app.handle_event(Event::Profile(Ok((me, vec![].into()))));
    assert!(app.handle_key(key('f')).is_empty());
    assert!(app.status.as_ref().unwrap().error);
}

#[test]
fn profile_editor_loads_then_saves_the_fields() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    let jobs = app.handle_key(key('e'));
    assert!(matches!(jobs[..], [Job::LoadProfileEditor]));
    app.handle_event(Event::ProfileEditor(Ok(
        crate::tui::worker::ProfileFields {
            display_name: "Me".into(),
            description: "old bio".into(),
        },
    )));
    type_str(&mut app, " Myself");
    app.handle_key(code(KeyCode::Tab));
    app.handle_key(code(KeyCode::Enter));
    type_str(&mut app, "line2");
    let jobs = app.handle_key(ctrl('s'));
    match &jobs[..] {
        [
            Job::SaveProfile {
                display_name,
                description,
                avatar,
            },
        ] => {
            assert_eq!(display_name.as_deref(), Some("Me Myself"));
            assert_eq!(description.as_deref(), Some("old bio\nline2"));
            assert_eq!(avatar, &None);
        }
        other => panic!("{other:?}"),
    }
    let jobs = app.handle_event(Event::ProfileSaved(Ok(())));
    assert!(app.overlay.is_none());
    assert!(matches!(&jobs[..], [Job::OpenProfile(_)]));
}

#[test]
fn editing_someone_elses_profile_is_refused() {
    let mut app = logged_in();
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.profile.actor.as_deref(), Some("did:plc:alice"));
    assert!(app.handle_key(key('e')).is_empty());
    assert!(app.status.as_ref().unwrap().error);
}

#[test]
fn esc_goes_back_to_the_search_the_profile_was_opened_from() {
    let mut app = logged_in();
    app.handle_key(key('3'));
    app.handle_key(ctrl('t'));
    type_str(&mut app, "carol");
    app.handle_key(code(KeyCode::Enter));
    let actors: Vec<Profile> = ["carol", "dave"]
        .iter()
        .map(|n| {
            serde_json::from_value(
                json!({"did": format!("did:plc:{n}"), "handle": format!("{n}.test")}),
            )
            .unwrap()
        })
        .collect();
    app.handle_event(Event::SearchActors {
        query: "carol".into(),
        result: Ok(actors.into()),
    });
    app.handle_key(key('j'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:dave"));
    assert_eq!(app.tab, Tab::Profile);
    // R reloads the profile and still remembers where it came from.
    app.handle_key(key('R'));
    assert_eq!(app.profile.came_from, Some(Tab::Search));
    let jobs = app.handle_key(code(KeyCode::Esc));
    assert!(jobs.is_empty(), "going back fetches nothing: {jobs:?}");
    assert_eq!(app.tab, Tab::Search);
    assert_eq!(app.search.input.text(), "carol");
    assert_eq!(
        app.search.actors.selected, 1,
        "the selection is where it was"
    );
    assert!(!app.search.editing, "back to the results, not to typing");
    // The Profile tab shows the user's own profile next time.
    let jobs = app.handle_key(key('5'));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
}

#[test]
fn esc_goes_back_to_the_timeline_too() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.handle_key(code(KeyCode::Esc)).is_empty());
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(app.timeline.selected, 1);
}

#[test]
fn choosing_a_tab_forgets_the_way_back() {
    let mut app = logged_in();
    app.handle_key(code(KeyCode::Enter)); // alice, from the timeline
    app.handle_key(key('1'));
    app.handle_key(key('5'));
    assert_eq!(app.profile.came_from, None);
    // Esc on someone else's profile with nowhere to go back to: your own.
    let jobs = app.handle_key(code(KeyCode::Esc));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:me"));
}

#[test]
fn pending_counts_jobs_in_flight() {
    let mut app = logged_in();
    // The notifications, loading in the background since the start.
    assert_eq!(app.pending, 1);
    app.handle_key(key('l'));
    assert_eq!(app.pending, 2);
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Err(Error::api("x")),
    });
    assert_eq!(app.pending, 1);
}

#[test]
fn a_second_like_press_waits_for_the_first_answer() {
    let mut app = logged_in();
    assert_eq!(app.handle_key(key('l')).len(), 1);
    // The answer has not arrived: a second press must not create a second record.
    assert!(app.handle_key(key('l')).is_empty());
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Err(Error::api("boom")),
    });
    // Answered (even with an error): the post can be liked again.
    assert_eq!(app.handle_key(key('l')).len(), 1);
}

#[test]
fn a_second_follow_press_waits_for_the_first_answer() {
    let mut app = logged_in();
    assert_eq!(app.handle_key(key('f')).len(), 1);
    assert!(app.handle_key(key('f')).is_empty());
    app.handle_event(Event::Unfollowed {
        did: "did:plc:alice".into(),
        result: Ok(()),
    });
    // Alice's post left the timeline; f now acts on Bob.
    assert!(
        matches!(&app.handle_key(key('f'))[..], [Job::Unfollow { did, .. }] if did == "did:plc:bob")
    );
}

#[test]
fn a_late_editor_answer_does_not_overwrite_typing() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.handle_key(key('e'));
    let fields = || crate::tui::worker::ProfileFields {
        display_name: "Server".into(),
        description: String::new(),
    };
    app.handle_event(Event::ProfileEditor(Ok(fields())));
    app.handle_key(ctrl('u'));
    type_str(&mut app, "Typed");
    // A second answer (from an earlier, abandoned editor) arrives late.
    app.handle_event(Event::ProfileEditor(Ok(fields())));
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!()
    };
    assert_eq!(e.fields[0].text(), "Typed");
}

#[test]
fn an_expired_refresh_token_brings_back_the_login_form() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Err(Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
    ))));
    let form = app.login.as_ref().expect("login form");
    assert_eq!(form.fields[0].text(), "https://pds.test");
    assert_eq!(form.fields[1].text(), "me.test");
    assert_eq!(form.focus, 2);
    assert!(form.error.as_deref().unwrap().contains("expired"));
}

/// The theme picker goes back to the settings screen only when the
/// settings screen opened it. A picker opened from the settings that
/// was closed another way (here the session expiring) used to leave
/// that behind, and the next T then Esc opened the settings screen on
/// whatever tab was shown.
#[test]
fn a_picker_opened_with_t_closes_to_the_list_after_one_from_the_settings() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.handle_key(key('s'));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
    app.handle_event(Event::Timeline(Err(Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
    ))));
    assert!(app.overlay.is_none());
    app.handle_event(Event::LoggedIn(Ok(session())));
    app.handle_key(key('1'));
    app.handle_key(key('T'));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none(), "{:?}", app.overlay);
    app.handle_key(key('T'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.overlay.is_none(), "{:?}", app.overlay);
}

#[test]
fn a_failed_search_settles_even_after_leaving_the_tab() {
    let mut app = logged_in();
    app.handle_key(key('/'));
    type_str(&mut app, "x");
    app.handle_key(code(KeyCode::Enter));
    assert!(!app.search.posts.loaded);
    app.handle_key(key('1'));
    app.handle_event(Event::SearchPosts {
        query: "x".into(),
        result: Err(Error::api("boom")),
    });
    assert!(app.search.posts.loaded);
}

#[test]
fn a_failed_profile_says_why_and_a_stale_one_is_dropped() {
    let mut app = logged_in();
    app.handle_key(code(KeyCode::Enter)); // alice
    app.handle_event(Event::Profile(Err(Error::api("gone"))));
    assert_eq!(app.profile.error.as_deref(), Some("gone"));
    // Bob's profile, opened earlier, answering late, is not shown as Alice's.
    let bob: Profile =
        serde_json::from_value(json!({"did": "did:plc:bob", "handle": "bob.test"})).unwrap();
    app.handle_event(Event::Profile(Ok((bob, vec![].into()))));
    assert!(app.profile.profile.is_none());
    let alice: Profile =
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap();
    app.handle_event(Event::Profile(Ok((alice, vec![].into()))));
    assert_eq!(app.profile.profile.as_ref().unwrap().did, "did:plc:alice");
    assert!(app.profile.error.is_none());
}

#[rstest::rstest]
#[case::digit(KeyEvent::from(KeyCode::Char('3')))]
#[case::tab(KeyEvent::from(KeyCode::Tab))]
#[case::slash(KeyEvent::from(KeyCode::Char('/')))]
fn arriving_at_an_empty_search_tab_types_into_the_box(#[case] arrive: KeyEvent) {
    let mut app = logged_in();
    app.handle_key(arrive);
    // Tab goes by the Chat tab first, which is next to the Timeline.
    if arrive.code == KeyCode::Tab {
        app.handle_key(arrive);
    }
    assert_eq!(app.tab, Tab::Search);
    // q, l and f would quit, like and follow on a result list.
    type_str(&mut app, "q l f");
    assert!(!app.quit);
    assert_eq!(app.search.input.text(), "q l f");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::SearchPosts(q)] if q == "q l f"));
    // After Enter the results have the keys: j moves, it is not typed.
    app.handle_event(Event::SearchPosts {
        query: "q l f".into(),
        result: Ok(vec![
            post("at://s/1", "did:plc:x", false),
            post("at://s/2", "did:plc:y", false),
        ]
        .into()),
    });
    app.handle_key(key('j'));
    assert_eq!(app.search.posts.selected, 1);
    assert_eq!(app.search.input.text(), "q l f");
}

#[test]
fn backtab_arrives_at_search_focused_too() {
    let mut app = logged_in();
    // From the Timeline back round Profile and Notifications, to Search.
    for _ in 0..3 {
        app.handle_key(code(KeyCode::BackTab));
    }
    assert_eq!(app.tab, Tab::Search);
    assert!(app.search.editing);
}

#[test]
fn coming_back_to_a_search_with_results_leaves_the_keys_to_the_results() {
    let mut app = logged_in();
    app.handle_key(key('3'));
    type_str(&mut app, "rust");
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('1'));
    app.handle_key(key('3'));
    assert!(!app.search.editing, "j/k must move through the results");
    // i (or /) goes back to typing.
    app.handle_key(key('i'));
    assert!(app.search.editing);
    type_str(&mut app, "!");
    assert_eq!(app.search.input.text(), "rust!");
}

#[test]
fn tab_leaves_the_search_box_for_the_next_tab() {
    let mut app = logged_in();
    app.handle_key(key('3'));
    type_str(&mut app, "abc");
    app.handle_key(code(KeyCode::Tab));
    assert_eq!(app.tab, Tab::Notifications);
    assert!(!app.search.editing);
    assert_eq!(app.search.input.text(), "abc");
}

#[test]
fn paste_reaches_the_focused_search_box() {
    let mut app = logged_in();
    app.handle_key(key('3'));
    app.handle_paste("pasted words");
    assert_eq!(app.search.input.text(), "pasted words");
}

#[test]
fn tabs_wrap_in_both_directions() {
    assert_eq!(Tab::Timeline.next(-1), *Tab::ALL.last().unwrap());
    assert_eq!(Tab::ALL.last().unwrap().next(1), Tab::Timeline);
}

#[test]
fn status_messages_expire_errors_later() {
    let mut app = logged_in();
    app.info("liked");
    let at = app.status.as_ref().unwrap().at;
    assert!(!app.expire_status(at + STATUS_TTL - Duration::from_millis(1)));
    assert!(app.expire_status(at + STATUS_TTL));
    assert!(app.status.is_none());
    app.error("boom");
    let at = app.status.as_ref().unwrap().at;
    assert!(!app.expire_status(at + STATUS_TTL));
    assert!(app.expire_status(at + ERROR_TTL));
    // Nothing to expire: the screen does not change.
    assert!(!app.expire_status(at + ERROR_TTL));
}

#[test]
fn help_scrolls_and_only_closes_on_purpose() {
    let mut app = logged_in();
    app.handle_key(key('?'));
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    app.handle_key(key('k'));
    assert!(matches!(app.overlay, Some(Overlay::Help { scroll: 1 })));
    // A stray key is not a request to close.
    app.handle_key(key('x'));
    assert!(app.overlay.is_some());
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    app.handle_key(key('?'));
    app.handle_key(key('?'));
    assert!(app.overlay.is_none());
}

#[test]
fn the_theme_picker_previews_and_esc_goes_back() {
    let mut app = logged_in();
    app.handle_key(key('T'));
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    assert_eq!(app.theme.name, THEMES[2].name, "moving previews the theme");
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert_eq!(app.theme_index, 0);
    assert!(
        app.take_settings_save().is_none(),
        "cancelling saves nothing"
    );
}

// The requests of the start all find the session expired, and their
// answers come one by one. The login form the first one brought up stays as
// the user fills it: the next ones do not bring up an empty one over it.
#[test]
fn a_second_expired_answer_keeps_the_login_form_being_filled() {
    let mut app = logged_in();
    let expired = || {
        Error::api("com.atproto.server.refreshSession failed: ExpiredToken: Token has been revoked")
    };
    app.handle_event(Event::Timeline(Err(expired())));
    assert!(app.login.is_some());
    type_str(&mut app, "app-pass-🔑-1234");
    app.handle_event(Event::PinnedFeeds(Err(expired())));
    app.handle_event(Event::Notifications {
        seen_at: String::new(),
        result: Err(expired()),
    });
    let form = app.login.as_ref().unwrap();
    assert_eq!(form.fields[form.focus].text(), "app-pass-🔑-1234");
}

// Once logged in again, an answer to a request sent with the old tokens
// that says the session expired is old news: it does not ask for a login
// again.
#[test]
fn an_expired_answer_from_before_logging_in_again_is_old_news() {
    let mut app = logged_in();
    let expired = || {
        Error::api("com.atproto.server.refreshSession failed: ExpiredToken: Token has been revoked")
    };
    // Two loads out with the old tokens.
    let first = press(&mut app, key('R'));
    let second = press(&mut app, key('R'));
    assert_eq!((first.len(), second.len()), (1, 1));
    app.handle_answer(first[0], Event::Timeline(Err(expired())));
    assert!(app.login.is_some());
    type_str(&mut app, "app-pass-1234");
    let login = press(&mut app, code(KeyCode::Enter));
    assert_eq!(login.len(), 1);
    app.handle_answer(login[0], Event::LoggedIn(Ok(session())));
    assert!(app.login.is_none());
    app.handle_answer(second[0], Event::Timeline(Err(expired())));
    assert!(
        app.login.is_none(),
        "an old answer brought the login form back"
    );
}

// A profile left for another before it came: its error is not the other
// one's, which goes on loading without an error box.
#[test]
fn the_error_of_a_profile_left_is_not_shown_on_the_next() {
    let mut app = logged_in();
    let alice = press(&mut app, code(KeyCode::Enter));
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('j'));
    let bob = press(&mut app, code(KeyCode::Enter));
    assert_eq!((alice.len(), bob.len()), (1, 1));
    app.handle_answer(
        alice[0],
        Event::Profile(Err(Error::api(
            "app.bsky.actor.getProfile failed: Profile not found",
        ))),
    );
    assert!(app.profile.loading);
    assert!(app.profile.error.is_none());
    assert!(app.status.is_none(), "{:?}", app.status);
}

// A new search that fails does not page on from the results of the one
// before it.
#[test]
fn a_failed_search_does_not_page_the_results_of_the_one_before() {
    let mut app = logged_in();
    app.handle_key(key('3'));
    type_str(&mut app, "cats");
    app.handle_key(code(KeyCode::Enter));
    let posts: Vec<Post> = (0..3)
        .map(|i| post(&format!("at://cat/{i}"), "did:plc:c", false))
        .collect();
    app.handle_event(Event::SearchPosts {
        query: "cats".into(),
        result: Ok(Page {
            items: posts,
            cursor: Some("cats-2".into()),
        }),
    });
    app.handle_key(key('/'));
    for _ in 0..4 {
        app.handle_key(code(KeyCode::Backspace));
    }
    type_str(&mut app, "dogs");
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::SearchPosts {
        query: "dogs".into(),
        result: Err(Error::api("app.bsky.feed.searchPosts failed: HTTP 502")),
    });
    let mut asked = Vec::new();
    for _ in 0..3 {
        asked.extend(app.handle_key(key('j')));
    }
    assert!(
        !asked.iter().any(|j| matches!(j, Job::More { feed: Feed::SearchPosts(q), cursor } if q == "dogs" && cursor == "cats-2")),
        "the cats results' cursor is used to page the dogs query: {asked:?}"
    );
}

// R on a profile reads it again where the reader is, as it does on every
// other list: the posts stay while it loads, and the selection stays on the
// post it was on.
#[test]
fn reloading_a_profile_keeps_the_place_in_its_posts() {
    let mut app = logged_in();
    app.handle_key(code(KeyCode::Enter)); // alice
    let alice: Profile =
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap();
    let posts: Vec<Post> = (0..5)
        .map(|i| post(&format!("at://alice/{i}"), "did:plc:alice", true))
        .collect();
    app.handle_event(Event::Profile(Ok((alice.clone(), posts.clone().into()))));
    for _ in 0..3 {
        app.handle_key(key('j'));
    }
    let jobs = app.handle_key(key('R'));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:alice"));
    assert_eq!(
        app.profile.posts.items.len(),
        5,
        "the posts stay while it loads"
    );
    assert_eq!(app.profile.came_from, Some(Tab::Timeline));
    app.handle_event(Event::Profile(Ok((alice, posts.into()))));
    assert_eq!(app.profile.posts.current().unwrap().uri, "at://alice/3");
}
