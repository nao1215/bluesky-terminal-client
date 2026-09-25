use super::*;

/// Your own profile says that e and s exist, as buttons; someone
/// else's has neither.
#[test]
fn your_own_profile_shows_the_edit_and_settings_buttons() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('5'),
    ));
    app.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
    let screen = render(&mut app, 100, 24);
    assert!(
        screen.contains("this is you  [ e Edit profile ]  [ s Settings ]"),
        "{screen}"
    );
    assert!(screen.contains("s settings"), "{screen}");
    // Still there with the screen open over it.
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('s'),
    ));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("[ s Settings ]"), "{screen}");
    assert!(screen.contains(" Settings "), "{screen}");
}

/// The screen names every setting with its value, and says where the
/// selected one comes from.
#[test]
fn the_settings_screen_lists_each_setting_and_where_it_comes_from() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.env.graphics = Some("kitty".into());
    app.env.browser = Some("firefox".into());
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('5'),
    ));
    app.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('s'),
    ));
    let screen = render(&mut app, 100, 30);
    for row in app.settings_rows() {
        assert!(screen.contains(row.name), "{}:\n{screen}", row.name);
    }
    assert!(screen.contains("▶ Theme"), "{screen}");
    assert!(screen.contains("bluesky"), "{screen}");
    assert!(screen.contains("Pictures             kitty"), "{screen}");
    assert!(screen.contains("Browser              firefox"), "{screen}");
    assert!(
        screen.contains("enter chooses one from the list"),
        "{screen}"
    );
    assert!(
        screen.contains("j k move  enter change  esc close"),
        "{screen}"
    );
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('j'),
    ));
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("set by BSKY_GRAPHICS for this run"),
        "{screen}"
    );
}

/// Typing a setting shows the line it is typed in, and choosing a
/// folder shows the folder browser, which lists folders only.
#[test]
fn a_setting_is_typed_on_its_own_line_and_a_folder_chosen_in_the_browser() {
    use crossterm::event::{KeyCode, KeyEvent};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("写真👨\u{200d}👩\u{200d}👧")).unwrap();
    std::fs::write(dir.path().join("photo.png"), "not listed").unwrap();
    let (mut app, _) = App::new(Some(session()), "x");
    app.settings.download_dir = Some(dir.path().display().to_string());
    app.handle_key(KeyEvent::from(KeyCode::Char('5')));
    app.handle_key(KeyEvent::from(KeyCode::Char('s')));
    for _ in 0..4 {
        app.handle_key(KeyEvent::from(KeyCode::Char('j')));
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("enter keeps it, empty is the default, esc cancels"),
        "{screen}"
    );
    assert!(screen.contains("https://video.bsky.app"), "{screen}");
    assert!(screen.contains("enter keep  esc cancel"), "{screen}");
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    for _ in 0..2 {
        app.handle_key(KeyEvent::from(KeyCode::Char('k')));
    }
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("x goes back to the default"), "{screen}");
    assert!(screen.contains("x default"), "{screen}");
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let screen = render_text_only(&mut app, 100, 30);
    assert!(screen.contains("Choose a folder"), "{screen}");
    assert!(screen.contains("写真👨\u{200d}👩\u{200d}👧/"), "{screen}");
    assert!(!screen.contains("photo.png"), "{screen}");
    assert!(
        screen.contains("enter open  space choose this folder  n new folder"),
        "{screen}"
    );
    // n names a new folder on the bottom row, typed or pasted.
    app.handle_key(KeyEvent::from(KeyCode::Char('n')));
    for c in "新しい ".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.handle_paste("🏔️👨\u{200d}👩\u{200d}👧");
    let hints = crate::tui::keys::hints(&app);
    assert_eq!(hints, [("enter", "make it"), ("esc", "cancel")]);
    let screen = render_text_only(&mut app, 100, 30);
    assert!(
        screen.contains("New folder: 新しい 🏔️👨\u{200d}👩\u{200d}👧"),
        "{screen}"
    );
    // A name that cannot be a folder's says why above it.
    app.handle_paste("/");
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let screen = render_text_only(&mut app, 100, 30);
    assert!(
        screen.contains("a folder name cannot contain / or \\"),
        "{screen}"
    );
    app.handle_key(KeyEvent::from(KeyCode::Backspace));
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let made = dir.path().join("新しい 🏔️👨\u{200d}👩\u{200d}👧");
    assert!(made.is_dir());
    app.handle_key(KeyEvent::from(KeyCode::Char(' ')));
    assert_eq!(app.settings.download_dir, Some(made.display().to_string()));
}

