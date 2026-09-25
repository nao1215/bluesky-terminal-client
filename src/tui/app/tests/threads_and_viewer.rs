use super::*;

// o always leads somewhere: the post's link where it has one, and the
// post itself on bsky.app where it has none, which is where its
// replies and its author's other posts are.
#[test]
fn o_opens_the_link_or_the_post_itself() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p1",
            json!({"$type": "app.bsky.embed.external#view",
                   "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
        ),
        post(
            "at://did:plc:bob/app.bsky.feed.post/p2",
            "did:plc:bob",
            true,
        ),
    ]
    .into())));
    let jobs = app.handle_key(key('o'));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
        "{jobs:?}"
    );
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('o'));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:bob/post/p2"),
        "{jobs:?}"
    );
    // Space still opens the viewer, and says so when there is nothing
    // to view: it does not send the reader to a browser.
    let jobs = app.handle_key(key(' '));
    assert!(jobs.is_empty(), "{jobs:?}");
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("no pictures, video, or link")),
        "{:?}",
        app.status
    );
}

// A terminal that cannot show pictures has no viewer: space opens the
// post on bsky.app, where they can be seen, and a link as before.
#[test]
fn without_pictures_space_opens_the_post_in_the_browser() {
    let mut app = logged_in();
    app.without_pictures();
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("cannot show pictures")),
        "{:?}",
        app.status
    );
    app.handle_event(Event::Timeline(Ok(vec![
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p1",
            json!({"$type": "app.bsky.embed.images#view", "images": [{"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}]}),
        ),
        with_pictures(
            "at://did:plc:alice/app.bsky.feed.post/p2",
            json!({"$type": "app.bsky.embed.external#view", "external": {"uri": "https://example.com/a", "title": "A", "description": ""}}),
        ),
    ]
    .into())));
    let jobs = app.handle_key(key(' '));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://bsky.app/profile/did:plc:alice/post/p1"),
        "{jobs:?}"
    );
    assert!(app.overlay.is_none());
    app.handle_key(key('j'));
    let jobs = app.handle_key(key(' '));
    assert!(
        matches!(&jobs[..], [Job::OpenLink { url: u, .. }] if u == "https://example.com/a"),
        "{jobs:?}"
    );
}

#[test]
fn without_pictures_the_keys_say_what_space_does() {
    let mut app = logged_in();
    assert!(crate::tui::keys::hints(&app).contains(&("space", "view")));
    app.without_pictures();
    let hints = crate::tui::keys::hints(&app);
    assert!(hints.contains(&("space", "open in browser")), "{hints:?}");
    assert!(!hints.contains(&("space", "view")));
    let help = crate::tui::keys::help(false);
    assert!(help.iter().all(|(title, _)| *title != "Viewer"));
    let posts = &help.iter().find(|(t, _)| *t == "Posts").unwrap().1;
    assert!(
        posts
            .iter()
            .any(|(k, d)| *k == "space" && d.contains("in the browser"))
    );
    assert!(
        crate::tui::keys::help(true)
            .iter()
            .any(|(t, _)| *t == "Viewer")
    );
}

#[test]
fn v_opens_the_thread_and_esc_closes_it() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    let th = app.threads.last().unwrap();
    assert_eq!(th.list.items.len(), 4);
    assert_eq!(
        th.list.selected, 1,
        "the opened post is selected, below its parent"
    );
    // Keys act on the thread: j moves to the first reply and l likes it.
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('l'));
    assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r1"));
    app.handle_event(Event::Liked {
        post_uri: "at://r1".into(),
        result: Ok("at://like".into()),
    });
    let liked = app.threads.last().unwrap().list.items[2]
        .post()
        .unwrap()
        .clone();
    assert_eq!(liked.like_count, 1);
    app.handle_key(code(KeyCode::Esc));
    assert!(app.threads.is_empty());
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(app.timeline.selected, 0, "the timeline is where it was");
}

#[test]
fn a_thread_inside_a_thread_stacks() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://r1"));
    assert_eq!(app.threads.len(), 2);
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.threads.len(), 1);
}

