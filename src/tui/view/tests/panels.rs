use super::*;

/// Your own profile says that e and s exist, as buttons; someone
/// else's has neither.
#[test]
fn your_own_profile_shows_the_edit_and_settings_buttons() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('4'),
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
        crossterm::event::KeyCode::Char('4'),
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
    assert!(screen.contains("Pictures         kitty"), "{screen}");
    assert!(screen.contains("Browser          firefox"), "{screen}");
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
    app.handle_key(KeyEvent::from(KeyCode::Char('4')));
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
        screen.contains("enter open  space choose this folder"),
        "{screen}"
    );
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
        crossterm::event::KeyCode::Char('4'),
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
    for _ in 0..3 {
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
    app.accounts = vec![me.clone()];
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
    // None yet: how to add one.
    let mut empty = column_app(0);
    let screen = render(&mut empty, 80, 20);
    assert!(
        screen.contains("No columns yet. Press + to add one"),
        "{screen}"
    );
    assert!(screen.contains("+ add a column"), "{screen}");
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
    assert!(narrow.contains("Timeline,"), "{narrow}");
    assert!(narrow.contains("Search,"), "{narrow}");
    assert!(!narrow.contains("Time\n"), "{narrow}");
    // The two columns come back where they fit, and a description too
    // long for its column wraps rather than losing its end.
    let forty = render(&mut app, 40, 16);
    assert!(
        forty.contains("1 2 3 4 5 6    Timeline, Search,"),
        "{forty}"
    );
    assert!(
        forty
            .lines()
            .any(|l| l.contains("Notifications,") && !l.contains("1 2 3 4")),
        "{forty}"
    );
    assert!(
        forty
            .lines()
            .any(|l| l.trim_matches('│').trim() == "Profile, Columns,"),
        "{forty}"
    );
    let wide = render(&mut app, 80, 24);
    assert!(
        wide.contains("1 2 3 4 5 6    Timeline, Search, Notifications,"),
        "{wide}"
    );
}
