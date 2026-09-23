use super::*;

#[test]
fn login_screen_explains_app_passwords() {
    let (mut app, _) = App::new(None, "https://bsky.social");
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("Log in to Bluesky"), "{screen}");
    assert!(screen.contains("an app password is safer"), "{screen}");
    assert!(screen.contains("unofficial client"), "{screen}");
    assert!(screen.contains("Password"));
    assert!(screen.contains("https://bsky.social"));
}

#[test]
fn a_long_login_error_is_shown_whole() {
    let (mut app, _) = App::new(None, "https://bsky.social");
    app.login.as_mut().unwrap().error = Some(
        "com.atproto.server.createSession failed: AuthenticationRequired: Invalid identifier or password".into(),
    );
    let screen = render(&mut app, 80, 24);
    assert!(
        screen.contains("Invalid identifier or password"),
        "{screen}"
    );
    assert!(screen.contains("esc quit"), "{screen}");
}

// Without pictures the text starts where the avatar was, and a line
// says what each post carries; descriptions keep their emoji whole.
#[test]
fn without_pictures_a_post_says_what_it_carries() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.without_pictures();
    let mut photos = posts(1).remove(0);
    photos.embed =
        serde_json::from_value(json!({"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "家族👨‍👩‍👧 🇯🇵 1️⃣ ❤️"},
            {"thumb": "https://t/2", "fullsize": "https://f/2", "alt": ""}
        ]}))
        .ok();
    let mut clip = posts(2).remove(1);
    clip.embed = serde_json::from_value(json!({"$type": "app.bsky.embed.video#view",
        "playlist": "https://v/p.m3u8", "alt": "a cat"}))
    .ok();
    app.handle_event(Event::Timeline(Ok(vec![photos, clip].into())));
    let screen = render_text_only(&mut app, 80, 24);
    assert!(screen.contains("▣ 2 pictures: 家族👨‍👩‍👧 🇯🇵 1️⃣ ❤️"), "{screen}");
    assert!(screen.contains("▶ video: a cat"), "{screen}");
    // The name starts right after the selection marker and one space.
    assert!(
        !screen.contains("▶ video\n"),
        "the video is named once: {screen}"
    );
    assert!(
        screen.lines().any(|l| l.starts_with("▌  Alice")),
        "{screen}"
    );
    assert!(screen.contains("space open in browser"), "{screen}");
    // A narrow screen cuts the line as one piece.
    let narrow = render_text_only(&mut app, 24, 24);
    assert!(narrow.contains("▣ 2 pictures: 家族👨‍👩‍👧…"), "{narrow}");
}

/// Without pictures nothing is drawn where an avatar would be: the
/// empty box used to be painted over the text that starts there, so the
/// first cells of a name lost their bold and the first letters of the
/// post took the placeholder's color.
#[test]
fn without_pictures_an_avatar_leaves_the_text_over_it_alone() {
    use ratatui::style::Modifier;
    let (mut app, _) = App::new(Some(session()), "x");
    app.without_pictures();
    let post: Post = serde_json::from_value(json!({
        "uri": "at://p/1", "cid": "c",
        "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "家族👨\u{200d}👩\u{200d}👧 Alice",
                   "avatar": "https://a/alice.jpg"},
        "record": {"text": "emphasizes design", "createdAt": "2026-09-22T00:00:00Z"},
    }))
    .unwrap();
    app.handle_event(Event::Timeline(Ok(vec![post].into())));
    let mut images = Images::none();
    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
    let buf = term.backend().buffer().clone();
    // The name starts at column 3, the text on the row under it.
    let (y, x) = (1..12u16)
        .find_map(|y| {
            (0..60u16)
                .find(|&x| buf[(x, y)].symbol() == "家")
                .map(|x| (y, x))
        })
        .expect("the name");
    // Every cell that starts a character of the name, the family emoji
    // included, is bold and in the same color as "Alice" further on.
    let alice = (x..60).find(|&c| buf[(c, y)].symbol() == "A").unwrap();
    let want = buf[(alice, y)].clone();
    for c in x..alice {
        let cell = &buf[(c, y)];
        if cell.symbol().is_empty() || cell.symbol() == " " {
            continue;
        }
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "{:?} at {c} is not bold",
            cell.symbol()
        );
        assert_eq!(cell.fg, want.fg, "{:?} at {c}", cell.symbol());
    }
    let body = buf[(x, y + 1)].clone();
    assert_eq!(body.symbol(), "e");
    assert_eq!(
        body.fg,
        buf[(x + 4, y + 1)].fg,
        "the first letters keep the text's color"
    );
    assert_eq!(body.modifier, buf[(x + 4, y + 1)].modifier);
}

