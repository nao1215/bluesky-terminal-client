use super::*;

/// A terminal that draws no pictures says what a quote carries, the
/// same way it does for a post's own pictures.
#[test]
fn a_quote_of_a_picture_post_says_what_it_carries_as_text() {
    let post: Post = serde_json::from_value(json!({
        "uri": "at://p/q", "cid": "c",
        "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
        "record": {"text": "look at this", "createdAt": "2026-09-22T00:00:00Z"},
        "embed": {"$type": "app.bsky.embed.record#view", "record": {
            "$type": "app.bsky.embed.record#viewRecord",
            "uri": "at://did:plc:bob/app.bsky.feed.post/q", "cid": "cq",
            "author": {"did": "did:plc:bob", "handle": "bob.test"},
            "value": {"text": "from the top"},
            "embeds": [{"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "山の頂上👨\u{200d}👩\u{200d}👧"}
            ]}]
        }}
    }))
    .unwrap();
    let (mut app, _) = App::new(Some(session()), "x");
    app.without_pictures();
    app.handle_event(Event::Timeline(Ok(vec![post].into())));
    let screen = render_text_only(&mut app, 80, 24);
    assert!(screen.contains("❝ @bob.test: from the top"), "{screen}");
    assert!(
        screen.contains("▣ 1 picture: 山の頂上👨\u{200d}👩\u{200d}👧"),
        "{screen}"
    );
}

/// A quote post says whose post it quotes. Beside a picture the quote
/// used to be dropped, and a quote of a post that is gone or blocked
/// showed nothing at all, which reads as an empty post.
#[test]
fn a_quote_names_who_it_quotes_beside_a_picture_and_says_when_it_cannot() {
    let quote_of = |embed: serde_json::Value| -> Post {
        serde_json::from_value(json!({
            "uri": "at://p/q", "cid": "c",
            "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
            "record": {"text": "look at this", "createdAt": "2026-09-22T00:00:00Z"},
            "embed": embed,
        }))
        .unwrap()
    };
    let viewed = json!({
        "$type": "app.bsky.embed.record#viewRecord",
        "uri": "at://did:plc:bob/app.bsky.feed.post/q", "cid": "c",
        "author": {"did": "did:plc:bob", "handle": "bob.test"},
        "value": {"text": "今日は👨\u{200d}👩\u{200d}👧 the quoted words"}
    });
    let with_picture = quote_of(json!({
        "$type": "app.bsky.embed.recordWithMedia#view",
        "record": {"$type": "app.bsky.embed.record#view", "record": viewed},
        "media": {"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}
        ]}
    }));
    let gone = quote_of(json!({
        "$type": "app.bsky.embed.record#view",
        "record": {"$type": "app.bsky.embed.record#viewNotFound",
                   "uri": "at://did:plc:bob/app.bsky.feed.post/x", "notFound": true}
    }));
    let blocked = quote_of(json!({
        "$type": "app.bsky.embed.record#view",
        "record": {"$type": "app.bsky.embed.record#viewBlocked",
                   "uri": "at://did:plc:bob/app.bsky.feed.post/y", "blocked": true,
                   "author": {"did": "did:plc:bob"}}
    }));
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(
        Ok(vec![with_picture, gone, blocked].into()),
    ));
    let screen = render_text_only(&mut app, 80, 40);
    assert!(
        screen.contains("❝ @bob.test: 今日は👨\u{200d}👩\u{200d}👧 the quoted words"),
        "{screen}"
    );
    assert!(screen.contains("quoted post not found"), "{screen}");
    assert!(
        screen.contains("quoted post from an account you cannot see"),
        "{screen}"
    );
}

/// A quote of something that is not a post is named on one line by
/// what it is: a feed, a list, or a starter pack.
#[rstest::rstest]
#[case(json!({"$type": "app.bsky.feed.defs#generatorView", "displayName": "猫🐈‍⬛ Cats"}), "❝ feed: 猫🐈‍⬛ Cats")]
#[case(json!({"$type": "app.bsky.graph.defs#listView", "name": "Rustaceans 🦀"}), "❝ list: Rustaceans 🦀")]
#[case(
    json!({"$type": "app.bsky.graph.defs#starterPackViewBasic",
           "uri": "at://did:plc:bob/app.bsky.graph.starterpack/sp", "cid": "c",
           "record": {"$type": "app.bsky.graph.starterpack", "name": "👨\u{200d}👩\u{200d}👧 家族 🇯🇵"},
           "creator": {"did": "did:plc:bob", "handle": "bob.test"}}),
    "❝ starter pack: 👨\u{200d}👩\u{200d}👧 家族 🇯🇵"
)]
fn a_quote_of_a_feed_a_list_or_a_starter_pack_says_what_it_is(
    #[case] record: serde_json::Value,
    #[case] want: &str,
) {
    let line = quote_line(&record, 80, &THEMES[0]);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(text, want);
}

/// The actions list offers D only where it works: on a post of your own.
#[test]
fn the_actions_list_offers_delete_only_on_your_own_post() {
    let (mut app, _) = App::new(Some(session()), "x");
    let mine: Post = serde_json::from_value(json!({
        "uri": "at://did:plc:me/app.bsky.feed.post/mine", "cid": "c",
        "author": {"did": "did:plc:me", "handle": "me.test", "displayName": "Me"},
        "record": {"text": "my own post", "createdAt": "2026-09-22T00:00:00Z"},
    }))
    .unwrap();
    app.handle_event(Event::Timeline(Ok(vec![posts(1).remove(0), mine].into())));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('.'),
    ));
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains("Actions"), "{screen}");
    assert!(!screen.contains("delete your post"), "{screen}");
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Esc,
    ));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('j'),
    ));
    app.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('.'),
    ));
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains("D      delete your post"), "{screen}");
    // The help says how it is confirmed.
    app.overlay = Some(Overlay::Help { scroll: 0 });
    let help = render(&mut app, 100, 50);
    assert!(help.contains("delete your own post"), "{help}");
}

