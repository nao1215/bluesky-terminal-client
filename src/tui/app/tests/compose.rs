use super::*;

#[test]
fn reply_sends_the_thread_reference() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    app.handle_key(key('r'));
    type_str(&mut app, "hello");
    let jobs = app.handle_key(ctrl('s'));
    match &jobs[..] {
        [
            Job::Post {
                text,
                reply: Some(r),
                media,
                ..
            },
        ] => {
            assert_eq!(text, "hello");
            assert_eq!(r.parent.uri, "at://b/p/2");
            assert_eq!(r.root.uri, "at://b/p/2");
            assert!(media.is_empty());
        }
        other => panic!("{other:?}"),
    }
    app.handle_event(Event::Posted {
        reply_to: Some("at://b/p/2".into()),
        result: Ok(()),
    });
    assert!(app.overlay.is_none());
    assert_eq!(app.timeline.items[1].reply_count, 1);
}

#[test]
fn composer_refuses_empty_and_too_long_posts() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert!(app.status.as_ref().unwrap().error);
    type_str(&mut app, &"あ".repeat(crate::api::MAX_POST_GRAPHEMES + 1));
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert!(app.status.as_ref().unwrap().text.contains("301"));
}

#[test]
fn composer_refuses_a_post_of_emoji_over_the_byte_limit() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    // 121 families: 121 characters, 3025 bytes.
    app.handle_paste(&"👨\u{200d}👩\u{200d}👧\u{200d}👦".repeat(121));
    assert!(app.handle_key(ctrl('s')).is_empty());
    let status = app.status.as_ref().unwrap();
    assert!(
        status.error && status.text.contains("3025 bytes"),
        "{}",
        status.text
    );
    // One fewer fits both limits and is sent.
    app.handle_key(KeyEvent::from(KeyCode::Backspace));
    assert!(matches!(app.handle_key(ctrl('s'))[..], [Job::Post { .. }]));
}

/// Replying reloads the timeline so your own post shows up; the
/// selection stays on the post it was on, wherever that post now is,
/// instead of jumping back to the first post.
#[test]
fn a_reload_after_replying_keeps_the_selected_post() {
    let mut app = logged_in();
    app.handle_key(key('j'));
    assert_eq!(app.timeline.current().unwrap().uri, "at://b/p/2");
    app.handle_key(key('r'));
    type_str(&mut app, "Good point");
    let jobs = app.handle_event(Event::Posted {
        reply_to: Some("at://b/p/2".into()),
        result: Ok(()),
    });
    assert!(matches!(jobs[..], [Job::Timeline]));
    // The reload has your reply on top and Bob's post one further down.
    app.handle_event(Event::Timeline(Ok(vec![
        post("at://me/p/3", "did:plc:me", false),
        post("at://a/p/1", "did:plc:alice", true),
        post("at://b/p/2", "did:plc:bob", true),
    ]
    .into())));
    assert_eq!(app.timeline.current().unwrap().uri, "at://b/p/2");
    // A post that is gone leaves the selection at the top.
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://a/p/1",
        "did:plc:alice",
        true,
    )]
    .into())));
    assert_eq!(app.timeline.selected, 0);
}

// was: the login form the expired session brings up dropped the
// composer, so the draft was gone after logging in again.
#[test]
fn a_draft_survives_the_session_expiring_while_it_is_sent() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    type_str(&mut app, "a long draft");
    app.handle_key(ctrl('s'));
    app.handle_event(Event::Posted {
        reply_to: None,
        result: Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        )),
    });
    assert!(app.login.is_some(), "the login form is up");
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("the composer was dropped")
    };
    assert_eq!(c.input.text(), "a long draft");
    assert!(!c.sending);
    // The login form takes the keys while it is up.
    app.handle_key(key('x'));
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!()
    };
    assert_eq!(c.input.text(), "a long draft");
    app.handle_event(Event::LoggedIn(Ok(session())));
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("the draft was lost on logging in again")
    };
    assert_eq!(c.input.text(), "a long draft");
    // And it can be sent again.
    let jobs = app.handle_key(ctrl('s'));
    assert!(matches!(&jobs[..], [Job::Post { .. }]), "{jobs:?}");
}