#[test]
fn timeline_scrolls_to_keep_the_selection_visible() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(20).into())));
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("post number 0"));
    for _ in 0..15 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char('j'),
        ));
    }
    let screen = render(&mut app, 80, 24);
    assert!(screen.contains("post number 15"), "{screen}");
    assert!(!screen.contains("post number 0\n"));
    assert!(app.timeline.offset > 0);
}

#[test]
fn emoji_in_names_and_text_are_never_cut_apart() {
    let name = "👨‍👩‍👧‍👦 Family 🇯🇵";
    let text = "今日は👍🏽 1️⃣ ❤️ e\u{301}t\u{e9} 🇯🇵🇺🇸 😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀";
    let post: Post = serde_json::from_value(json!({
        "uri": "at://p/e", "cid": "c",
        "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": name},
        "record": {"text": text, "createdAt": "2026-09-22T00:00:00Z"},
        "likeCount": 3,
    }))
    .unwrap();
    for width in MIN_W..=80 {
        let (mut app, _) = App::new(Some(session()), "x");
        app.handle_event(Event::Timeline(Ok(vec![post.clone()].into())));
        let rows = cells(&mut app, width, 24);
        for row in &rows {
            let shown: usize = row.iter().map(|c| c.width().max(1)).sum();
            assert!(shown <= width as usize, "{width}: {row:?}");
            for c in row {
                assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
            }
        }
        let screen: String = rows.iter().map(|r| r.concat() + "\n").collect();
        for whole in ["👍🏽", "1️⃣", "🇯🇵", "🇺🇸", "e\u{301}"] {
            assert!(
                screen.contains(whole),
                "{width}: {whole} is not whole in\n{screen}"
            );
        }
        if width >= 41 {
            assert!(screen.contains("👨‍👩‍👧‍👦 Family 🇯🇵"), "{screen}");
        }
    }
}

#[test]
fn the_feed_bar_names_the_feeds_and_keeps_the_shown_one_in_sight() {
    let names = [
        "Discover",
        "Science 🔬",
        "日本語👨\u{200d}👩\u{200d}👧\u{200d}👦フィード",
        "Cats",
        "A feed with a very long name that goes on",
    ];
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    app.handle_event(Event::PinnedFeeds(Ok(names
        .iter()
        .map(|n| crate::api::types::FeedInfo {
            uri: format!("at://f/{n}"),
            name: n.to_string(),
        })
        .collect())));
    let rows = cells(&mut app, 100, 12);
    let bar = rows[1].concat();
    assert!(
        bar.starts_with("  Following   Discover   Science 🔬 "),
        "{bar:?}"
    );
    assert!(bar.trim_end().ends_with("…  [ ] feeds"), "{bar:?}");
    let screen = render(&mut app, 100, 12);
    assert!(screen.contains("post number 0"), "{screen}");
    // The last feed shown on a narrow screen: still in sight.
    for _ in 0..5 {
        app.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char(']'),
        ));
    }
    for width in [30u16, 40, 60, 100] {
        let rows = cells(&mut app, width, 12);
        let bar: String = rows[1].concat();
        assert!(bar.contains("A feed with"), "{width}: {bar:?}");
        assert!(bar.contains("[ ] feeds"), "{width}: {bar:?}");
        for row in &rows {
            for c in row {
                assert!(!is_fragment(c), "{width}: a cut cluster {c:?} in {row:?}");
            }
        }
    }
    // Until it has loaded, the feed says so; empty, it says that.
    assert!(render(&mut app, 100, 12).contains("loading…"));
    app.handle_event(Event::CustomFeed {
        uri: format!("at://f/{}", names[4]),
        result: Ok(Vec::new().into()),
    });
    let screen = render(&mut app, 100, 12);
    assert!(screen.contains("No posts in this feed yet"), "{screen}");
}

#[test]
fn empty_timeline_says_why() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(vec![].into())));
    assert!(render(&mut app, 80, 10).contains("No posts from accounts you follow"));
}

#[test]
fn a_terminal_below_the_minimum_says_so_and_gives_both_sizes() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    let small = render(&mut app, 23, 7);
    assert!(small.contains("Terminal too small"), "{small}");
    assert!(small.contains("24x8 needed, now 23x7"), "{small}");
    // Only the notice: no shreds of the tabs, the posts, or the hints.
    assert!(!small.contains("Timeline"), "{small}");
    assert!(!small.contains("post number"), "{small}");
    assert!(!small.contains("? help"), "{small}");
    // One cell short in either direction is still too small.
    assert!(render(&mut app, 24, 7).contains("Terminal too small"));
    assert!(render(&mut app, 23, 8).contains("Terminal too small"));
    // At the minimum the client is drawn.
    let ok = render(&mut app, 24, 8);
    assert!(!ok.contains("Terminal too small"), "{ok}");
    assert!(ok.contains("post number 0"), "{ok}");
}

