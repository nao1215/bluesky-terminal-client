use super::*;

#[test]
fn every_theme_draws_every_view_without_panicking() {
    use crate::tui::theme::{ColorDepth, THEMES};
    for depth in [ColorDepth::TrueColor, ColorDepth::Ansi256] {
        for (i, theme) in THEMES.iter().enumerate() {
            let (mut app, _) = App::new(Some(session()), "x");
            app.apply_settings(
                crate::config::Settings {
                    theme: Some(theme.name.into()),
                    ..Default::default()
                },
                depth,
                None,
            );
            assert_eq!(app.theme_index, i);
            app.handle_event(Event::Timeline(Ok(posts(3).into())));
            for overlay in [
                None,
                Some(Overlay::Help { scroll: 0 }),
                Some(Overlay::Themes {
                    selected: i,
                    previous: 0,
                }),
            ] {
                app.overlay = overlay;
                for (w, h) in [(100, 30), (MIN_W, MIN_H), (20, 6), (1, 1)] {
                    render(&mut app, w, h);
                }
            }
        }
    }
}

#[test]
fn a_theme_with_a_background_paints_the_whole_screen() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.apply_settings(
        crate::config::Settings {
            theme: Some("dracula".into()),
            ..Default::default()
        },
        crate::tui::theme::ColorDepth::TrueColor,
        None,
    );
    let buf = render_buffer(&mut app, 40, 10);
    // An empty cell far from any text still has the theme's background.
    assert_eq!(
        buf[(39, 5)].bg,
        ratatui::style::Color::Rgb(0x28, 0x2a, 0x36)
    );
}

#[test]
fn monochrome_draws_no_color_at_all() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.apply_settings(
        Default::default(),
        crate::tui::theme::ColorDepth::None,
        None,
    );
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    app.overlay = Some(Overlay::Help { scroll: 0 });
    let buf = render_buffer(&mut app, 100, 30);
    for cell in buf.content() {
        assert_eq!(cell.fg, ratatui::style::Color::Reset, "{cell:?}");
        assert_eq!(cell.bg, ratatui::style::Color::Reset, "{cell:?}");
    }
}

#[test]
fn the_picker_shows_a_window_of_themes_even_on_a_tall_screen() {
    let (mut app, _) = App::new(Some(session()), "x");
    let nord = crate::tui::theme::index_of("nord").unwrap();
    app.overlay = Some(Overlay::Themes {
        selected: nord,
        previous: 0,
    });
    let screen = render(&mut app, 100, 60);
    assert!(screen.contains("▶ nord"), "{screen}");
    let shown = THEMES
        .iter()
        .filter(|t| screen.contains(&format!(" {} ", t.name)))
        .count();
    assert!(shown <= THEME_ROWS + 1, "{shown} themes drawn:\n{screen}");
    assert!(screen.contains('▲') && screen.contains('▼'), "{screen}");
    // Every theme is reached by scrolling.
    for (i, theme) in THEMES.iter().enumerate() {
        app.overlay = Some(Overlay::Themes {
            selected: i,
            previous: 0,
        });
        let screen = render(&mut app, 100, 60);
        assert!(
            screen.contains(&format!("▶ {}", theme.name)),
            "{}",
            theme.name
        );
    }
}

#[test]
fn an_error_is_shown_in_the_middle_until_a_key() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(Vec::new().into())));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('f'),
        crossterm::event::KeyModifiers::NONE,
    ));
    app.status = Some(crate::tui::app::Status {
        text: "something went wrong".into(),
        error: true,
        at: std::time::Instant::now(),
    });
    let screen = render(&mut app, 80, 24);
    let rows: Vec<&str> = screen.lines().collect();
    let row = rows
        .iter()
        .position(|l| l.contains("something went wrong"))
        .expect("shown");
    assert!(
        (8..16).contains(&row),
        "in the middle, not the bottom:\n{screen}"
    );
    assert!(!rows[23].contains("something went wrong"));
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('j'),
        crossterm::event::KeyModifiers::NONE,
    ));
    assert!(!render(&mut app, 80, 24).contains("something went wrong"));
}

#[test]
fn the_picker_scrolls_to_keep_the_selection_on_a_short_screen() {
    let (mut app, _) = App::new(Some(session()), "x");
    let last = THEMES.len() - 1;
    app.overlay = Some(Overlay::Themes {
        selected: last,
        previous: 0,
    });
    let screen = render(&mut app, 80, 20);
    assert!(screen.contains("▶ monochrome"), "{screen}");
    assert!(
        screen.contains(&format!("{}/{}", last + 1, THEMES.len())),
        "{screen}"
    );
    assert!(
        !screen.contains("  bluesky "),
        "the top has scrolled away:\n{screen}"
    );
}