#[test]
fn a_profile_editor_survives_the_session_expiring_while_it_is_saved() {
    let mut app = logged_in();
    app.overlay = Some(Overlay::EditProfile(EditProfile {
        fields: [
            TextInput::single("My new name"),
            TextInput::multi("About me"),
            TextInput::single(""),
        ],
        focus: 0,
        loading: false,
        saving: true,
        browser: None,
        avatar_chosen: None,
        loaded: Default::default(),
    }));
    app.handle_event(Event::ProfileSaved(Err(Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
    ))));
    assert!(app.login.is_some());
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!("the editor was dropped")
    };
    assert_eq!(e.fields[0].text(), "My new name");
    assert!(!e.saving);
    app.handle_event(Event::LoggedIn(Ok(session())));
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!("the editor was lost on logging in again")
    };
    assert_eq!(e.fields[1].text(), "About me");
}

// Another account has nothing to do with the draft: it goes, as the rest
// of the first account's state does.
#[test]
fn logging_in_as_someone_else_drops_the_draft() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    type_str(&mut app, "a long draft");
    app.handle_key(ctrl('s'));
    app.handle_event(Event::Posted {
        reply_to: None,
        result: Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
        )),
    });
    app.handle_event(Event::LoggedIn(Ok(Session {
        did: "did:plc:other".into(),
        handle: "other.test".into(),
        ..session()
    })));
    assert!(app.overlay.is_none());
}

#[test]
fn pictures_are_chosen_described_and_sent_without_text() {
    let dir = pictures();
    let mut app = composer_with_pictures(&dir);
    assert_eq!(app.browse_from.as_deref(), Some(dir.path()), "remembered");
    app.handle_key(code(KeyCode::Tab));
    type_str(&mut app, "first");
    app.handle_key(code(KeyCode::Tab));
    app.handle_paste("second");
    // Tab comes back round to the text, which stays empty.
    app.handle_key(code(KeyCode::Tab));
    assert_eq!(composer(&app).focus, 0);
    let jobs = app.handle_key(ctrl('s'));
    let [Job::Post { text, media, .. }] = &jobs[..] else {
        panic!("{jobs:?}")
    };
    assert_eq!(text, "");
    assert_eq!(
        media,
        &[
            Attachment {
                path: dir.path().join("a.png"),
                alt: "first".into()
            },
            Attachment {
                path: dir.path().join("b.png"),
                alt: "second".into()
            },
        ]
    );
}

#[test]
fn ctrl_x_removes_the_picture_being_described_or_the_last() {
    let dir = pictures();
    let mut app = composer_with_pictures(&dir);
    app.handle_key(code(KeyCode::Tab));
    app.handle_key(ctrl('x'));
    let c = composer(&app);
    assert_eq!(c.media.len(), 1);
    assert!(c.media[0].path.ends_with("b.png"));
    assert_eq!(c.focus, 1, "on the picture that moved up");
    app.handle_key(code(KeyCode::BackTab));
    app.handle_key(ctrl('x'));
    assert!(composer(&app).media.is_empty());
    assert!(
        app.handle_key(ctrl('x')).is_empty(),
        "nothing left to remove"
    );
    // Without pictures or text there is nothing to post.
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert_eq!(app.status.as_ref().unwrap().text, "the post is empty");
}

#[test]
fn a_fifth_picture_is_refused_and_the_browser_holds_the_keys() {
    let dir = pictures();
    let mut app = composer_with_pictures(&dir);
    app.handle_key(ctrl('o'));
    assert_eq!(composer(&app).browser.as_ref().unwrap().room, 2);
    // Typing and pasting do not reach the post behind the browser.
    app.handle_paste("hidden");
    app.handle_key(key('z'));
    app.handle_key(code(KeyCode::Esc));
    assert!(composer(&app).browser.is_none());
    assert!(composer(&app).input.is_empty());
    let Some(Overlay::Compose(c)) = &mut app.overlay else {
        unreachable!()
    };
    let one = c.media[0].clone();
    c.media.extend([one.clone(), one]);
    app.handle_key(ctrl('o'));
    assert!(composer(&app).browser.is_none());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("at most 4 pictures")
    );
}

#[test]
fn the_avatar_is_chosen_in_the_browser() {
    let dir = pictures();
    let mut app = logged_in();
    app.browse_from = Some(dir.path().to_path_buf());
    app.overlay = Some(Overlay::EditProfile(EditProfile {
        fields: [
            TextInput::single("Me"),
            TextInput::multi(""),
            TextInput::single(""),
        ],
        focus: 0,
        loading: false,
        saving: false,
        browser: None,
        avatar_chosen: None,
        loaded: Default::default(),
    }));
    app.handle_key(ctrl('o'));
    app.handle_key(key('G'));
    app.handle_key(code(KeyCode::Enter));
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!()
    };
    assert!(e.browser.is_none());
    assert_eq!(e.focus, 2);
    assert_eq!(
        e.fields[2].text(),
        dir.path().join("b.png").display().to_string()
    );
}

