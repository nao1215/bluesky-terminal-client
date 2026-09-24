use super::*;

/// A reply loses two more rows to the post it answers, so not every
/// name fits. The ones that do not are counted rather than dropped.
#[test]
fn a_composer_too_short_for_every_name_counts_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(1).into())));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('r'),
    ));
    let Some(Overlay::Compose(c)) = &mut app.overlay else {
        panic!()
    };
    for i in 0..4 {
        c.media.push(crate::tui::app::Attached::new(
            dir.path().join(format!("pic{i}.png")),
        ));
    }
    let screen = render(&mut app, MIN_W, MIN_H);
    assert!(screen.contains(" 1 pic0.png"), "{screen}");
    assert!(screen.contains("and 3 more"), "{screen}");
}

#[test]
fn the_composer_lists_its_pictures_with_their_alt_text() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(Vec::new().into())));
    app.browse_from = Some(dir.path().to_path_buf());
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let Some(Overlay::Compose(c)) = &mut app.overlay else {
        panic!()
    };
    c.media
        .push(crate::tui::app::Attached::new(dir.path().join("cat.png")));
    let screen = render(&mut app, 100, 40);
    assert!(
        screen.contains("1 cat.png  alt: (none; tab to describe it)"),
        "{screen}"
    );
    assert!(screen.contains("tab alt text"), "{screen}");
}

/// Folder and file names come from the user's disk and are often
/// Japanese with emoji: the browser's path line, its list, the preview's
/// caption, and the composer's attachment list never cut one apart, at
/// any width.
#[test]
fn emoji_in_folder_and_file_names_are_never_cut_apart() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir
        .path()
        .join("家族👨\u{200d}👩\u{200d}👧\u{200d}👦の旅行🇯🇵");
    std::fs::create_dir(&folder).unwrap();
    std::fs::create_dir(folder.join("サブ👍🏽フォルダ")).unwrap();
    let name = "👍🏽いいね写真_with_a_rather_long_name_1️⃣.png";
    image::RgbImage::from_pixel(30, 20, image::Rgb([5, 5, 5]))
        .save(folder.join(name))
        .unwrap();
    for width in (20u16..=100).step_by(3) {
        let (mut app, _) = App::new(Some(session()), "https://bsky.social");
        app.handle_event(Event::Timeline(Ok(Vec::new().into())));
        app.browse_from = Some(folder.clone());
        for (c, m) in [
            ('n', crossterm::event::KeyModifiers::NONE),
            ('o', crossterm::event::KeyModifiers::CONTROL),
            ('j', crossterm::event::KeyModifiers::NONE),
            ('j', crossterm::event::KeyModifiers::NONE),
        ] {
            app.handle_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                m,
            ));
        }
        let rows = cells(&mut app, width, 30);
        for row in &rows {
            for c in row {
                assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
            }
        }
        if width >= 98 {
            let screen: String = rows.iter().map(|r| r.concat() + "\n").collect();
            assert!(
                screen.contains("家族👨\u{200d}👩\u{200d}👧\u{200d}👦の旅行🇯🇵"),
                "{screen}"
            );
            assert!(screen.contains("サブ👍🏽フォルダ/"), "{screen}");
            assert!(screen.contains("👍🏽いいね写真"), "{screen}");
        }
        // Chosen, the picture is listed in the composer by its name.
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Enter,
        ));
        for row in cells(&mut app, width, 30) {
            for c in &row {
                assert!(
                    !is_fragment(c),
                    "{width}: composer: a cut cluster {c:?} in {row:?}"
                );
            }
        }
    }
}

#[test]
fn the_browser_shows_the_folder_its_entries_and_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("trips")).unwrap();
    image::RgbImage::from_pixel(30, 20, image::Rgb([5, 5, 5]))
        .save(dir.path().join("snow.png"))
        .unwrap();
    let (mut app, _) = App::new(Some(session()), "https://bsky.social");
    app.handle_event(Event::Timeline(Ok(Vec::new().into())));
    app.browse_from = Some(dir.path().to_path_buf());
    for (c, m) in [
        ('n', crossterm::event::KeyModifiers::NONE),
        ('o', crossterm::event::KeyModifiers::CONTROL),
    ] {
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            m,
        ));
    }
    let screen = render(&mut app, 100, 30);
    assert!(
        screen.contains("Attach pictures (up to 4) or a video"),
        "{screen}"
    );
    assert!(screen.contains("trips/"), "{screen}");
    assert!(screen.contains("enter opens the folder"), "{screen}");
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('j'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let screen = render(&mut app, 100, 30);
    assert!(screen.contains("30×20 ·"), "{screen}");
    assert!(screen.contains("space mark"), "{screen}");
}

#[rstest::rstest]
#[case(0, "0 B")]
#[case(1023, "1023 B")]
#[case(2048, "2 KB")]
#[case(3 * 1024 * 1024 / 2, "1.5 MB")]
fn byte_counts_read_like_a_person_would_say_them(#[case] n: u64, #[case] want: &str) {
    assert_eq!(human_bytes(n), want);
}

#[test]
fn list_avatars_use_the_small_bluesky_version() {
    let full = "https://cdn.bsky.app/img/avatar/plain/did:plc:x/bafy@jpeg";
    assert_eq!(
        small_avatar(full),
        "https://cdn.bsky.app/img/avatar_thumbnail/plain/did:plc:x/bafy@jpeg"
    );
    assert_eq!(
        small_avatar("http://127.0.0.1/img/a.png"),
        "http://127.0.0.1/img/a.png"
    );
}

// On a short terminal the profile editor keeps the description on screen,
// with the line being typed, and the keys that save and cancel. The rows
// used to be squeezed at random: the description field got no row at all,
// so what was typed into it was nowhere to be seen.
#[test]
fn a_short_profile_editor_keeps_the_description_being_typed() {
    use crossterm::event::{KeyCode, KeyEvent};
    for h in [8, 10, 12] {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_key(KeyEvent::from(KeyCode::Char('5')));
        app.handle_event(Event::Profile(Ok((own_profile(), posts(1).into()))));
        app.handle_key(KeyEvent::from(KeyCode::Char('e')));
        app.handle_event(Event::ProfileEditor(Ok(
            crate::tui::worker::ProfileFields {
                display_name: "Nao 家族".into(),
                description: "one\ntwo\nthree\nfour\nthe last line 日本語".into(),
            },
        )));
        app.handle_key(KeyEvent::from(KeyCode::Tab));
        let screen = render_text_only(&mut app, 80, h);
        for want in [
            "Display name",
            "Nao 家族",
            "Description",
            "the last line 日本語",
            "esc cancel",
        ] {
            assert!(screen.contains(want), "80x{h} {want}:\n{screen}");
        }
    }
}