#[test]
fn a_late_thread_answer_is_dropped_and_a_tab_switch_closes_threads() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_key(code(KeyCode::Esc));
    // The answer for the thread already closed changes nothing.
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &[])),
    });
    assert!(app.threads.is_empty());
    app.handle_key(key('v'));
    app.handle_key(key('3'));
    assert!(app.threads.is_empty());
}

// was: R set the selection back to the focused post, so the next key
// acted on it instead of the reply that was picked.
#[test]
fn reloading_a_thread_keeps_the_selected_reply() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    assert_eq!(app.threads[0].list.selected, 3, "the second reply");
    app.handle_key(key('R'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    assert_eq!(app.threads[0].list.selected, 3);
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r2"),
        "{jobs:?}"
    );
}

#[test]
fn reloading_a_thread_whose_selected_reply_is_gone_goes_back_to_the_post() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    app.handle_key(key('R'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1"])),
    });
    assert_eq!(
        app.threads[0].list.selected, 1,
        "the opened post, below its parent"
    );
}

#[test]
fn a_failed_thread_says_why_and_r_retries() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Err(Error::api("NotFound: Post not found")),
    });
    assert_eq!(
        app.threads[0].error.as_deref(),
        Some("NotFound: Post not found")
    );
    let jobs = app.handle_key(key('R'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"));
}

// was: a reload that failed (R, or the reload after a reply was sent)
// put the error in place of the whole thread, while the keys still acted
// on the posts no longer shown: l liked a reply the reader could not see.
#[test]
fn a_failed_reload_keeps_the_thread_on_screen() {
    let mut app = logged_in();
    app.handle_key(key('v'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    app.handle_key(key('j'));
    app.handle_key(key('j'));
    app.handle_key(key('R'));
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Err(Error::api("connection reset")),
    });
    let th = &app.threads[0];
    assert_eq!(th.error, None, "the posts stay shown");
    assert!(th.list.loaded);
    assert_eq!(th.list.items.len(), 4);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|s| s.text.contains("connection reset")),
        "{:?}",
        app.status
    );
    let jobs = app.handle_key(key('l'));
    assert!(
        matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://r2"),
        "{jobs:?}"
    );
}

#[test]
fn an_unreadable_settings_file_is_never_overwritten() {
    let mut app = logged_in();
    app.apply_settings(
        Settings::default(),
        ColorDepth::TrueColor,
        Some("settings.json is not valid and was ignored".into()),
    );
    app.handle_key(key('T'));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.take_settings_save().is_none());
    assert_eq!(app.theme_index, 1, "used for this session");
    let status = app.status.as_ref().unwrap();
    assert!(
        status.error && status.text.contains("not overwritten"),
        "{status:?}"
    );
}

// A thread reloaded with R, and the same post opened again with v before
// the reload came back: both answers come, the newer first. Neither thread
// is left saying "loading" once Esc goes back to the first.
#[test]
fn a_thread_reloaded_and_opened_again_is_not_left_loading() {
    let mut app = logged_in();
    let open = press(&mut app, key('v'));
    app.handle_answer(
        open[0],
        Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        },
    );
    let reload = press(&mut app, key('R'));
    let again = press(&mut app, key('v'));
    assert_eq!((reload.len(), again.len()), (1, 1));
    for seq in [again[0], reload[0]] {
        app.handle_answer(
            seq,
            Event::Thread {
                uri: "at://a/p/1".into(),
                result: Ok(thread_json("at://a/p/1", &["at://r1"])),
            },
        );
    }
    assert!(app.threads.iter().all(|t| t.list.loaded));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.threads.last().unwrap().list.loaded);
}

// A reply sent in an open thread shows there once the thread is read
// again.
#[test]
fn a_reply_shows_in_the_thread_it_was_sent_in() {
    let mut app = logged_in();
    let open = press(&mut app, key('v'));
    app.handle_answer(
        open[0],
        Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &[])),
        },
    );
    app.handle_key(key('r'));
    type_str(&mut app, "reply");
    let send = press(&mut app, ctrl('s'));
    let jobs = app.handle_answer(
        send[0],
        Event::Posted {
            reply_to: Some("at://a/p/1".into()),
            result: Ok(()),
        },
    );
    let seqs: Vec<(u64, Job)> = jobs.into_iter().map(|j| (app.stamp(&j), j)).collect();
    let (seq, _) = seqs
        .iter()
        .find(|(_, j)| matches!(j, Job::Thread(_)))
        .unwrap();
    app.handle_answer(
        *seq,
        Event::Thread {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json(
                "at://a/p/1",
                &["at://did:plc:me/app.bsky.feed.post/myreply"],
            )),
        },
    );
    let uris: Vec<String> = app.threads[0]
        .list
        .items
        .iter()
        .filter_map(|r| r.post().map(|p| p.uri.clone()))
        .collect();
    assert!(
        uris.iter().any(|u| u.ends_with("myreply")),
        "the reply is not in the thread: {uris:?}"
    );
}

