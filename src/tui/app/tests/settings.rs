use super::*;

#[test]
fn enter_in_the_picker_saves_the_theme_and_keeps_other_settings() {
    let mut app = logged_in();
    let mut settings = Settings::default();
    settings.other.insert("future".into(), json!(1));
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    app.handle_key(key('T'));
    app.handle_key(key('k')); // wraps to the last theme
    app.handle_key(code(KeyCode::Enter));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.theme.as_deref(), Some(THEMES[THEMES.len() - 1].name));
    assert_eq!(saved.other.get("future"), Some(&json!(1)));
    app.settings_saved(Ok(()));
    assert!(app.status.as_ref().unwrap().text.contains("monochrome"));
}

#[test]
fn a_saved_theme_is_used_and_an_unknown_one_warns() {
    let mut app = logged_in();
    let settings = Settings {
        theme: Some("Nord".into()),
        ..Settings::default()
    };
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    assert_eq!(app.theme.name, "nord");
    let settings = Settings {
        theme: Some("neon".into()),
        ..Settings::default()
    };
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    assert_eq!(app.theme.name, "bluesky");
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("unknown theme \"neon\"")
    );
}

#[test]
fn no_color_keeps_monochrome_and_refuses_the_picker() {
    let mut app = logged_in();
    let settings = Settings {
        theme: Some("dracula".into()),
        ..Settings::default()
    };
    app.apply_settings(settings, ColorDepth::None, None);
    assert!(app.theme.mono);
    app.handle_key(key('T'));
    assert!(app.overlay.is_none());
    assert!(app.status.as_ref().unwrap().text.contains("NO_COLOR"));
}

#[test]
fn s_opens_the_settings_only_on_your_own_profile() {
    let mut app = logged_in();
    // Not on another tab: s is nothing there.
    app.handle_key(key('s'));
    assert!(app.overlay.is_none());
    app.handle_key(key('5'));
    app.handle_key(key('s'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    // Someone else's profile has no settings.
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_key(key('s'));
    assert!(app.overlay.is_none());
}

#[test]
fn the_settings_screen_names_every_setting() {
    let mut app = logged_in();
    let names: Vec<_> = app.settings_rows().iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        [
            "Theme",
            "Pictures",
            "Download folder",
            "Picture cache",
            "Video service",
            "Browser"
        ]
    );
    app.handle_key(key('5'));
    app.handle_key(key('s'));
    // Moving wraps both ways.
    app.handle_key(key('k'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 5, .. })
    ));
    app.handle_key(key('j'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
}

#[test]
fn pictures_off_is_kept_and_asked_of_the_event_loop() {
    let mut app = logged_in();
    let mut settings = Settings::default();
    settings.other.insert("future".into(), json!(1));
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    settings_on(&mut app, "Pictures");
    app.handle_key(code(KeyCode::Enter));
    assert!(!app.pictures, "the lists are text at once");
    assert_eq!(app.take_pictures_change(), Some(false));
    assert_eq!(app.take_pictures_change(), None);
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.pictures.as_deref(), Some("off"));
    assert_eq!(saved.other.get("future"), Some(&json!(1)));
    app.settings_saved(Ok(()));
    assert_eq!(app.status.as_ref().unwrap().text, "pictures: off");
    // The screen stays open, and says what it is now.
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 1, .. })
    ));
    assert_eq!(app.settings_rows()[1].value, "off");

    // Back to auto: the event loop finds out whether the terminal
    // draws them, and says.
    app.handle_key(key(' '));
    assert_eq!(app.take_pictures_change(), Some(true));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.pictures.as_deref(), Some("auto"));
    app.pictures_back(true);
    assert!(app.pictures);
    app.pictures_back(false);
    assert!(!app.pictures);
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("cannot show pictures")
    );
}

#[test]
fn a_setting_the_environment_fixes_is_shown_and_not_changed() {
    let mut app = logged_in();
    app.env = crate::config::Environment {
        graphics: Some("kitty".into()),
        download_dir: Some("/tmp/写真👨\u{200d}👩\u{200d}👧".into()),
        ..Default::default()
    };
    let rows = app.settings_rows();
    assert_eq!(rows[1].note, "set by BSKY_GRAPHICS for this run");
    assert!(!rows[1].editable);
    assert_eq!(rows[2].value, "/tmp/写真👨\u{200d}👩\u{200d}👧");
    assert_eq!(rows[2].note, "set by BSKY_DOWNLOAD_DIR for this run");
    settings_on(&mut app, "Pictures");
    app.handle_key(code(KeyCode::Enter));
    assert!(app.pictures);
    assert_eq!(app.take_pictures_change(), None);
    assert!(app.take_settings_save().is_none());
    assert!(app.status.as_ref().unwrap().text.contains("BSKY_GRAPHICS"));
}