#[test]
fn the_notice_replaces_the_login_form_too_and_fits_the_narrowest_screen() {
    let (mut login, _) = App::new(None, "x");
    let small = render(&mut login, 20, 7);
    assert!(small.contains("Terminal too small"), "{small}");
    assert!(!small.contains("Handle"), "{small}");
    // Narrower than the words: they wrap instead of being cut off.
    let thin = render(&mut login, 9, 6);
    assert!(thin.contains("Terminal"), "{thin}");
    assert!(thin.contains("small"), "{thin}");
    assert!(thin.contains("now 9x6"), "{thin}");
}

/// Every view, on every screen from the smallest the client draws in up
/// to a comfortable one, stays inside the screen and never cuts a
/// cluster apart. The narrow screens are where the columns a layout
/// reserves (an avatar, a key column, a row of thumbnails) stop fitting.
#[test]
fn every_view_on_a_cramped_screen_stays_inside_it() {
    use crossterm::event::{KeyCode, KeyEvent};
    fn ch(c: char) -> KeyEvent {
        KeyEvent::from(KeyCode::Char(c))
    }
    fn emoji_posts() -> Vec<Post> {
        (0..4)
            .map(|i| {
                serde_json::from_value(json!({
                    "uri": format!("at://p/{i}"), "cid": "c",
                    "author": {"did": "did:plc:a", "handle": "alice.test",
                               "displayName": "👨‍👩‍👧‍👦 家族 🇯🇵 Alice"},
                    "record": {"text": "今日は👍🏽 1️⃣ ❤️ e\u{301}te\u{301} 🇯🇵🇺🇸 https://example.com/a?b=1 @bob.test #タグ",
                               "createdAt": "2026-09-22T00:00:00Z"},
                    "likeCount": 12,
                    "embed": {"$type": "app.bsky.embed.images#view", "images": [
                        {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "山の頂上👨‍👩‍👧", "aspectRatio": {"width": 4, "height": 3}},
                        {"thumb": "https://t/2", "fullsize": "https://f/2", "alt": ""}
                    ]}
                }))
                .unwrap()
            })
            .collect()
    }
    /// A named view, built from scratch for each size.
    type State = (&'static str, Box<dyn Fn() -> App>);
    let states: Vec<State> = vec![
        (
            "timeline",
            Box::new(|| {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a
            }),
        ),
        (
            "compose",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('n'));
                for c in "今日は👍🏽 1️⃣ ❤️ 🇯🇵 a rather long draft that wraps".chars()
                {
                    a.handle_key(ch(c));
                }
                a
            }),
        ),
        (
            "compose+pictures",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('n'));
                let Some(Overlay::Compose(c)) = &mut a.overlay else {
                    panic!()
                };
                for i in 0..4 {
                    c.media
                        .push(crate::tui::app::Attached::new(std::path::PathBuf::from(
                            format!("/tmp/写真👨\u{200d}👩\u{200d}👧{i}.png"),
                        )));
                }
                a
            }),
        ),
        (
            "reply",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('r'));
                a
            }),
        ),
        (
            "help",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('?'));
                a
            }),
        ),
        (
            "themes",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('T'));
                a
            }),
        ),
        (
            "viewer",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(KeyEvent::from(KeyCode::Char(' ')));
                a
            }),
        ),
        (
            "thread",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.handle_key(ch('v'));
                a
            }),
        ),
        (
            "search",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('2'));
                a.handle_key(ch('/'));
                for c in "検索👍🏽 word".chars() {
                    a.handle_key(ch(c));
                }
                a
            }),
        ),
        (
            "profile",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('4'));
                a
            }),
        ),
        (
            "own profile",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('4'));
                a.handle_event(Event::Profile(Ok((own_profile(), emoji_posts().into()))));
                a
            }),
        ),
        (
            "settings",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.env.download_dir =
                    Some("/home/me/写真/👨\u{200d}👩\u{200d}👧 家族 🇯🇵/1️⃣ ❤️ e\u{301}".into());
                a.handle_key(ch('4'));
                a.handle_event(Event::Profile(Ok((own_profile(), emoji_posts().into()))));
                a.handle_key(ch('s'));
                a.handle_key(ch('j'));
                a.handle_key(ch('j'));
                a
            }),
        ),
        (
            "settings typing",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('4'));
                a.handle_key(ch('s'));
                for _ in 0..5 {
                    a.handle_key(ch('j'));
                }
                a.handle_key(KeyEvent::from(KeyCode::Enter));
                for c in "ブラウザ👨\u{200d}👩\u{200d}👧 🇯🇵 1️⃣ ❤️ e\u{301} مرحبا".chars()
                {
                    a.handle_key(ch(c));
                }
                a
            }),
        ),
        (
            "settings folder",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.browse_from = Some(std::env::temp_dir());
                a.env.download_dir = None;
                a.handle_key(ch('4'));
                a.handle_key(ch('s'));
                a.handle_key(ch('j'));
                a.handle_key(ch('j'));
                a.handle_key(KeyEvent::from(KeyCode::Enter));
                a
            }),
        ),
        (
            "columns",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_event(Event::Timeline(Ok(emoji_posts().into())));
                a.columns = crate::tui::columns::Columns::from_sources(&[
                    crate::config::ColumnSource::Following,
                    crate::config::ColumnSource::Notifications,
                    crate::config::ColumnSource::Search {
                        query: "家族👨\u{200d}👩\u{200d}👧 🇯🇵 1️⃣".into(),
                    },
                ]);
                a.handle_key(ch('5'));
                for (i, c) in a.columns.items.clone().iter().enumerate() {
                    let (id, generation) = (c.id, c.generation);
                    let result = if i == 1 {
                        Ok(MorePage::Notifications(Vec::new().into()))
                    } else {
                        Ok(MorePage::Posts(emoji_posts().into()))
                    };
                    a.handle_event(Event::Column {
                        id,
                        generation,
                        cursor: None,
                        result,
                    });
                }
                a
            }),
        ),
        (
            "chat",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('6'));
                let convo: crate::api::types::Convo = serde_json::from_value(json!({
                    "id": "c", "rev": "r",
                    "members": [{"did": "did:plc:me", "handle": "me.test"},
                                {"did": "did:plc:a", "handle": "alice.test", "displayName": "👨‍👩‍👧‍👦 家族 🇯🇵 Alice"}],
                    "lastMessage": {"$type": "chat.bsky.convo.defs#messageView", "id": "m", "rev": "r",
                                    "text": "今日は👍🏽 1️⃣ ❤️ e\u{301}te\u{301} مرحبا", "sender": {"did": "did:plc:a"}, "sentAt": "2026-09-22T00:00:00Z"},
                    "muted": false, "unreadCount": 3
                }))
                .unwrap();
                a.handle_event(Event::Convos {
                    cursor: None,
                    result: Ok(vec![convo].into()),
                });
                a
            }),
        ),
        (
            "chat open",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('6'));
                let convo: crate::api::types::Convo = serde_json::from_value(json!({
                    "id": "c", "rev": "r",
                    "members": [{"did": "did:plc:a", "handle": "alice.test", "displayName": "👨‍👩‍👧‍👦 家族 🇯🇵"}],
                    "muted": false, "unreadCount": 0
                }))
                .unwrap();
                a.handle_event(Event::Convos {
                    cursor: None,
                    result: Ok(vec![convo].into()),
                });
                a.handle_key(KeyEvent::from(KeyCode::Enter));
                a.handle_event(Event::Messages {
                    convo_id: "c".into(),
                    cursor: None,
                    result: Ok(vec![crate::api::types::ChatMessage {
                        id: "m".into(),
                        text: "今日は👍🏽 1️⃣ ❤️ e\u{301} a long message that has to wrap over the rows of a narrow screen مرحبا".into(),
                        sender: "did:plc:a".into(),
                        sent_at: "2026-09-22T00:00:00Z".into(),
                        ..Default::default()
                    }]
                    .into()),
                });
                a.handle_key(ch('i'));
                for c in "返事👨‍👩‍👧 🇯🇵".chars() {
                    a.handle_key(ch(c));
                }
                a
            }),
        ),
        (
            "add column",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.handle_key(ch('5'));
                a.handle_key(ch('+'));
                a
            }),
        ),
        (
            "accounts",
            Box::new(move || {
                let (mut a, _) = App::new(Some(session()), "x");
                a.accounts = vec![
                    crate::tui::app::Account {
                        did: "did:plc:me".into(),
                        handle: "me.test".into(),
                    },
                    crate::tui::app::Account {
                        did: "did:plc:x".into(),
                        handle: "家族👨\u{200d}👩\u{200d}👧-🇯🇵-1️⃣-a-very-long-handle.example"
                            .into(),
                    },
                ];
                a.handle_key(ch('A'));
                a
            }),
        ),
        (
            "login",
            Box::new(|| {
                let (a, _) = App::new(None, "https://bsky.social");
                a
            }),
        ),
    ];
    for (name, make) in &states {
        for w in [MIN_W, MIN_W + 1, 30, 33, 40, 47, 56] {
            for h in [MIN_H, MIN_H + 1, 12, 16] {
                let mut app = make();
                let rows = cells(&mut app, w, h);
                for (y, row) in rows.iter().enumerate() {
                    let shown: usize = row.iter().map(|c| c.width().max(1)).sum();
                    assert!(shown <= w as usize, "{name} {w}x{h} row {y}: {row:?}");
                    for c in row {
                        assert!(
                            !is_fragment(c),
                            "{name} {w}x{h} row {y}: cut {c:?} in {row:?}"
                        );
                    }
                }
            }
        }
    }
}