// H1b: a like still on its way, A -> B -> A, l sends a second Like.

/// Rest the selection on the post shown for [`read_ahead::REST`], as the
/// event loop's ticks would, and take the jobs, numbered.
fn rest(app: &mut App) -> Vec<(u64, Job)> {
    let t0 = Instant::now();
    assert!(app.poll_read_ahead(t0).is_empty(), "not before it rests");
    app.poll_read_ahead(t0 + read_ahead::REST)
        .into_iter()
        .map(|j| (app.stamp(&j), j))
        .collect()
}

// v waited a round trip for the thread every time. The thread of the post
// the selection rests on is read before v asks for it, and v shows it at
// once without asking again.
#[test]
fn the_thread_of_the_post_the_selection_rests_on_is_shown_at_once() {
    let mut app = logged_in();
    let pending = app.pending;
    let jobs = rest(&mut app);
    assert!(
        matches!(&jobs[..], [(_, Job::ReadAhead(u))] if u == "at://a/p/1"),
        "{jobs:?}"
    );
    assert_eq!(app.pending, pending, "the screen does not wait for it");
    // Once while it rests there.
    assert!(
        app.poll_read_ahead(Instant::now() + read_ahead::FRESH)
            .is_empty()
    );
    let seq = jobs[0].0;
    app.handle_answer(
        seq,
        Event::ReadAhead {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        },
    );
    assert_eq!(app.pending, pending);
    let jobs = app.handle_key(key('v'));
    assert!(jobs.is_empty(), "{jobs:?}");
    let th = app.threads.last().unwrap();
    assert!(th.list.loaded);
    assert_eq!(th.list.items.len(), 3);
    assert_eq!(th.list.selected, 1, "the opened post is selected");
}

// Moving on before the selection rests reads nothing: a held j reads no
// thread of the posts it passes.
#[test]
fn a_post_passed_by_is_not_read_ahead() {
    let mut app = logged_in();
    let t0 = Instant::now();
    assert!(app.poll_read_ahead(t0).is_empty());
    app.handle_key(key('j'));
    assert!(app.poll_read_ahead(t0 + read_ahead::REST / 2).is_empty());
    app.handle_key(key('k'));
    assert!(app.poll_read_ahead(t0 + read_ahead::REST).is_empty());
    assert!(
        app.poll_read_ahead(t0 + read_ahead::REST + read_ahead::REST / 2)
            .is_empty()
    );
    assert!(!app.poll_read_ahead(t0 + read_ahead::REST * 2).is_empty());
}

// A like sent after the thread was asked for is not in what it brought:
// shown as it came, the post would look unliked. v reads it again.
#[test]
fn a_thread_read_before_a_like_is_not_shown() {
    let mut app = logged_in();
    let jobs = rest(&mut app);
    press(&mut app, key('l'));
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        },
    );
    let jobs = app.handle_key(key('v'));
    assert!(
        matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"),
        "{jobs:?}"
    );
    assert!(app.threads.last().unwrap().list.items.is_empty());
}

// A thread read ahead a while ago is shown at once and read again, and the
// new answer replaces it.
#[test]
fn an_old_thread_read_ahead_is_shown_while_it_is_read_again() {
    let mut app = logged_in();
    let jobs = rest(&mut app);
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            uri: "at://a/p/1".into(),
            result: Ok(thread_json("at://a/p/1", &["at://r1"])),
        },
    );
    for t in &mut app.read_ahead.threads {
        t.at -= read_ahead::FRESH;
    }
    let jobs = app.handle_key(key('v'));
    assert!(
        matches!(&jobs[..], [Job::Thread(u)] if u == "at://a/p/1"),
        "{jobs:?}"
    );
    let th = app.threads.last().unwrap();
    assert!(!th.list.loaded);
    assert_eq!(th.list.items.len(), 3);
    app.handle_event(Event::Thread {
        uri: "at://a/p/1".into(),
        result: Ok(thread_json("at://a/p/1", &["at://r1", "at://r2"])),
    });
    let th = app.threads.last().unwrap();
    assert!(th.list.loaded);
    assert_eq!(th.list.items.len(), 4);
}