#[test]
fn tiny_terminals_do_not_panic() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(3).into())));
    for (w, h) in [(1, 1), (5, 3), (10, 2), (20, 5)] {
        render(&mut app, w, h);
    }
    app.overlay = Some(Overlay::Help { scroll: 0 });
    render(&mut app, 8, 4);
    let (mut login, _) = App::new(None, "x");
    render(&mut login, 3, 3);
}

#[test]
fn image_rows_follow_the_aspect_ratio_within_bounds() {
    // 36 cells of 10px is 360px wide; 4:3 is 270px tall, 14 rows of 20px,
    // capped at the maximum.
    assert_eq!(image_rows(Some((4, 3)), 36, (10, 20)), IMAGE_ROWS_MAX);
    // 16:9 at 36 cells: 202.5px, 11 rows.
    assert_eq!(image_rows(Some((16, 9)), 36, (10, 20)), 11);
    // A panorama still gets a visible band.
    assert_eq!(image_rows(Some((10, 1)), 36, (10, 20)), IMAGE_ROWS_MIN);
    assert_eq!(image_rows(None, 36, (10, 20)), IMAGE_ROWS);
    // A degenerate cell size does not divide by zero.
    assert_eq!(image_rows(Some((1, 1)), 10, (0, 0)), 10);
}

#[test]
fn image_boxes_share_the_width() {
    assert_eq!(image_box_width(80, 1), IMAGE_MAX_W);
    assert_eq!(image_box_width(61, 4), 14);
    assert_eq!(image_box_width(3, 4), 0);
    assert_eq!(image_box_width(80, 0), 0);
}

#[test]
fn hints_stay_visible_while_a_status_message_shows() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.handle_event(Event::Timeline(Ok(posts(2).into())));
    app.handle_event(Event::Liked {
        post_uri: "at://p/0".into(),
        result: Ok("at://l".into()),
    });
    let screen = render(&mut app, 100, 24);
    assert!(screen.contains("liked"), "{screen}");
    assert!(screen.contains("? help"), "{screen}");
}

#[test]
fn help_is_clamped_to_its_last_page() {
    let (mut app, _) = App::new(Some(session()), "x");
    app.overlay = Some(Overlay::Help { scroll: u16::MAX });
    let screen = render(&mut app, 80, 16);
    assert!(screen.contains("close"), "{screen}");
    let Some(Overlay::Help { scroll }) = app.overlay else {
        panic!()
    };
    // No more rows than three per key (a description wraps onto two at
    // most at this width) and two per section.
    let rows: usize = keys::HELP.iter().map(|s| s.keys.len() * 3 + 2).sum();
    assert!(
        usize::from(scroll) < rows,
        "scroll was not clamped: {scroll}"
    );
    // The last section is on screen after scrolling to the end.
    assert!(screen.contains("scroll"), "{screen}");
}

// A line break in alt text keeps the words on either side apart.
#[test]
fn a_line_break_in_alt_text_keeps_its_words_apart() {
    let (mut a, _) = App::new(Some(session()), "x");
    let p: Post = serde_json::from_value(json!({
        "uri": "at://p/1", "cid": "c",
        "author": {"did": "did:plc:a", "handle": "alice.test"},
        "record": {"text": "pic"},
        "embed": {"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "first line of alt\nsecond line WORD"}]}
    })).unwrap();
    a.handle_event(Event::Timeline(Ok(vec![p].into())));
    a.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char(' '),
    ));
    assert!(a.viewer_open());
    let s = render_text_only(&mut a, 80, 20);

    assert!(
        !s.contains("altsecond"),
        "the line break of the alt text glues two words: {s}"
    );
}

// Half-width katakana with a sound mark (ｶﾞ, ﾊﾟ) takes two cells, as the
// terminal draws it: a post of it wraps inside the screen and loses no
// character. Measured as one cell, each line was laid out shorter than it
// is drawn, and its last characters fell off the right edge.
#[test]
fn half_width_katakana_with_sound_marks_wraps_without_losing_any() {
    let text = "ｶﾞｲｼﾞﾝ ﾊﾟﾝﾀﾞﾀﾞﾖ ﾃﾞﾌﾞｿﾞｳｶﾞﾊﾞﾝｻﾞｲ ｵﾜﾘ";
    let post: Post = serde_json::from_value(json!({
        "uri": "at://p/k", "cid": "c",
        "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "Alice"},
        "record": {"text": text, "createdAt": "2026-09-22T00:00:00Z"},
    }))
    .unwrap();
    for w in 24..=40u16 {
        let (mut app, _) = App::new(Some(session()), "x");
        app.without_pictures();
        app.handle_event(Event::Timeline(Ok(vec![post.clone()].into())));
        let screen = render_text_only(&mut app, w, 16);
        // The post's rows run on from one to the next past the selection
        // marker.
        let shown: String = screen
            .split_whitespace()
            .collect::<String>()
            .replace('▌', "");
        for word in text.split_whitespace() {
            assert!(shown.contains(word), "{w}: {word} is cut:\n{screen}");
        }
    }
}