#[test]
fn one_video_goes_alone_and_nothing_joins_it() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    let Some(Overlay::Compose(c)) = &mut app.overlay else {
        unreachable!()
    };
    c.media.push(Attached::new(testdata("clip.mp4")));
    assert!(c.media[0].is_video());
    assert_eq!(c.media[0].info.dims, Some((64, 36)));
    app.handle_key(ctrl('o'));
    assert!(composer(&app).browser.is_none());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("a video can have nothing else")
    );
    let jobs = app.handle_key(ctrl('s'));
    let [Job::Post { media, .. }] = &jobs[..] else {
        panic!("{jobs:?}")
    };
    assert_eq!(media[0].path, testdata("clip.mp4"));
}

#[test]
fn pictures_and_a_video_cannot_share_a_post() {
    let photo = Attached::new(testdata("photo.png"));
    let clip = Attached::new(testdata("clip.mp4"));
    let gif = Attached::new(testdata("moving.gif"));
    assert!(gif.is_video() && gif.info.animated_gif);
    assert_eq!(media_problem(std::slice::from_ref(&clip)), None);
    assert_eq!(media_problem(&[photo.clone(), photo.clone()]), None);
    let mixed = media_problem(&[photo.clone(), clip.clone()]).unwrap();
    assert!(mixed.contains("up to 4 pictures or one video"), "{mixed}");
    assert!(media_problem(&[clip.clone(), gif]).is_some());
    let five = vec![photo; 5];
    assert!(media_problem(&five).unwrap().contains("at most 4 pictures"));
}

#[test]
fn notifications_loaded_at_start_are_seen_only_when_their_tab_is() {
    let mut app = logged_in();
    let jobs = app.handle_event(Event::Notifications {
        seen_at: "2026-09-22T01:00:00.000Z".into(),
        result: Ok(vec![notif("reply", "at://r", false, None, None)].into()),
    });
    assert!(jobs.is_empty(), "not seen from the timeline: {jobs:?}");
    assert_eq!(app.unread, 1, "but counted on the tab");
    let jobs = app.handle_key(key('4'));
    assert!(matches!(&jobs[..], [Job::UpdateSeen(at)] if at == "2026-09-22T01:00:00.000Z"));
    app.handle_key(key('1'));
    assert!(app.handle_key(key('4')).is_empty(), "marked once");
}

// The error box closes on any key. In the composer Esc only closes it:
// it would otherwise throw the draft away too.
#[test]
fn a_key_that_closes_an_error_leaves_the_draft_alone() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    let long: String = "家族👨\u{200d}👩\u{200d}👧 ".repeat(80);
    app.handle_paste(&long);
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert!(app.status.as_ref().is_some_and(|s| s.error));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.status.is_none(), "the error is closed");
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("the draft was thrown away")
    };
    assert_eq!(c.input.text(), long);
    // With the box closed, Esc closes the composer as before.
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
}

// A paste while the post is on its way would show in the box and be lost
// when the answer closes it; it is not taken.
#[test]
fn a_paste_while_the_post_is_sent_is_not_taken() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    type_str(&mut app, "hello");
    assert_eq!(app.handle_key(ctrl('s')).len(), 1);
    app.handle_paste(" world 🇯🇵");
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!()
    };
    assert_eq!(c.input.text(), "hello");
}

// A profile the editor did not change is not rewritten: a description
// with a tab, as another client wrote it, is sent back only when edited.
#[test]
fn the_profile_editor_sends_only_the_fields_that_were_changed() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.handle_key(key('e'));
    app.handle_paste("pasted while loading");
    app.handle_event(Event::ProfileEditor(Ok(
        crate::tui::worker::ProfileFields {
            display_name: "Me 🌸".into(),
            description: "col1\tcol2\n".into(),
        },
    )));
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!()
    };
    assert_eq!(
        e.fields[0].text(),
        "Me 🌸",
        "the paste while loading was not taken"
    );
    let jobs = app.handle_key(ctrl('s'));
    assert!(
        matches!(
            &jobs[..],
            [Job::SaveProfile {
                display_name: None,
                description: None,
                avatar: None
            }]
        ),
        "{jobs:?}"
    );
}