// A thread that failed to read ahead says nothing: nobody asked for it.
#[test]
fn a_thread_that_failed_to_read_ahead_says_nothing() {
    let mut app = logged_in();
    let jobs = rest(&mut app);
    let status = app.status.clone();
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            uri: "at://a/p/1".into(),
            result: Err(crate::error::Error::api("HTTP 502")),
        },
    );
    assert_eq!(app.status, status);
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(_)]), "{jobs:?}");
}

// The thread read ahead opened at once, and then waited for its pictures:
// they are downloaded ahead too, the small avatars as a list draws them.
#[test]
fn the_pictures_of_a_thread_read_ahead_are_downloaded_ahead() {
    let mut app = logged_in();
    let jobs = rest(&mut app);
    let node = serde_json::from_value(json!({
        "$type": "app.bsky.feed.defs#threadViewPost",
        "post": {"uri": "at://a/p/1", "cid": "c", "record": {"text": "山 🏔️"},
                 "author": {"did": "did:plc:a", "handle": "a.test",
                            "avatar": "https://cdn.bsky.app/img/avatar/plain/did:plc:a/bafa@jpeg"},
                 "embed": {"$type": "app.bsky.embed.images#view",
                           "images": [{"thumb": "https://cdn.test/t1", "fullsize": "https://cdn.test/f1", "alt": ""},
                                      {"thumb": "https://cdn.test/t2", "fullsize": "https://cdn.test/f2", "alt": ""}]}},
        "replies": [{
            "$type": "app.bsky.feed.defs#threadViewPost",
            "post": {"uri": "at://r/1", "cid": "c", "record": {"text": "👨‍👩‍👧"},
                     "author": {"did": "did:plc:r", "handle": "r.test",
                                "avatar": "https://cdn.bsky.app/img/avatar/plain/did:plc:r/bafr@jpeg"}},
            "replies": []
        }]
    }))
    .unwrap();
    assert!(app.take_pictures_ahead().is_empty());
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            uri: "at://a/p/1".into(),
            result: Ok(node),
        },
    );
    assert_eq!(
        app.take_pictures_ahead(),
        [
            "https://cdn.bsky.app/img/avatar_thumbnail/plain/did:plc:a/bafa@jpeg",
            "https://cdn.test/t1",
            "https://cdn.test/t2",
            "https://cdn.bsky.app/img/avatar_thumbnail/plain/did:plc:r/bafr@jpeg",
        ]
    );
    assert!(
        app.take_pictures_ahead().is_empty(),
        "each is asked for once"
    );
}

// Space on a video waited for its playlists; the selection resting on a
// post with a video has them read, once while it rests there.
#[test]
fn the_video_of_the_post_the_selection_rests_on_is_readied() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![with_pictures(
        "at://a/p/1",
        json!({"$type": "app.bsky.embed.video#view", "cid": "c",
               "playlist": "https://video.bsky.app/watch/did%3Aplc%3Aa/bafv/playlist.m3u8",
               "thumbnail": "https://video.cdn.test/t.jpg", "alt": "山の動画 🏔️"}),
    )]
    .into())));
    assert!(app.take_videos_ahead().is_empty());
    let t0 = Instant::now();
    app.poll_read_ahead(t0);
    assert!(app.take_videos_ahead().is_empty(), "not before it rests");
    app.poll_read_ahead(t0 + read_ahead::REST);
    assert_eq!(
        app.take_videos_ahead(),
        ["https://video.bsky.app/watch/did%3Aplc%3Aa/bafv/playlist.m3u8"]
    );
    app.poll_read_ahead(t0 + read_ahead::REST * 3);
    assert!(app.take_videos_ahead().is_empty());
}