#[test]
fn a_reply_shows_its_thread_above_it() {
    let (mut app, _) = App::new(Some(session()), "x");
    let mut reply = posts(1).remove(0);
    let parent: crate::api::types::RefPost = serde_json::from_value(json!({
        "$type": "app.bsky.feed.defs#postView", "uri": "at://parent", "cid": "c",
        "author": {"did": "d", "handle": "carol.test", "displayName": "Carol"},
        "record": {"text": "the question\nsecond line"}
    }))
    .unwrap();
    reply.context = Some(Box::new(crate::api::types::ReplyContext {
        root: Some(crate::api::types::RefPost::NotFound {
            uri: "at://root".into(),
        }),
        gap: true,
        parent,
    }));
    app.handle_event(Event::Timeline(Ok(vec![reply].into())));
    let screen = render(&mut app, 80, 24);
    let rows: Vec<&str> = screen.lines().collect();
    let at = |s: &str| {
        rows.iter()
            .position(|r| r.contains(s))
            .unwrap_or_else(|| panic!("{s}:\n{screen}"))
    };
    assert!(at("(post not found)") < at("┆ ⋮"));
    assert!(at("┆ ⋮") < at("Carol @carol.test: the question"));
    assert!(at("Carol @carol.test: the question") < at("post number 0"));
    assert!(!screen.contains("second line"), "one line per post above");
}

#[test]
fn a_thread_indents_replies_and_shows_placeholders() {
    let (mut app, _) = App::new(Some(session()), "x");
    let node: crate::api::types::ThreadNode = serde_json::from_value(json!({
        "$type": "app.bsky.feed.defs#threadViewPost",
        "post": {"uri": "at://focus", "cid": "c", "author": {"did": "d", "handle": "a.test"}, "record": {"text": "the focus"}},
        "parent": {"$type": "app.bsky.feed.defs#notFoundPost", "uri": "at://gone", "notFound": true},
        "replies": [{"$type": "app.bsky.feed.defs#threadViewPost",
            "post": {"uri": "at://r1", "cid": "c", "author": {"did": "d", "handle": "b.test"}, "record": {"text": "a reply"}},
            "replies": []}]
    }))
    .unwrap();
    let (rows, focus) = crate::tui::thread::flatten(node);
    app.threads.push(crate::tui::app::ThreadView {
        uri: "at://focus".into(),
        list: List {
            items: rows,
            selected: focus,
            loaded: true,
            ..List::default()
        },
        error: None,
    });
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("Thread"), "{screen}");
    assert!(screen.contains("(post not found)"), "{screen}");
    let focus_line = screen.lines().find(|l| l.contains("the focus")).unwrap();
    let reply_line = screen.lines().find(|l| l.contains("a reply")).unwrap();
    let col = |l: &str, s: &str| l.find(s).unwrap();
    assert!(
        col(reply_line, "a reply") > col(focus_line, "the focus"),
        "the reply is indented:\n{screen}"
    );
    assert!(reply_line.contains('│'), "with a guide line:\n{screen}");
}

#[test]
fn notifications_say_who_did_what_and_mark_the_unread() {
    let (mut app, _) = App::new(Some(session()), "x");
    let n = |reason: &str, read: bool| crate::tui::worker::NotifItem {
        n: serde_json::from_value(json!({
            "uri": format!("at://{reason}"), "reason": reason, "isRead": read,
            "author": {"did": "d", "handle": format!("{reason}.test"), "displayName": "Eve"},
            "indexedAt": "2026-09-22T00:00:00.000Z"
        }))
        .unwrap(),
        post: None,
        subject: (reason == "like").then(|| posts(1).remove(0)),
        fresh: !read,
    };
    app.tab = Tab::Notifications;
    app.unread = 1;
    app.notifications = List {
        items: vec![
            n("like", false),
            n("follow", true),
            n("starterpack-joined", true),
            n("brand-new", true),
        ],
        loaded: true,
        ..List::default()
    };
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains("4 Notifications (1)"), "{screen}");
    assert!(
        screen.contains("● Eve @like.test liked your post"),
        "{screen}"
    );
    assert!(
        screen.contains("post number 0"),
        "the liked post is quoted:\n{screen}"
    );
    assert!(screen.contains("Eve @follow.test followed you"), "{screen}");
    assert!(!screen.contains("● Eve @follow.test"), "{screen}");
    assert!(
        screen.contains("joined through your starter pack"),
        "{screen}"
    );
    assert!(
        screen.contains("(brand-new)"),
        "an unknown reason is still shown:\n{screen}"
    );
}