/// A long path keeps its end, where the folder's own name is, and the
/// selected row stays on a screen too short for the whole list.
#[test]
fn a_short_screen_keeps_the_selected_setting_in_sight() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.env.download_dir = Some(format!(
        "/very/{}/写真👨\u{200d}👩\u{200d}👧",
        "long/".repeat(20)
    ));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('5'),
    ));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('s'),
    ));
    for _ in 0..5 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
    }
    let screen = render_text_only(&mut app, 40, MIN_H);
    assert!(screen.contains("▶ Browser"), "{screen}");
    // Past the last rows, Language and Account, round to the third.
    for _ in 0..5 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
    }
    let screen = render_text_only(&mut app, 40, 12);
    assert!(screen.contains("▶ Download folder"), "{screen}");
    assert!(screen.contains("写真👨\u{200d}👩\u{200d}👧"), "{screen}");
}

#[test]
fn the_account_list_marks_the_one_in_use_and_the_tabs_name_it() {
    use crate::tui::app::Account;
    let (mut app, _) = App::new(Some(session()), "x");
    let me = Account {
        did: "did:plc:me".into(),
        handle: "me.test".into(),
    };
    // One account: the tab bar names nobody.
    app.accounts = vec![me];
    let screen = render(&mut app, 100, 24);
    assert!(
        !screen.lines().next().unwrap().contains("@me.test"),
        "{screen}"
    );
    app.accounts.push(Account {
        did: "did:plc:w".into(),
        handle: "work.example".into(),
    });
    let screen = render(&mut app, 100, 24);
    assert!(
        screen.lines().next().unwrap().ends_with("@me.test"),
        "{screen}"
    );
    // Too narrow for the name whole: left out, not cut.
    let narrow = render(&mut app, 60, 24);
    assert!(!narrow.lines().next().unwrap().contains("@me"), "{narrow}");
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('A'),
    ));
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains(" Accounts "), "{screen}");
    assert!(screen.contains("▶ @me.test  in use"), "{screen}");
    assert!(screen.contains("  @work.example"), "{screen}");
    assert!(screen.contains("a log in another  x log out"), "{screen}");
    assert!(
        screen.contains("enter use  a add  x log out  esc close"),
        "{screen}"
    );
}

/// As many columns as fit side by side, the focused one among them,
/// and how many are off screen said at the edges.
#[test]
fn the_columns_fit_the_width_and_say_how_many_are_off_screen() {
    let mut app = column_app(4);
    let screen = render(&mut app, 110, 20);
    let head = screen.lines().nth(1).unwrap();
    assert!(head.contains("Search: q0"), "{screen}");
    assert!(head.contains("Search: q2 1›"), "{screen}");
    assert!(!head.contains("q3"), "{screen}");
    assert_eq!(screen.matches("post number 0").count(), 3, "{screen}");
    for _ in 0..3 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Right,
        ));
    }
    let screen = render(&mut app, 110, 20);
    let head = screen.lines().nth(1).unwrap();
    assert!(head.contains("‹1 Search: q1"), "{screen}");
    assert!(head.contains("Search: q3"), "{screen}");
    assert!(screen.contains("← → column"), "{screen}");
    // One column on a narrow screen.
    let narrow = render(&mut app, 40, 20);
    assert!(
        narrow.lines().nth(1).unwrap().contains("‹3 Search: q3"),
        "{narrow}"
    );
    // None: the timeline alone, as it was before any.
    let mut empty = column_app(0);
    let screen = render(&mut empty, 80, 20);
    assert!(!screen.contains("column"), "{screen}");
}