#[test]
fn the_theme_row_opens_the_picker_and_comes_back_to_the_settings() {
    let mut app = logged_in();
    settings_on(&mut app, "Theme");
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.theme.as_deref(), Some(THEMES[1].name));
    assert_eq!(app.settings_rows()[0].value, THEMES[1].name);
    // Esc in the picker comes back too, with the theme as it was.
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Esc));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
    assert_eq!(app.theme_index, 1);
    // T on its own still closes to the list.
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('T'));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
}

#[test]
fn a_folder_chosen_on_the_screen_is_kept_and_d_saves_there() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("写真👨\u{200d}👩\u{200d}👧 🇯🇵");
    std::fs::create_dir(&chosen).unwrap();
    let mut app = logged_in();
    let mut settings = Settings::default();
    settings.other.insert("future".into(), json!(1));
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    app.browse_from = Some(dir.path().to_path_buf());
    settings_on(&mut app, "Download folder");
    app.handle_key(code(KeyCode::Enter));
    let Some(Overlay::Settings {
        edit: Some(SettingEdit::Folder(b)),
        ..
    }) = &mut app.overlay
    else {
        panic!("{:?}", app.overlay)
    };
    // Into the folder, then choose it.
    b.dir = dir.path().to_path_buf();
    app.handle_key(code(KeyCode::Char('~')));
    let Some(Overlay::Settings {
        edit: Some(SettingEdit::Folder(b)),
        ..
    }) = &mut app.overlay
    else {
        panic!()
    };
    **b = Browser::folder(&chosen);
    app.handle_key(key(' '));
    assert!(
        matches!(
            app.overlay,
            Some(Overlay::Settings {
                selected: 2,
                edit: None
            })
        ),
        "{:?}",
        app.overlay
    );
    let want = std::path::absolute(&chosen).unwrap();
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.download_dir, Some(want.display().to_string()));
    assert_eq!(saved.other.get("future"), Some(&json!(1)));
    app.settings_saved(Ok(()));
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .starts_with("download folder: "),
        "{:?}",
        app.status
    );
    let row = &app.settings_rows()[2];
    assert_eq!(row.value, want.display().to_string());
    assert!(row.resettable);
    assert!(
        row.note.contains("x goes back to the default"),
        "{}",
        row.note
    );
    // The next d saves into it.
    app.handle_key(code(KeyCode::Esc));
    viewing_a_picture(&mut app);
    let jobs = app.handle_key(key('d'));
    assert!(
        matches!(&jobs[..], [Job::Download { dir: Some(d), .. }] if *d == want),
        "{jobs:?}"
    );
    // x puts it back.
    app.handle_key(code(KeyCode::Esc));
    settings_on(&mut app, "Download folder");
    app.handle_key(key('x'));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.download_dir, None);
    assert!(!app.settings_rows()[2].resettable);
}

#[test]
fn a_folder_that_cannot_be_written_is_refused_and_the_browser_stays() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a-file");
    std::fs::write(&file, "x").unwrap();
    let mut app = logged_in();
    settings_on(&mut app, "Picture cache");
    app.handle_key(code(KeyCode::Enter));
    let Some(Overlay::Settings {
        edit: Some(SettingEdit::Folder(b)),
        ..
    }) = &mut app.overlay
    else {
        panic!()
    };
    // A folder under a file cannot be made.
    b.dir = file.join("sub");
    app.handle_key(key(' '));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings {
            edit: Some(SettingEdit::Folder(_)),
            ..
        })
    ));
    assert!(app.take_settings_save().is_none());
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.error && s.text.starts_with("cannot write to ")),
        "{:?}",
        app.status
    );
    // Esc leaves the browser for the list, with nothing changed.
    app.handle_key(code(KeyCode::Esc));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings {
            selected: 3,
            edit: None
        })
    ));
}

#[test]
fn a_new_picture_cache_rebuilds_the_pictures() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = logged_in();
    settings_on(&mut app, "Picture cache");
    app.handle_key(code(KeyCode::Enter));
    let Some(Overlay::Settings {
        edit: Some(SettingEdit::Folder(b)),
        ..
    }) = &mut app.overlay
    else {
        panic!()
    };
    b.dir = dir.path().to_path_buf();
    app.handle_key(key(' '));
    assert_eq!(app.cache_dir(), Some(dir.path().to_path_buf()));
    assert_eq!(app.take_pictures_change(), Some(true));
    // Without pictures there is nothing to rebuild.
    app.pictures = false;
    app.handle_key(key('x'));
    assert_eq!(app.take_pictures_change(), None);
}