#[test]
fn scroll_moves_offset_only_as_far_as_needed() {
    let h = |_| 3;
    assert_eq!(scroll_offset(3, 0, 100, 7, h), 2);
    assert_eq!(scroll_offset(1, 2, 100, 7, h), 1, "up past the top");
    assert_eq!(scroll_offset(2, 1, 100, 7, h), 1, "already on screen");
    assert_eq!(scroll_offset(4, 4, 100, 7, h), 4);
    // A post taller than the screen is drawn from its top.
    assert_eq!(
        scroll_offset(2, 0, 100, 7, |i| if i == 2 { 20 } else { 3 }),
        2
    );
}

/// Room left below the last post is used for the posts above: after the
/// terminal grows, or near the end of a list, the screen is filled from
/// the bottom instead of leaving the first posts hidden.
#[test]
fn a_list_that_ends_on_screen_fills_it_from_above() {
    let h = |_| 3;
    // Five posts of 3 rows; the last three were on a 9-row screen.
    assert_eq!(scroll_offset(3, 2, 5, 9, h), 2, "the end just fills it");
    assert_eq!(scroll_offset(3, 2, 5, 15, h), 0, "grown: all five fit");
    assert_eq!(scroll_offset(4, 3, 5, 12, h), 1, "grown: four fit");
    // Not past what keeps the selection on screen, and not for a list
    // that goes on below the screen.
    assert_eq!(scroll_offset(1, 1, 50, 9, h), 1);
    assert_eq!(scroll_offset(0, 0, 5, 30, h), 0);
}

#[test]
fn jumping_to_the_end_measures_only_what_fits() {
    let mut measured = Vec::new();
    let offset = scroll_offset(9_999, 0, 10_000, 10, |i| {
        measured.push(i);
        3
    });
    assert_eq!(offset, 9_997);
    // Only the posts near the end are measured (the caller caches the
    // heights, so asking for one again costs nothing).
    measured.sort_unstable();
    measured.dedup();
    assert_eq!(measured, [9_996, 9_997, 9_998, 9_999]);
}

/// The composer on the smallest screen the client draws in: the box has
/// no room for the thumbnails, and the names of the pictures that will
/// be sent must take their rows rather than fall off the bottom.
#[test]
fn a_short_composer_still_lists_every_picture_it_would_send() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(1).into())));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('n'),
    ));
    let Some(Overlay::Compose(c)) = &mut app.overlay else {
        panic!()
    };
    for i in 0..4 {
        c.media.push(crate::tui::app::Attached::new(
            dir.path()
                .join(format!("写真👨\u{200d}👩\u{200d}👧{i}.png")),
        ));
    }
    // The cells a wide character covers are skipped, so the names read
    // as the terminal shows them.
    let screen = render_text_only(&mut app, MIN_W, MIN_H);
    for i in 1..=4 {
        assert!(
            screen.contains(&format!(" {i} 写真👨\u{200d}👩\u{200d}👧{}", i - 1)),
            "{i}: {screen}"
        );
    }
}

// A list over the screen taller than the screen scrolls with its
// selection: the entry Enter would act on is always shown. The lists used
// to draw from their first entry only, so on a short terminal moving down
// went on past the bottom of the box, and Enter did what was not shown.
#[test]
fn a_list_taller_than_its_box_shows_the_selected_entry() {
    use crate::tui::app::{Account, Overlay};
    let lists: Vec<(&str, Overlay)> = vec![
        (
            "actions",
            Overlay::Actions {
                selected: 11,
                about: None,
            },
        ),
        ("languages", Overlay::Languages { selected: 8 }),
        (
            "add column",
            Overlay::AddColumn {
                selected: 11,
                query: None,
            },
        ),
        ("accounts", Overlay::Accounts { selected: 9 }),
    ];
    for (name, overlay) in lists {
        for h in [8, 10, 12] {
            let (mut app, _) = App::new(Some(session()), "x");
            app.handle_event(Event::Timeline(Ok(posts(3).into())));
            app.feeds = (0..8)
                .map(|i| crate::api::types::FeedInfo {
                    uri: format!("at://did:plc:f/app.bsky.feed.generator/{i}"),
                    name: format!("feed {i}"),
                })
                .collect();
            app.accounts = (0..10)
                .map(|i| Account {
                    did: format!("did:plc:{i}"),
                    handle: format!("user{i}.test"),
                })
                .collect();
            app.overlay = Some(overlay.clone());
            let screen = render_text_only(&mut app, 80, h);
            assert!(screen.contains('▶'), "{name} 80x{h}:\n{screen}");
        }
    }
}