/// A long title is shortened, not the count of columns off screen after it.
#[test]
fn a_long_column_title_keeps_the_count_of_columns_off_screen() {
    let mut app = column_app(3);
    for i in [0, 1] {
        app.columns.items[i].source = crate::config::ColumnSource::Search {
            query: "家族👨‍👩‍👧 and 🇯🇵 a rather long search query here".into(),
        };
    }
    let screen = render(&mut app, 80, 12);
    let head = screen.lines().nth(1).unwrap();
    assert!(head.contains("… 1›"), "{screen}");
    for _ in 0..2 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Right,
        ));
    }
    let narrow = render(&mut app, 30, 12);
    assert!(
        narrow.lines().nth(1).unwrap().contains("‹2 Search: q2"),
        "{narrow}"
    );
}

#[test]
fn the_add_column_list_names_every_source_and_asks_for_a_search() {
    let mut app = column_app(0);
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('+'),
    ));
    let screen = render(&mut app, 100, 24);
    for want in [
        "Add a column",
        "▶ Following",
        "Notifications",
        "Your posts (@me.test)",
        "Search…",
    ] {
        assert!(screen.contains(want), "{want}:\n{screen}");
    }
    for _ in 0..3 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
    }
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Enter,
    ));
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains("Search posts for:"), "{screen}");
    assert!(screen.contains("enter add  esc back"), "{screen}");
}

/// The key column is 18 cells wide, which leaves nothing for the
/// description on a narrow screen: the help was the one screen that
/// could not be read where it is needed most.
#[test]
fn help_on_a_narrow_screen_puts_each_description_under_its_keys() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(1).into())));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('?'),
    ));
    let narrow = render(&mut app, MIN_W, 16);
    assert!(narrow.contains("1 2 3 4"), "{narrow}");
    // The words are whole and on their own rows, not cut to four cells.
    assert!(narrow.contains("the tabs,"), "{narrow}");
    assert!(narrow.contains("order shown"), "{narrow}");
    assert!(!narrow.contains("Time\n"), "{narrow}");
    // The two columns come back where they fit, and a description too
    // long for its column wraps rather than losing its end.
    let forty = render(&mut app, 40, 16);
    assert!(
        forty
            .lines()
            .any(|l| l.contains("1 2 3 4 5") && l.contains("the tabs,")),
        "{forty}"
    );
    assert!(
        forty
            .lines()
            .any(|l| l.contains("shown") && !l.contains("1 2 3 4 5")),
        "{forty}"
    );
    // Wide enough, every description is one line.
    let wide = render(&mut app, 80, 24);
    assert!(
        wide.lines()
            .any(|l| l.contains("1 2 3 4 5") && l.contains("the tabs, in the order shown")),
        "{wide}"
    );
}

/// A conversation shorter than the screen sits just above the input
/// line, newest last, as it does once it fills the screen.
#[test]
fn a_short_conversation_sits_above_the_input_line() {
    use crossterm::event::{KeyCode, KeyEvent};
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_key(KeyEvent::from(KeyCode::Char('2')));
    let convo: crate::api::types::Convo = serde_json::from_value(json!({
        "id": "c", "rev": "r",
        "members": [{"did": "did:plc:me", "handle": "me.test"},
                    {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"}],
        "muted": false, "unreadCount": 0
    }))
    .unwrap();
    app.handle_event(Event::Convos {
        cursor: None,
        result: Ok(vec![convo].into()),
    });
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    app.handle_event(Event::Messages {
        convo_id: "c".into(),
        cursor: None,
        result: Ok(vec![crate::api::types::ChatMessage {
            id: "m".into(),
            text: "see you at 7 🌤".into(),
            sender: "did:plc:a".into(),
            sent_at: "2026-09-22T00:00:00Z".into(),
            ..Default::default()
        }]
        .into()),
    });
    let screen = render(&mut app, 80, 20);
    let rows: Vec<&str> = screen.lines().collect();
    let input = rows
        .iter()
        .position(|r| r.contains("i write a message"))
        .expect(&screen);
    assert!(rows[input - 1].contains("see you at 7 🌤"), "{screen}");
    assert!(rows[input - 2].contains("Alice"), "{screen}");
    assert!(
        rows[1].contains("Alice"),
        "the title stays on top: {screen}"
    );
    assert!(rows[2].trim().is_empty(), "{screen}");
}

/// Someone's profile says when you muted or blocked them, and the actions
/// list offers to undo it; your own offers neither.
#[test]
fn a_profile_says_muted_and_blocked_and_the_list_offers_to_undo_it() {
    use crossterm::event::{KeyCode, KeyEvent};
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_key(KeyEvent::from(KeyCode::Char('5')));
    app.profile.actor = Some("did:plc:alice".into());
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({
            "did": "did:plc:alice", "handle": "alice.test",
            "displayName": "家族👨\u{200d}👩\u{200d}👧 Alice 🇯🇵",
            "viewer": {"muted": true, "blocking": "at://did:plc:me/app.bsky.graph.block/b1"}
        }))
        .unwrap(),
        Vec::new().into(),
    ))));
    let screen = render(&mut app, 100, 24);
    assert!(
        screen.contains("not following  · muted  · blocked"),
        "{screen}"
    );
    app.handle_key(KeyEvent::from(KeyCode::Char('.')));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("M      unmute them"), "{screen}");
    assert!(screen.contains("B      unblock them"), "{screen}");
    // Your own profile: nothing to mute or block.
    let (mut own, _) = App::new(Some(session()), "x");
    own.handle_key(KeyEvent::from(KeyCode::Char('5')));
    own.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
    own.handle_key(KeyEvent::from(KeyCode::Char('.')));
    let screen = render(&mut own, 100, 30);
    assert!(!screen.contains("mute them"), "{screen}");
}