#[test]
fn the_video_service_and_the_browser_are_typed_and_go_with_their_jobs() {
    let mut app = logged_in();
    settings_on(&mut app, "Video service");
    app.handle_key(code(KeyCode::Enter));
    // The field starts with what is set now.
    let Some(Overlay::Settings {
        edit: Some(SettingEdit::Text(input)),
        ..
    }) = &app.overlay
    else {
        panic!()
    };
    assert_eq!(input.text(), crate::api::DEFAULT_VIDEO_SERVICE);
    app.handle_key(ctrl('u'));
    type_str(&mut app, "ftp://nope");
    app.handle_key(code(KeyCode::Enter));
    assert!(app.status.as_ref().is_some_and(|s| s.error), "refused");
    assert!(app.take_settings_save().is_none());
    app.handle_key(ctrl('u'));
    app.handle_paste("https://video.example/");
    app.handle_key(code(KeyCode::Enter));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(
        saved.video_service.as_deref(),
        Some("https://video.example/")
    );
    // Kept without the slash, as the variable is.
    assert_eq!(app.settings_rows()[4].value, "https://video.example");

    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    type_str(&mut app, "my browser 🦊");
    app.handle_key(code(KeyCode::Enter));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.browser.as_deref(), Some("my browser 🦊"));
    app.handle_key(code(KeyCode::Esc));

    // The jobs carry them.
    app.handle_key(key('1'));
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://did:plc:bob/app.bsky.feed.post/p2",
        "did:plc:bob",
        true,
    )]
    .into())));
    let jobs = app.handle_key(key('o'));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { browser: Some(b), .. }] if b == "my browser 🦊"),
        "{jobs:?}"
    );
    app.handle_key(key('n'));
    type_str(&mut app, "hello");
    let jobs = app.handle_key(ctrl('s'));
    assert!(
        matches!(&jobs[..], [Job::Post { video_service, .. }] if video_service == "https://video.example"),
        "{jobs:?}"
    );

    app.handle_event(Event::Posted {
        reply_to: None,
        result: Ok(()),
    });

    // Esc in the field keeps what was there; an empty field is the
    // default.
    settings_on(&mut app, "Browser");
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(ctrl('u'));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.take_settings_save().is_none());
    assert_eq!(app.settings_rows()[5].value, "my browser 🦊");
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(ctrl('u'));
    app.handle_key(code(KeyCode::Enter));
    let saved = app.take_settings_save().expect("settings to save");
    assert_eq!(saved.browser, None);
    assert_eq!(
        app.settings_rows()[5].value,
        crate::browser::system_opener()
    );
}

#[test]
fn a_setting_a_variable_fixes_cannot_be_changed_or_reset() {
    let mut app = logged_in();
    app.env.browser = Some("firefox".into());
    app.settings.browser = Some("chromium".into());
    let row = &app.settings_rows()[5];
    assert_eq!(row.value, "firefox");
    assert!(!row.editable && !row.resettable);
    settings_on(&mut app, "Browser");
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('x'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { edit: None, .. })
    ));
    assert!(app.take_settings_save().is_none());
    assert_eq!(app.browser().as_deref(), Some("firefox"));
}

#[test]
fn pictures_are_not_saved_over_an_unreadable_settings_file() {
    let mut app = logged_in();
    app.apply_settings(
        Settings::default(),
        ColorDepth::TrueColor,
        Some("settings.json is not valid and was ignored".into()),
    );
    settings_on(&mut app, "Pictures");
    app.handle_key(code(KeyCode::Enter));
    // Off for this run, and the file is left as it is.
    assert!(!app.pictures);
    assert_eq!(app.take_pictures_change(), Some(false));
    assert!(app.take_settings_save().is_none());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("for this session only")
    );
}

#[test]
fn a_failed_first_page_is_kept_as_the_reason() {
    let mut app = logged_in();
    app.handle_key(key('4'));
    app.handle_event(Event::Notifications {
        seen_at: "t".into(),
        result: Err(Error::api("listNotifications failed: boom")),
    });
    assert_eq!(
        app.notifications.error.as_deref(),
        Some("listNotifications failed: boom")
    );
    app.handle_key(key('R'));
    assert!(app.notifications.error.is_none());
}