/// A mute by one of your lists says which list; the list offers your own
/// mute, since M does not undo the list's.
#[test]
fn a_profile_muted_by_a_list_names_the_list() {
    use crossterm::event::{KeyCode, KeyEvent};
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_key(KeyEvent::from(KeyCode::Char('5')));
    app.profile.actor = Some("did:plc:alice".into());
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({
            "did": "did:plc:alice", "handle": "alice.test",
            "viewer": {"muted": true, "mutedByList": {"uri": "at://x", "name": "Spam 🚫 仲間"}}
        }))
        .unwrap(),
        Vec::new().into(),
    ))));
    let screen = render_text_only(&mut app, 100, 24);
    assert!(
        screen.contains("· muted by the list Spam 🚫 仲間"),
        "{screen}"
    );
    app.handle_key(KeyEvent::from(KeyCode::Char('.')));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("M      mute them"), "{screen}");
}

// k held at the top of a long conversation asks for the older messages,
// however many lines the ones shown take.
#[test]
fn k_at_the_top_of_a_long_conversation_loads_older_messages() {
    let (mut a, _) = App::new(Some(session()), "x");
    a.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('2'),
    ));
    let convo: crate::api::types::Convo = serde_json::from_value(json!({
        "id": "c", "rev": "r",
        "members": [{"did": "did:plc:a", "handle": "alice.test"}],
        "muted": false, "unreadCount": 0
    }))
    .unwrap();
    a.handle_event(Event::Convos {
        cursor: None,
        result: Ok(vec![convo].into()),
    });
    a.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Enter,
    ));
    let msgs: Vec<crate::api::types::ChatMessage> = (0..50)
        .rev()
        .map(|i| crate::api::types::ChatMessage {
            id: format!("m{i}"),
            text: format!("hi {i}"),
            sender: "did:plc:a".into(),
            sent_at: "2026-09-22T00:00:00Z".into(),
            ..Default::default()
        })
        .collect();
    a.handle_event(Event::Messages {
        convo_id: "c".into(),
        cursor: None,
        result: Ok(crate::tui::worker::Page {
            items: msgs,
            cursor: Some("older".into()),
        }),
    });
    let mut asked = false;
    for _ in 0..300 {
        render(&mut a, 100, 60);
        let jobs = a.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('k'),
        ));
        if jobs.iter().any(|j| {
            matches!(
                j,
                crate::tui::worker::Job::Messages {
                    cursor: Some(_),
                    ..
                }
            )
        }) {
            asked = true;
            break;
        }
    }
    let screen = render(&mut a, 100, 60);
    assert!(
        asked,
        "k held at the top never asked for older messages; scroll={}\n{screen}",
        a.chat.open.as_ref().unwrap().scroll
    );
}
