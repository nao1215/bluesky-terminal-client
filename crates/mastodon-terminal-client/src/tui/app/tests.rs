use super::*;
use crate::api::types::{Account, Mention};
use crate::api::{Next, Page};
use crate::error::Error;
use crate::tui::worker::Thread;
use proptest::prelude::*;

fn me() -> Session {
    Session {
        service: "http://127.0.0.1:9".into(),
        user_id: "7".into(),
        username: "me".into(),
        token: "fake-token-for-tests".into(),
        scopes: Vec::new(),
    }
}

fn account(acct: &str) -> Account {
    Account {
        id: format!("id-{acct}"),
        username: acct.split('@').next().unwrap_or(acct).into(),
        acct: acct.into(),
        ..Account::default()
    }
}

fn status(id: &str) -> Status {
    Status {
        id: id.into(),
        account: account("alice"),
        content: format!("<p>post {id} 👨‍👩‍👧</p>"),
        visibility: "public".into(),
        ..Status::default()
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn press(app: &mut App, code: KeyCode) -> Jobs {
    app.handle_key(key(code))
}

fn typed(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

fn page(items: Vec<Status>, next: Next) -> Event {
    Event::Timeline {
        kind: TimelineKind::Home,
        max_id: None,
        result: Ok(Page { items, next }),
    }
}

/// The app as an account, its home timeline holding `ids`.
fn with_home(ids: &[&str]) -> App {
    let (mut app, jobs) = App::new(Some(me()), "http://127.0.0.1:9");
    let (seq, _) = jobs[0].clone();
    let items = ids.iter().map(|i| status(i)).collect();
    app.handle_answer(seq, page(items, Next::End));
    app
}

fn home(app: &App) -> &List<Status> {
    app.timeline(TimelineKind::Home)
}

// An account opens on Home and asks for it and the server's limits; a
// visitor opens on Local, which the server shows to anyone.
#[test]
fn an_account_opens_on_home_and_a_visitor_on_local() {
    let (app, jobs) = App::new(Some(me()), "http://127.0.0.1:9");
    assert_eq!(app.tab, Tab::Home);
    assert_eq!(
        jobs.iter().map(|(_, j)| j.clone()).collect::<Vec<_>>(),
        [
            Job::Timeline {
                kind: TimelineKind::Home,
                max_id: None
            },
            Job::Limits
        ]
    );
    let (app, jobs) = App::new(None, "http://127.0.0.1:9");
    assert_eq!(app.tab, Tab::Local);
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Local,
            max_id: None
        }
    );
    assert_eq!(jobs.len(), 1);
}

// A visitor's Home and Notifications ask the server nothing: it shows
// them to no visitor.
#[test]
fn a_visitor_asks_for_no_tab_that_needs_an_account() {
    let (mut app, _) = App::new(None, "http://127.0.0.1:9");
    assert!(press(&mut app, KeyCode::Char('1')).is_empty());
    assert!(press(&mut app, KeyCode::Char('4')).is_empty());
    assert!(press(&mut app, KeyCode::Char('R')).is_empty());
    let jobs = press(&mut app, KeyCode::Char('3'));
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Public,
            max_id: None
        }
    );
}

// A page asked for before a refresh lands after the refresh's: it is
// dropped, and the list shows the newer page.
#[test]
fn a_page_answered_after_a_refresh_is_dropped() {
    let (mut app, jobs) = App::new(Some(me()), "http://127.0.0.1:9");
    let (old, _) = jobs[0].clone();
    let (new, _) = press(&mut app, KeyCode::Char('R'))[0].clone();
    app.handle_answer(new, page(vec![status("new")], Next::End));
    app.handle_answer(old, page(vec![status("old")], Next::End));
    let ids: Vec<&str> = home(&app).items.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["new"]);
    assert_eq!(app.pending(), 1, "only the limits are still waited for");
}

// Near the end the next page is asked for where the Link header said it
// starts; a page that brings nothing new ends the list.
#[test]
fn the_next_page_starts_where_the_server_said_and_a_repeat_ends_the_list() {
    let (mut app, jobs) = App::new(Some(me()), "http://127.0.0.1:9");
    app.handle_answer(
        jobs[0].0,
        page(vec![status("3"), status("2")], Next::MaxId("2".into())),
    );
    let jobs = press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Home,
            max_id: Some("2".into())
        }
    );
    // A second j while it is on its way asks nothing more.
    assert!(press(&mut app, KeyCode::Char('j')).is_empty());
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Home,
            max_id: Some("2".into()),
            result: Ok(Page {
                items: vec![status("2")],
                next: Next::MaxId("2".into()),
            }),
        },
    );
    assert_eq!(home(&app).items.len(), 2);
    assert_eq!(home(&app).next, None);
    assert!(press(&mut app, KeyCode::Char('j')).is_empty());
}

// A page with no Link header goes on from its last status.
#[test]
fn a_page_without_a_link_goes_on_from_its_last_status() {
    let (mut app, jobs) = App::new(Some(me()), "http://127.0.0.1:9");
    app.handle_answer(jobs[0].0, page(vec![status("9")], Next::Unsaid));
    assert_eq!(home(&app).next.as_deref(), Some("9"));
}

// A page that fails says why, and the list keeps what it had.
#[test]
fn a_page_that_fails_keeps_the_list_and_says_why() {
    let mut app = with_home(&["1"]);
    let (seq, _) = press(&mut app, KeyCode::Char('R'))[0].clone();
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            max_id: None,
            result: Err(Error::api("v1/timelines/home failed: busy")),
        },
    );
    assert_eq!(home(&app).items.len(), 1);
    assert_eq!(
        home(&app).error.as_deref(),
        Some("v1/timelines/home failed: busy")
    );
}

// l favourites at once wherever the status is shown, a boost of it on
// another tab too; the server saying no takes it back and says why.
#[test]
fn a_favourite_shows_at_once_everywhere_and_is_taken_back_when_refused() {
    let mut app = with_home(&["1"]);
    let jobs = press(&mut app, KeyCode::Char('2'));
    let mut boost = status("b");
    boost.reblog = Some(Box::new(status("1")));
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Local,
            max_id: None,
            result: Ok(Page {
                items: vec![boost],
                next: Next::End,
            }),
        },
    );
    press(&mut app, KeyCode::Char('1'));
    let jobs = press(&mut app, KeyCode::Char('l'));
    assert_eq!(
        jobs[0].1,
        Job::Favourite {
            id: "1".into(),
            on: true
        }
    );
    let fav = |app: &App| {
        let h = &app.timeline(TimelineKind::Home).items[0];
        let l = app.timeline(TimelineKind::Local).items[0].shown();
        (
            h.favourited,
            h.favourites_count,
            l.favourited,
            l.favourites_count,
        )
    };
    assert_eq!(fav(&app), (Some(true), 1, Some(true), 1));
    // A second l before the answer waits for it.
    assert!(press(&mut app, KeyCode::Char('l')).is_empty());
    app.handle_answer(
        jobs[0].0,
        Event::Favourited {
            id: "1".into(),
            on: true,
            result: Err(Error::api("v1/statuses/1/favourite failed: gone")),
        },
    );
    assert_eq!(fav(&app), (Some(false), 0, Some(false), 0));
    assert!(app.message.as_ref().unwrap().error);
    // The error goes with a key; answered, l works again.
    press(&mut app, KeyCode::Char('x'));
    assert_eq!(press(&mut app, KeyCode::Char('l')).len(), 1);
}

// b boosts and b again takes it back; a post for followers is not boosted.
#[test]
fn a_boost_is_taken_back_with_b_and_a_closed_post_is_not_boosted() {
    let mut app = with_home(&["1", "2"]);
    let jobs = press(&mut app, KeyCode::Char('b'));
    assert_eq!(
        jobs[0].1,
        Job::Boost {
            id: "1".into(),
            on: true
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Boosted {
            id: "1".into(),
            on: true,
            result: Ok(()),
        },
    );
    assert_eq!(home(&app).items[0].reblogs_count, 1);
    let jobs = press(&mut app, KeyCode::Char('b'));
    assert_eq!(
        jobs[0].1,
        Job::Boost {
            id: "1".into(),
            on: false
        }
    );
    app.timelines[0].items[1].visibility = "private".into();
    press(&mut app, KeyCode::Char('j'));
    assert!(press(&mut app, KeyCode::Char('b')).is_empty());
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .text
            .contains("cannot be boosted")
    );
}

// A visitor is told how to log in, and nothing is sent.
#[test]
fn a_visitor_is_told_to_log_in_to_write() {
    let (mut app, jobs) = App::new(None, "http://127.0.0.1:9");
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Local,
            max_id: None,
            result: Ok(Page {
                items: vec![status("1")],
                next: Next::End,
            }),
        },
    );
    for c in ['l', 'b', 'r', 'n'] {
        assert!(press(&mut app, KeyCode::Char(c)).is_empty(), "{c}");
        assert!(app.overlay.is_none(), "{c}");
        assert_eq!(
            app.message.as_ref().unwrap().text,
            "log in to do that: run `mstdn login --host 127.0.0.1:9`"
        );
    }
}

// w shows the text behind a warning and hides it again.
#[test]
fn w_shows_and_hides_the_text_behind_a_warning() {
    let mut app = with_home(&["1"]);
    app.timelines[0].items[0].spoiler_text = "ネタバレ".into();
    let s = app.selected().unwrap().clone();
    assert!(!app.text_shown(&s));
    press(&mut app, KeyCode::Char('w'));
    assert!(app.text_shown(&s));
    press(&mut app, KeyCode::Char('w'));
    assert!(!app.text_shown(&s));
}

// The composer sends the text, the warning and who may see it, once.
#[test]
fn the_composer_sends_the_text_warning_and_visibility() {
    let mut app = with_home(&["1"]);
    press(&mut app, KeyCode::Char('n'));
    typed(&mut app, "こんにちは 🇯🇵");
    app.handle_key(ctrl('w'));
    typed(&mut app, "ネタバレ");
    app.handle_key(ctrl('p'));
    let jobs = app.handle_key(ctrl('s'));
    assert_eq!(
        jobs[0].1,
        Job::Post(Box::new(Draft {
            quote: None,
            quote_policy: None,
            files: Vec::new(),
            text: "こんにちは 🇯🇵".into(),
            cw: "ネタバレ".into(),
            visibility: Visibility::Unlisted,
            reply: None,
        }))
    );
    // While it is on its way, keys do not change it, and it is not sent again.
    assert!(app.handle_key(ctrl('s')).is_empty());
    app.handle_answer(jobs[0].0, Event::Posted(Ok(Box::new(status("new")))));
    assert!(app.overlay.is_none());
    assert_eq!(app.message.as_ref().unwrap().text, "posted");
}

// A post the server refuses leaves the composer open with its text, to
// send again.
#[test]
fn a_refused_post_keeps_the_text() {
    let mut app = with_home(&[]);
    press(&mut app, KeyCode::Char('n'));
    typed(&mut app, "hi");
    let jobs = app.handle_key(ctrl('s'));
    app.handle_answer(
        jobs[0].0,
        Event::Posted(Err(Error::api("v1/statuses failed: no"))),
    );
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("the composer closed");
    };
    assert_eq!(c.text.text(), "hi");
    assert!(c.sending.is_none());
    // The error goes with a key, and the next one sends again.
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(app.handle_key(ctrl('s')).len(), 1);
}

#[test]
fn unsupported_actions_restore_state_and_deliver_the_error_once() {
    for key_char in ['l', 'b'] {
        let mut app = with_home(&["1", "2"]);
        let jobs = press(&mut app, KeyCode::Char(key_char));
        let error = Error::unsupported("reactions", "127.0.0.1:9");
        let event = || {
            if key_char == 'l' {
                Event::Favourited {
                    id: "1".into(),
                    on: true,
                    result: Err(error.clone()),
                }
            } else {
                Event::Boosted {
                    id: "1".into(),
                    on: true,
                    result: Err(error.clone()),
                }
            }
        };
        app.handle_answer(jobs[0].0, event());
        assert!(app.acting.is_empty());
        let s = app.selected().unwrap();
        assert_eq!((s.favourites_count, s.reblogs_count), (0, 0));
        assert!(!s.favourited.unwrap_or(false));
        assert!(!s.reblogged.unwrap_or(false));
        assert_eq!(app.message.as_ref().unwrap().text, error.message());
        press(&mut app, KeyCode::Char('x'));
        app.handle_answer(jobs[0].0, event());
        assert!(app.message.is_none());
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected().unwrap().id, "2");
    }
    let mut app = with_home(&[]);
    press(&mut app, KeyCode::Char('n'));
    typed(&mut app, "keep this draft");
    let jobs = app.handle_key(ctrl('s'));
    app.handle_answer(
        jobs[0].0,
        Event::Posted(Err(Error::unsupported("posts", "127.0.0.1:9"))),
    );
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("draft lost")
    };
    assert_eq!(c.text.text(), "keep this draft");
    assert!(c.sending.is_none());
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Esc);
    assert!(app.overlay.is_none());
    assert!(!app.quit);
}

// Nothing, or more than the server takes, is not sent.
#[test]
fn an_empty_or_too_long_post_is_not_sent() {
    let mut app = with_home(&[]);
    press(&mut app, KeyCode::Char('n'));
    typed(&mut app, "  ");
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert_eq!(app.message.as_ref().unwrap().text, "write something first");
    app.limits.max_characters = 5;
    typed(&mut app, "👨‍👩‍👧👨‍👩‍👧👨‍👩‍👧👨‍👩‍👧");
    // Graphemes, as Mastodon counts: two spaces and four families are 6.
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert_eq!(
        app.message.as_ref().unwrap().text,
        "too long: 6 of 5 characters"
    );
}

// r answers the selected status: its author and those it mentions, not
// yourself, its warning and who may see it.
#[test]
fn a_reply_mentions_the_others_and_keeps_the_warning_and_visibility() {
    let mut app = with_home(&["1"]);
    {
        let s = &mut app.timelines[0].items[0];
        s.account = account("bob@x.test");
        s.spoiler_text = "注意".into();
        s.visibility = "private".into();
        s.mentions = vec![
            Mention {
                acct: "me".into(),
                ..Mention::default()
            },
            Mention {
                acct: "carol".into(),
                ..Mention::default()
            },
            Mention {
                acct: "BOB@x.test".into(),
                ..Mention::default()
            },
        ];
    }
    press(&mut app, KeyCode::Char('r'));
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("no composer");
    };
    assert_eq!(c.text.text(), "@bob@x.test @carol ");
    assert_eq!(c.warning.as_ref().unwrap().text(), "注意");
    assert_eq!(c.visibility, Visibility::Private);
    assert_eq!(c.reply, Some(("1".into(), "@bob@x.test".into())));
    let jobs = app.handle_key(ctrl('s'));
    let Job::Post(d) = &jobs[0].1 else {
        panic!("{jobs:?}");
    };
    assert_eq!(d.reply.as_deref(), Some("1"));
}

// Pasted text goes into the field with the keys, line breaks and all.
#[test]
fn a_paste_goes_into_the_composer() {
    let mut app = with_home(&[]);
    app.handle_paste("ignored");
    press(&mut app, KeyCode::Char('n'));
    app.handle_paste("一行目\nq");
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("no composer");
    };
    assert_eq!(c.text.text(), "一行目\nq");
    assert!(!app.quit);
}

// v opens the thread on the status, which stays selected when what it
// answers comes above it; esc closes it.
#[test]
fn a_thread_opens_on_its_status_and_esc_closes_it() {
    let mut app = with_home(&["2"]);
    let jobs = press(&mut app, KeyCode::Char('v'));
    assert_eq!(jobs[0].1, Job::Thread { id: "2".into() });
    assert_eq!(app.threads.len(), 1);
    let mut reply = status("3");
    reply.in_reply_to_id = Some("2".into());
    let mut deeper = status("4");
    deeper.in_reply_to_id = Some("3".into());
    app.handle_answer(
        jobs[0].0,
        Event::Thread {
            id: "2".into(),
            result: Ok(Box::new(Thread {
                ancestors: vec![status("1")],
                status: status("2"),
                descendants: vec![reply, deeper],
            })),
        },
    );
    let rows = &app.threads[0].rows;
    assert_eq!(app.selected().unwrap().id, "2");
    assert_eq!(
        rows.items.iter().map(|r| r.depth).collect::<Vec<_>>(),
        [0, 0, 1, 2]
    );
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(app.selected().unwrap().id, "3");
    // v on a reply opens its own thread over this one.
    press(&mut app, KeyCode::Char('v'));
    assert_eq!(app.threads.len(), 2);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Esc);
    assert!(app.threads.is_empty());
    assert_eq!(app.selected().unwrap().id, "2");
}

// A notification's keys act on the status it is about.
#[test]
fn a_notifications_keys_act_on_its_status() {
    let mut app = with_home(&[]);
    let jobs = press(&mut app, KeyCode::Char('4'));
    assert_eq!(jobs[0].1, Job::Notifications { max_id: None });
    app.handle_answer(
        jobs[0].0,
        Event::Notifications {
            max_id: None,
            result: Ok(Page {
                items: vec![
                    Notification {
                        id: "n1".into(),
                        kind: "follow".into(),
                        ..Notification::default()
                    },
                    Notification {
                        id: "n2".into(),
                        kind: "mention".into(),
                        status: Some(status("9")),
                        ..Notification::default()
                    },
                ],
                next: Next::End,
            }),
        },
    );
    assert!(
        press(&mut app, KeyCode::Char('l')).is_empty(),
        "a follow has no post"
    );
    press(&mut app, KeyCode::Char('j'));
    assert_eq!(
        press(&mut app, KeyCode::Char('l'))[0].1,
        Job::Favourite {
            id: "9".into(),
            on: true
        }
    );
}

// T previews each theme as it moves; esc goes back to the one before,
// enter keeps the one shown for the next run.
#[test]
fn the_theme_picker_previews_and_enter_keeps_it() {
    let mut app = with_home(&[]);
    app.apply_settings(Some(Settings::default()), ColorDepth::TrueColor);
    let before = app.theme;
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Char('j'));
    assert_ne!(app.theme, before);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.theme, before);
    assert!(app.take_settings_save().is_none());
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Char('G'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.theme.name, "monochrome");
    let saved = app.take_settings_save().unwrap();
    assert_eq!(saved.theme.as_deref(), Some("monochrome"));
    assert!(app.take_settings_save().is_none(), "saved once");
}

// Without a config directory the theme lasts until mstdn ends, and it says so.
#[test]
fn a_theme_with_nowhere_to_keep_it_is_used_for_this_run() {
    let mut app = with_home(&[]);
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Char('k'));
    press(&mut app, KeyCode::Enter);
    assert!(app.take_settings_save().is_none());
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .text
            .contains("until mstdn ends")
    );
}

// An error over the screen takes the next key, which does nothing else.
#[test]
fn an_error_takes_the_next_key() {
    let mut app = with_home(&["1"]);
    app.fail("bad");
    press(&mut app, KeyCode::Char('q'));
    assert!(!app.quit);
    assert!(app.message.is_none());
    press(&mut app, KeyCode::Char('q'));
    assert!(app.quit);
}

// A message goes after a while, an error after longer.
#[test]
fn a_message_goes_by_itself() {
    let mut app = with_home(&[]);
    app.say("hi");
    let now = Instant::now();
    assert!(!app.expire_message(now));
    assert!(app.expire_message(now + MESSAGE_FOR + Duration::from_secs(1)));
    app.fail("bad");
    assert!(!app.expire_message(Instant::now() + MESSAGE_FOR + Duration::from_secs(1)));
}

// `.` lists what the keys do now, and enter does the one chosen.
#[test]
fn the_actions_list_says_what_each_key_does_now_and_does_it() {
    let mut app = with_home(&["1"]);
    app.timelines[0].items[0].favourited = Some(true);
    press(&mut app, KeyCode::Char('.'));
    let listed = keys::actions(&app);
    assert!(
        listed.contains(&("l", "take back your favourite")),
        "{listed:?}"
    );
    press(&mut app, KeyCode::Char('j'));
    let jobs = press(&mut app, KeyCode::Enter);
    assert_eq!(
        jobs[0].1,
        Job::Favourite {
            id: "1".into(),
            on: false
        }
    );
    assert!(app.overlay.is_none());
}

// c copies the post's page, and o opens it.
#[test]
fn c_copies_and_o_opens_the_posts_page() {
    let mut app = with_home(&["1"]);
    app.timelines[0].items[0].url = Some("https://m.test/@alice/1".into());
    press(&mut app, KeyCode::Char('c'));
    assert_eq!(app.take_copy().as_deref(), Some("https://m.test/@alice/1"));
    assert_eq!(
        press(&mut app, KeyCode::Char('o'))[0].1,
        Job::OpenLink {
            program: None,
            url: "https://m.test/@alice/1".into()
        }
    );
}

/// The keys the random sequences are made of.
fn any_key() -> impl Strategy<Value = KeyEvent> {
    prop_oneof![
        prop::sample::select(vec![
            'j', 'k', 'g', 'G', 'v', 'w', 'l', 'b', 'r', 'n', '.', 'T', '?', 'R', '1', '2', '3',
            '4', '5', '6', '/', 's', 'A', 'd', ' ', 'x', '👍',
        ])
        .prop_map(|c| key(KeyCode::Char(c))),
        prop::sample::select(vec![
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::PageDown,
            KeyCode::PageUp,
            KeyCode::Backspace,
        ])
        .prop_map(key),
        prop::sample::select(vec!['s', 'w', 'p', 'u']).prop_map(ctrl),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Whatever keys come, with answers between them, nothing panics, the
    // selection stays in its list, and every job is numbered once.
    #[test]
    fn any_keys_keep_the_selection_in_the_list(keys in prop::collection::vec(any_key(), 1..80)) {
        let mut app = with_home(&["1", "2", "3"]);
        let mut seen = HashSet::new();
        for (i, k) in keys.into_iter().enumerate() {
            for (seq, job) in app.handle_key(k) {
                prop_assert!(seen.insert(seq));
                let ev = match job {
                    Job::NotificationState => Event::NotificationState(Ok((None, Some(0)))),
                    Job::MarkNotifications { id } => Event::NotificationsMarked { id, result: Ok(()) },
                    Job::Conversations { next } => Event::Conversations { first:next.is_none(),result:Err(Error::api("read failed")) },
                    Job::ChatThread { .. } => Event::ChatThread(Err(Error::api("read failed"))),
                    Job::ReadConversation { .. } => Event::ConversationRead(Err(Error::api("write failed"))),
                    Job::ChatPost(_) => Event::ChatPosted(Err(Error::api("write failed"))),
                    Job::ColumnChoices => Event::ColumnChoices(Ok(Vec::new())),
                    Job::Column { id, next, .. } => Event::Column { id, first:next.is_none(), result:Err(Error::api("unavailable")) },
                    Job::ProfileEditor => Event::ProfileEditor(Err(Error::api("unavailable"))),
                    Job::SaveProfile(_) => Event::ProfileSaved(Err(Error::api("refused"))),
                    Job::Identity => Event::Identity(Ok(account("me"))),
                    Job::Relationship { id } => Event::Relationship { id, result: Err(Error::api("read failed")) },
                    Job::Relate { id, .. } => Event::Related { id, result: Err(Error::api("write failed")) },
                    Job::Delete { id } => Event::Deleted { id, result: Ok(()) },
                    Job::Report { .. } => Event::Reported(Ok(())),
                    Job::Timeline { kind, max_id } => Event::Timeline {
                        kind,
                        max_id,
                        result: Ok(Page { items: vec![status(&format!("p{i}"))], next: Next::End }),
                    },
                    Job::Notifications { max_id } => Event::Notifications {
                        max_id,
                        result: Ok(Page { items: Vec::new(), next: Next::End }),
                    },
                    Job::Thread { id } => Event::Thread {
                        result: Ok(Box::new(Thread { status: status(&id), ..Thread::default() })),
                        id,
                    },
                    Job::Favourite { id, on } => Event::Favourited { id, on, result: Ok(()) },
                    Job::Boost { id, on } => Event::Boosted { id, on, result: Err(Error::api("no")) },
                    Job::Post(_) => Event::Posted(Ok(Box::new(status("posted")))),
                    Job::OpenLink { .. } => Event::Opened(true),
                    Job::Limits => Event::Limits(Limits::default()),
                    Job::Search { next, .. } => Event::Search { next, result: Err(Error::api("search failed")) },
                    Job::Profile { max_id, .. } => Event::Profile { more: max_id.is_some(), result: Err(Error::api("profile failed")) },
                    Job::Download { .. } => Event::Downloaded(Ok(1)),
                };
                app.handle_answer(seq, ev);
            }
            if app.quit {
                break;
            }
            for l in &app.timelines {
                prop_assert!(l.items.is_empty() || l.selected < l.items.len());
            }
            if let Some(th) = app.threads.last() {
                prop_assert!(th.rows.selected < th.rows.items.len());
            }
        }
    }
}

#[test]
fn search_input_gates_visitors_and_stale_results_do_not_replace_new_queries() {
    let (mut app, _) = App::new(None, "http://127.0.0.1:9");
    assert!(press(&mut app, KeyCode::Char('5')).is_empty());
    assert!(app.search.editing);
    typed(&mut app, "rust");
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    assert!(app.message.as_ref().unwrap().text.contains("#hashtags"));
    press(&mut app, KeyCode::Char('/'));
    app.search.input = TextInput::single("#rust");
    let old = press(&mut app, KeyCode::Enter).remove(0);
    assert_eq!(
        old.1,
        Job::Search {
            query: "#rust".into(),
            accounts: false,
            next: None
        }
    );
    press(&mut app, KeyCode::Char('/'));
    app.search.input = TextInput::single("#日本語");
    let fresh = press(&mut app, KeyCode::Enter).remove(0);
    for (seq, id) in [(fresh.0, "new"), (old.0, "old")] {
        app.handle_answer(
            seq,
            Event::Search {
                next: None,
                result: Ok(crate::tui::worker::SearchPage {
                    posts: vec![status(id)],
                    accounts: Vec::new(),
                    next: Some("cursor".into()),
                }),
            },
        );
    }
    assert_eq!(app.selected().unwrap().id, "new");
    let jobs = press(&mut app, KeyCode::Char('j'));
    assert!(
        matches!(&jobs[0].1, Job::Search { next: Some(n), query, .. } if n == "cursor" && query == "#日本語")
    );
    assert!(press(&mut app, KeyCode::Char('t')).is_empty());
    assert!(app.search.accounts);
}

#[test]
fn search_accounts_and_profiles_use_the_selected_account_and_page_its_posts() {
    let mut app = with_home(&["1"]);
    press(&mut app, KeyCode::Char('/'));
    app.handle_paste("ferris");
    app.handle_key(ctrl('t'));
    let (seq, job) = press(&mut app, KeyCode::Enter).remove(0);
    assert_eq!(
        job,
        Job::Search {
            query: "ferris".into(),
            accounts: true,
            next: None
        }
    );
    let user = account("ferris");
    app.handle_answer(
        seq,
        Event::Search {
            next: None,
            result: Ok(crate::tui::worker::SearchPage {
                posts: Vec::new(),
                accounts: vec![user.clone()],
                next: None,
            }),
        },
    );
    let (seq, job) = press(&mut app, KeyCode::Enter).remove(0);
    assert_eq!(
        job,
        Job::Profile {
            id: Some(user.id.clone()),
            max_id: None
        }
    );
    app.handle_answer(
        seq,
        Event::Profile {
            more: false,
            result: Ok(crate::tui::worker::ProfilePage {
                account: user.clone(),
                posts: Page {
                    items: vec![status("p1")],
                    next: Next::MaxId("p1".into()),
                },
            }),
        },
    );
    assert_eq!(app.tab, Tab::Profile);
    assert_eq!(app.selected().unwrap().id, "p1");
    let (seq, job) = press(&mut app, KeyCode::Char('j')).remove(0);
    assert_eq!(
        job,
        Job::Profile {
            id: Some(user.id),
            max_id: Some("p1".into())
        }
    );
    app.handle_answer(
        seq,
        Event::Profile {
            more: true,
            result: Err(Error::api("offline")),
        },
    );
    assert_eq!(app.selected().unwrap().id, "p1");
    assert_eq!(app.profile.posts.error.as_deref(), Some("offline"));
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.tab, Tab::Search);
    let jobs = press(&mut app, KeyCode::Char('6'));
    assert_eq!(
        jobs[0].1,
        Job::Profile {
            id: None,
            max_id: None
        }
    );
}

#[test]
fn enter_opens_the_author_v_opens_the_thread_and_profiles_reject_old_answers() {
    let mut app = with_home(&["1", "2"]);
    let (seq, job) = press(&mut app, KeyCode::Enter).remove(0);
    assert!(matches!(job, Job::Profile { id: Some(_), .. }));
    let current = press(&mut app, KeyCode::Char('6')).remove(0);
    app.handle_answer(
        seq,
        Event::Profile {
            more: false,
            result: Ok(crate::tui::worker::ProfilePage {
                account: account("stale"),
                posts: Page {
                    items: vec![status("stale")],
                    next: Next::End,
                },
            }),
        },
    );
    assert!(app.profile.account.is_none());
    assert_eq!(app.profile.posts.waiting, Some(current.0));
    press(&mut app, KeyCode::Char('1'));
    assert!(matches!(
        press(&mut app, KeyCode::Char('v'))[0].1,
        Job::Thread { .. }
    ));
}

#[test]
fn media_viewer_respects_warning_steps_replays_downloads_and_closes() {
    use crate::api::types::Media;
    let mut app = with_home(&["1"]);
    let s = &mut app.timelines[0].items[0];
    s.spoiler_text = "spoiler".into();
    s.media_attachments = vec![
        Media {
            kind: "image".into(),
            url: Some("http://127.0.0.1:9/p.png".into()),
            ..Default::default()
        },
        Media {
            kind: "video".into(),
            url: Some("http://127.0.0.1:9/v.mp4".into()),
            ..Default::default()
        },
    ];
    press(&mut app, KeyCode::Char(' '));
    assert!(app.overlay.is_none());
    press(&mut app, KeyCode::Char('w'));
    press(&mut app, KeyCode::Char(' '));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Viewer { index: 0, .. })
    ));
    press(&mut app, KeyCode::Right);
    press(&mut app, KeyCode::Char('r'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Viewer {
            index: 1,
            generation: 1,
            ..
        })
    ));
    let (seq, job) = press(&mut app, KeyCode::Char('d')).remove(0);
    assert!(matches!(job, Job::Download { files, id, .. } if files.len() == 2 && id == "1"));
    app.handle_answer(seq, Event::Downloaded(Err(Error::io("disk full"))));
    assert!(app.message.as_ref().unwrap().error);
    press(&mut app, KeyCode::Esc); // dismiss error
    press(&mut app, KeyCode::Char('q'));
    assert!(app.overlay.is_none());
    assert!(!app.quit);
}

#[test]
fn account_switch_drops_old_answers_including_writes_and_keeps_settings() {
    let mut app = with_home(&["1"]);
    app.apply_settings(Some(Settings::default()), ColorDepth::TrueColor);
    let old = press(&mut app, KeyCode::Char('l')).remove(0);
    let mut other = me();
    other.service = "http://127.0.0.1:8".into();
    other.username = "other".into();
    app.accounts = vec![me(), other.clone()];
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.take_switch(), Some(other.clone()));
    let jobs = app.switched(other);
    assert!(jobs.iter().all(|(n, _)| *n > old.0));
    app.handle_answer(
        old.0,
        Event::Favourited {
            id: "1".into(),
            on: true,
            result: Err(Error::api("old account failed")),
        },
    );
    assert!(app.message.is_none());
    assert!(app.timelines[0].items.is_empty());
    assert!(app.settings.is_some());
    assert_eq!(app.host(), "127.0.0.1:8");
}

#[test]
fn settings_language_overrides_locale_persists_and_reset_restores_locale() {
    use crate::i18n::{self, Lang};
    i18n::set(Lang::Ja);
    let mut app = with_home(&["1"]);
    let mut settings = Settings::default();
    settings.language = Some("en".into());
    app.apply_settings(Some(settings), ColorDepth::TrueColor);
    assert_eq!(i18n::current(), Lang::En);
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    assert_eq!(i18n::current(), Lang::Ja);
    assert_eq!(
        app.take_settings_save().unwrap().language.as_deref(),
        Some("ja")
    );
    assert!(matches!(app.overlay, Some(Overlay::Settings { .. })));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('x'));
    assert!(app.take_settings_save().unwrap().language.is_none());
    assert_eq!(i18n::current(), Lang::Ja);
    i18n::set(Lang::En);
}

#[test]
fn broken_settings_are_never_written_by_the_language_picker() {
    let mut app = with_home(&[]);
    app.apply_settings(None, ColorDepth::TrueColor);
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(app.take_settings_save().is_none());
}

#[test]
fn account_toggles_restore_the_entire_relationship_on_refusal() {
    for (key_char, action) in [
        ('f', RelationAction::Follow),
        ('M', RelationAction::Mute),
        ('B', RelationAction::Block),
    ] {
        for active in [false, true] {
            let mut app = with_home(&["one"]);
            let before = Relationship {
                id: "id-alice".into(),
                following: active,
                requested: active,
                muting: active,
                blocking: active,
            };
            app.relationships.insert(before.id.clone(), before.clone());
            let mut jobs = press(&mut app, KeyCode::Char(key_char));
            if key_char != 'f' {
                assert!(jobs.is_empty());
                jobs = press(&mut app, KeyCode::Char('y'));
            }
            assert!(
                matches!(&jobs[0].1, Job::Relate { id, action: a, on } if id == "id-alice" && *a == action && *on != active)
            );
            assert_eq!(action.active(&app.relationships["id-alice"]), !active);
            assert!(
                press(&mut app, KeyCode::Char(key_char)).is_empty(),
                "duplicate write"
            );
            app.handle_answer(
                jobs[0].0,
                Event::Related {
                    id: "id-alice".into(),
                    result: Err(Error::api("refused")),
                },
            );
            assert_eq!(app.relationships["id-alice"], before);
            assert!(app.message.as_ref().unwrap().error);
            assert!(app.relation_pending.is_empty());
        }
    }
}

#[test]
fn an_unknown_relationship_is_read_before_toggling_and_a_pending_follow_is_cancelled() {
    let mut app = with_home(&["one"]);
    let jobs = press(&mut app, KeyCode::Char('f'));
    assert!(matches!(&jobs[0].1, Job::Relationship { id } if id == "id-alice"));
    assert!(press(&mut app, KeyCode::Char('f')).is_empty());
    let jobs = app.handle_answer(
        jobs[0].0,
        Event::Relationship {
            id: "id-alice".into(),
            result: Ok(Relationship {
                id: "id-alice".into(),
                requested: true,
                ..Default::default()
            }),
        },
    );
    assert!(matches!(&jobs[0].1, Job::Relate { on: false, .. }));
    app.handle_answer(
        jobs[0].0,
        Event::Related {
            id: "id-alice".into(),
            result: Ok(Relationship {
                id: "id-alice".into(),
                ..Default::default()
            }),
        },
    );
    assert!(!app.relationships["id-alice"].requested);
}

#[test]
fn a_failed_relationship_read_never_starts_a_write_and_can_be_retried() {
    let mut app = with_home(&["one"]);
    let jobs = press(&mut app, KeyCode::Char('f'));
    assert!(
        app.handle_answer(
            jobs[0].0,
            Event::Relationship {
                id: "id-alice".into(),
                result: Err(Error::api("no scope"))
            }
        )
        .is_empty()
    );
    assert!(app.relationships.is_empty());
    press(&mut app, KeyCode::Esc); // dismiss error
    assert_eq!(press(&mut app, KeyCode::Char('f')).len(), 1);
}

#[test]
fn only_y_confirms_account_actions_and_deletion() {
    for action in ['M', 'B', 'D'] {
        for answer in [
            KeyCode::Char('n'),
            KeyCode::Char('Y'),
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('q'),
        ] {
            let mut app = with_home(&["one"]);
            if action == 'D' {
                app.timelines[0].items[0].account.id = me().user_id;
            }
            assert!(press(&mut app, KeyCode::Char(action)).is_empty());
            assert!(matches!(app.overlay, Some(Overlay::Confirm(_))));
            assert!(press(&mut app, answer).is_empty());
            assert!(app.overlay.is_none());
            assert!(!app.quit);
            assert!(app.relation_pending.is_empty());
            assert!(app.deleting.is_empty());
        }
    }
}

#[test]
fn account_actions_target_the_profile_and_threads_target_the_selected_author() {
    let mut app = with_home(&["one"]);
    app.tab = Tab::Profile;
    app.profile.account = Some(account("bob"));
    app.profile.posts.items.push(status("other"));
    let jobs = press(&mut app, KeyCode::Char('f'));
    assert!(matches!(&jobs[0].1, Job::Relationship { id } if id == "id-bob"));
    press(&mut app, KeyCode::Char('v'));
    let jobs = press(&mut app, KeyCode::Char('f'));
    assert!(matches!(&jobs[0].1, Job::Relationship { id } if id == "id-alice"));
}

#[test]
fn deletion_waits_for_success_and_removes_all_occurrences_including_late_pages() {
    let mut app = with_home(&["one", "two"]);
    app.timelines[0].items[0].account.id = me().user_id;
    let own = app.timelines[0].items[0].clone();
    app.search.posts.items.push(own.clone());
    app.profile.posts.items.push(own.clone());
    app.timelines[1].items.push(Status {
        id: "boost".into(),
        reblog: Some(Box::new(own.clone())),
        ..Default::default()
    });
    app.notifications.items.push(Notification {
        status: Some(own.clone()),
        ..Default::default()
    });
    press(&mut app, KeyCode::Char('v'));
    press(&mut app, KeyCode::Char('D'));
    let jobs = press(&mut app, KeyCode::Char('y'));
    assert!(app.selected().is_some());
    app.handle_answer(
        jobs[0].0,
        Event::Deleted {
            id: "one".into(),
            result: Err(Error::api("refused")),
        },
    );
    assert_eq!(app.selected().unwrap().id, "one");
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('D'));
    let jobs = press(&mut app, KeyCode::Char('y'));
    app.handle_answer(
        jobs[0].0,
        Event::Deleted {
            id: "one".into(),
            result: Ok(()),
        },
    );
    assert!(app.threads[0].rows.items.is_empty());
    assert!(app.notifications.items.is_empty());
    assert!(app.search.posts.items.is_empty());
    assert!(app.profile.posts.items.is_empty());
    assert!(app.timelines[1].items.is_empty());
    assert_eq!(app.timelines[0].items[0].id, "two");
    press(&mut app, KeyCode::Esc);
    let jobs = press(&mut app, KeyCode::Char('R'));
    app.handle_answer(jobs[0].0, page(vec![own, status("two")], Next::End));
    assert_eq!(app.timelines[0].items.len(), 1);
}

#[test]
fn visitors_and_other_peoples_posts_cannot_be_deleted() {
    let mut app = with_home(&["one"]);
    assert!(press(&mut app, KeyCode::Char('D')).is_empty());
    assert!(app.overlay.is_none());
    app.session = None;
    for action in ['f', 'M', 'B', 'D', '!'] {
        assert!(press(&mut app, KeyCode::Char(action)).is_empty());
        assert!(app.overlay.is_none());
    }
}

#[test]
fn reporting_requires_y_after_confirmation_and_keeps_the_comment_on_refusal() {
    let mut app = with_home(&["one"]);
    press(&mut app, KeyCode::Char('!'));
    press(&mut app, KeyCode::Tab);
    app.handle_paste("optional comment 🐈");
    press(&mut app, KeyCode::Tab);
    for answer in [
        KeyCode::Enter,
        KeyCode::Char('n'),
        KeyCode::Char('Y'),
        KeyCode::Esc,
    ] {
        assert!(press(&mut app, KeyCode::Enter).is_empty());
        assert!(press(&mut app, answer).is_empty());
    }
    press(&mut app, KeyCode::Enter);
    let jobs = press(&mut app, KeyCode::Char('y'));
    assert!(
        matches!(&jobs[0].1, Job::Report { account, status, comment } if account == "id-alice" && status.as_deref() == Some("one") && comment == "optional comment 🐈")
    );
    assert!(press(&mut app, KeyCode::Char('y')).is_empty());
    app.handle_answer(jobs[0].0, Event::Reported(Err(Error::api("refused"))));
    assert!(
        matches!(&app.overlay, Some(Overlay::Report(r)) if r.sending.is_none() && r.comment.text() == "optional comment 🐈")
    );
    press(&mut app, KeyCode::Esc); // error
    press(&mut app, KeyCode::Enter);
    let jobs = press(&mut app, KeyCode::Char('y'));
    app.handle_answer(jobs[0].0, Event::Reported(Ok(())));
    assert!(app.overlay.is_none());
}

#[test]
fn a_profile_without_a_selected_post_reports_only_the_account() {
    let mut app = with_home(&[]);
    app.tab = Tab::Profile;
    app.profile.account = Some(account("bob"));
    press(&mut app, KeyCode::Char('!'));
    press(&mut app, KeyCode::Enter);
    let jobs = press(&mut app, KeyCode::Char('y'));
    assert!(
        matches!(&jobs[0].1, Job::Report { account, status: None, comment } if account == "id-bob" && comment.is_empty())
    );
}

#[test]
fn switching_accounts_discards_relationship_state_and_old_answers() {
    let mut app = with_home(&["one"]);
    let jobs = press(&mut app, KeyCode::Char('f'));
    app.switched(Session {
        user_id: "another".into(),
        ..me()
    });
    assert!(
        app.handle_answer(
            jobs[0].0,
            Event::Relationship {
                id: "id-alice".into(),
                result: Ok(Relationship {
                    id: "id-alice".into(),
                    ..Default::default()
                })
            }
        )
        .is_empty()
    );
    assert!(app.relationships.is_empty());
}

#[test]
fn the_shared_browser_returns_to_the_composer_and_tab_edits_alt_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("picture.png"), b"test").unwrap();
    let mut app = with_home(&[]);
    app.browse_from = Some(dir.path().to_path_buf());
    press(&mut app, KeyCode::Char('n'));
    app.handle_key(ctrl('o'));
    assert!(matches!(&app.overlay, Some(Overlay::Compose(c)) if c.browser.is_some()));
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Tab);
    app.handle_paste("alt 🐈");
    let jobs = app.handle_key(ctrl('s'));
    let Job::Post(d) = &jobs[0].1 else { panic!() };
    assert!(d.text.is_empty(), "an attachment alone can be posted");
    assert_eq!(
        d.files,
        vec![(dir.path().join("picture.png"), "alt 🐈".into())]
    );
    app.handle_answer(jobs[0].0, Event::Posted(Err(Error::api("too large"))));
    press(&mut app, KeyCode::Esc);
    assert!(
        matches!(&app.overlay, Some(Overlay::Compose(c)) if c.media.len() == 1 && c.sending.is_none())
    );
    app.handle_key(ctrl('x'));
    assert!(
        matches!(&app.overlay, Some(Overlay::Compose(c)) if c.media.is_empty() && c.focus == Field::Text)
    );
}

#[test]
fn browser_cancel_and_mixed_media_never_start_uploads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.png"), b"test").unwrap();
    std::fs::write(dir.path().join("b.mp4"), b"test").unwrap();
    let mut app = with_home(&[]);
    app.browse_from = Some(dir.path().to_path_buf());
    press(&mut app, KeyCode::Char('n'));
    app.handle_key(ctrl('o'));
    assert!(press(&mut app, KeyCode::Esc).is_empty());
    assert!(
        matches!(&app.overlay, Some(Overlay::Compose(c)) if c.browser.is_none() && c.media.is_empty())
    );
    app.handle_key(ctrl('o'));
    press(&mut app, KeyCode::Char(' ')); // a.png, then cursor on b.mp4
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    assert!(matches!(&app.overlay, Some(Overlay::Compose(c)) if c.media.is_empty()));
    assert!(app.message.as_ref().unwrap().error);
}

#[test]
fn token_only_sessions_learn_their_identity_before_own_post_deletion() {
    let (mut app, jobs) = App::new(
        Some(Session {
            user_id: String::new(),
            username: String::new(),
            ..me()
        }),
        "http://127.0.0.1:9",
    );
    let identity = jobs.iter().find(|(_, j)| *j == Job::Identity).unwrap().0;
    app.handle_answer(
        identity,
        Event::Identity(Ok(Account {
            id: "7".into(),
            username: "me".into(),
            ..Default::default()
        })),
    );
    let mut own = status("one");
    own.account.id = "7".into();
    app.handle_answer(jobs[0].0, page(vec![own], Next::End));
    press(&mut app, KeyCode::Char('D'));
    assert!(matches!(&press(&mut app, KeyCode::Char('y'))[0].1, Job::Delete { id } if id == "one"));
}

#[test]
fn read_ahead_waits_adopts_and_invalidates_on_a_refused_write() {
    let mut app = with_home(&["s"]);
    let now = Instant::now();
    app.poll_read_ahead(now);
    assert!(
        app.poll_read_ahead(now + Duration::from_millis(399))
            .is_empty()
    );
    let jobs = app.poll_read_ahead(now + Duration::from_millis(400));
    assert_eq!(jobs.len(), 1);
    let seq = jobs[0].0;
    assert!(press(&mut app, KeyCode::Char('v')).is_empty());
    app.handle_answer(
        seq,
        Event::Thread {
            id: "s".into(),
            result: Ok(Box::new(Thread {
                status: status("s"),
                descendants: vec![status("reply")],
                ..Thread::default()
            })),
        },
    );
    assert_eq!(app.threads[0].rows.items.len(), 2);
    press(&mut app, KeyCode::Esc);
    assert!(press(&mut app, KeyCode::Char('v')).is_empty());
    press(&mut app, KeyCode::Esc);
    let jobs = press(&mut app, KeyCode::Char('l'));
    app.handle_answer(
        jobs[0].0,
        Event::Favourited {
            id: "s".into(),
            on: true,
            result: Err(Error::api("refused")),
        },
    );
    press(&mut app, KeyCode::Esc);
    assert_eq!(home(&app).items[0].favourited, Some(false));
    assert_eq!(press(&mut app, KeyCode::Char('v')).len(), 1);
}

#[test]
fn read_ahead_failure_retries_and_old_account_answers_are_ignored() {
    let mut app = with_home(&["s"]);
    let now = Instant::now();
    app.poll_read_ahead(now);
    let jobs = app.poll_read_ahead(now + Duration::from_millis(400));
    app.handle_answer(
        jobs[0].0,
        Event::Thread {
            id: "s".into(),
            result: Err(Error::api("gone")),
        },
    );
    assert!(app.message.is_none());
    assert_eq!(press(&mut app, KeyCode::Char('v')).len(), 1);
    app.switched(me());
    app.handle_answer(
        jobs[0].0,
        Event::Thread {
            id: "s".into(),
            result: Ok(Box::new(Thread::default())),
        },
    );
    assert!(app.cached_thread("s").is_none());
}

#[test]
fn notifications_count_new_ids_and_clear_when_viewed() {
    let mut app = with_home(&[]);
    let jobs = app.poll_read_ahead(Instant::now());
    let answer = |ids: &[&str]| Event::Notifications {
        max_id: None,
        result: Ok(Page {
            items: ids
                .iter()
                .map(|id| Notification {
                    id: (*id).into(),
                    ..Notification::default()
                })
                .collect(),
            next: Next::End,
        }),
    };
    app.handle_answer(jobs[0].0, answer(&["10", "9", "8"]));
    assert_eq!(app.unread, 3);
    assert!(press(&mut app, KeyCode::Char('4')).is_empty());
    assert_eq!(app.unread, 0);
    press(&mut app, KeyCode::Char('1'));
    let jobs = app.poll_read_ahead(Instant::now() + Duration::from_secs(61));
    app.handle_answer(jobs[0].0, answer(&["11", "10"]));
    assert_eq!(app.unread, 1);
    app.switched(me());
    assert_eq!(app.unread, 0);
}

fn profile_editor(app: &mut App) {
    app.tab = Tab::Profile;
    let account = Account {
        id: "7".into(),
        display_name: "Me".into(),
        source: Some(crate::api::types::AccountSource {
            note: "raw **bio**".into(),
            fields: vec![],
        }),
        ..Account::default()
    };
    app.profile.account = Some(account.clone());
    let jobs = press(app, KeyCode::Char('e'));
    app.handle_answer(jobs[0].0, Event::ProfileEditor(Ok(account)));
}

#[test]
fn missing_editable_profile_fields_closes_the_editor_and_reports_once() {
    let mut app = with_home(&["10"]);
    app.tab = Tab::Profile;
    let account = Account {
        id: "7".into(),
        ..Account::default()
    };
    app.profile.account = Some(account.clone());
    let jobs = press(&mut app, KeyCode::Char('e'));
    app.handle_answer(jobs[0].0, Event::ProfileEditor(Ok(account.clone())));
    assert!(app.overlay.is_none());
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .text
            .contains("editable profile fields")
    );
    assert!(app.message.as_ref().unwrap().text.contains(&app.origin));
    press(&mut app, KeyCode::Esc);
    app.handle_answer(jobs[0].0, Event::ProfileEditor(Ok(account)));
    assert!(app.message.is_none());
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.selected().unwrap().id, "10");
}

#[test]
fn profile_editor_sends_only_changes_on_save_and_keeps_a_refused_draft() {
    let mut app = with_home(&[]);
    assert!(press(&mut app, KeyCode::Char('e')).is_empty());
    profile_editor(&mut app);
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert!(app.overlay.is_none());
    profile_editor(&mut app);
    app.handle_key(ctrl('u'));
    typed(&mut app, "New name");
    let jobs = app.handle_key(ctrl('s'));
    let Job::SaveProfile(change) = &jobs[0].1 else {
        panic!()
    };
    assert_eq!(
        change.attributes,
        serde_json::json!({"display_name":"New name"})
    );
    assert!(change.pictures.is_empty());
    assert!(app.handle_key(ctrl('s')).is_empty());
    app.handle_answer(jobs[0].0, Event::ProfileSaved(Err(Error::api("refused"))));
    assert_eq!(app.profile.account.as_ref().unwrap().display_name, "Me");
    press(&mut app, KeyCode::Esc); // dismiss error
    assert!(
        matches!(&app.overlay,Some(Overlay::EditProfile(e)) if e.waiting.is_none() && e.fields[0].text() == "New name")
    );
    press(&mut app, KeyCode::Esc);
    assert!(app.overlay.is_none());
}

#[test]
fn profile_flags_raw_bio_fields_and_cancel_are_lossless() {
    let mut app = with_home(&[]);
    profile_editor(&mut app);
    press(&mut app, KeyCode::Tab);
    app.handle_paste(" + text");
    for _ in 0..3 {
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Char(' '));
    }
    press(&mut app, KeyCode::Tab);
    typed(&mut app, "Site");
    press(&mut app, KeyCode::Tab);
    typed(&mut app, "https://example.test");
    let jobs = app.handle_key(ctrl('s'));
    let Job::SaveProfile(change) = &jobs[0].1 else {
        panic!()
    };
    assert_eq!(change.attributes["note"], "raw **bio** + text");
    assert_eq!(change.attributes["locked"], true);
    assert_eq!(change.attributes["bot"], true);
    assert_eq!(change.attributes["discoverable"], true);
    assert_eq!(
        change.attributes["fields_attributes"]["0"],
        serde_json::json!({"name":"Site","value":"https://example.test"})
    );
    app.handle_answer(
        jobs[0].0,
        Event::ProfileSaved(Ok(Account {
            id: "7".into(),
            display_name: "Saved".into(),
            ..Account::default()
        })),
    );
    assert!(app.overlay.is_none());
    assert_eq!(app.profile.account.as_ref().unwrap().display_name, "Saved");
    profile_editor(&mut app);
    typed(&mut app, " unsaved");
    assert!(press(&mut app, KeyCode::Esc).is_empty());
    assert!(app.overlay.is_none());
}

#[test]
fn quotes_require_a_supported_server_and_confirmation_and_keep_refused_text() {
    let mut app = with_home(&["s"]);
    assert!(press(&mut app, KeyCode::Char('Q')).is_empty());
    assert!(app.overlay.is_none());
    assert!(
        app.message
            .as_ref()
            .unwrap()
            .text
            .contains("does not support quote posts")
    );
    press(&mut app, KeyCode::Char('x'));
    app.timelines[0].items[0].quote_approval = Some(crate::api::types::QuoteApproval {
        current_user: "automatic".into(),
    });
    press(&mut app, KeyCode::Char('Q'));
    typed(&mut app, "My quote");
    app.handle_key(ctrl('q'));
    app.handle_key(ctrl('q'));
    let jobs = app.handle_key(ctrl('s'));
    let Job::Post(d) = &jobs[0].1 else { panic!() };
    assert_eq!(d.quote.as_deref(), Some("s"));
    assert_eq!(d.quote_policy.as_deref(), Some("followers"));
    assert!(d.reply.is_none());
    app.handle_answer(jobs[0].0, Event::Posted(Err(Error::api("denied"))));
    assert!(
        matches!(&app.overlay,Some(Overlay::Compose(c)) if c.sending.is_none() && c.text.text() == "My quote")
    );
    press(&mut app, KeyCode::Esc);
    assert!(press(&mut app, KeyCode::Esc).is_empty());
    assert!(app.overlay.is_none());
    app.timelines[0].items[0].quote_approval = Some(crate::api::types::QuoteApproval {
        current_user: "denied".into(),
    });
    press(&mut app, KeyCode::Char('Q'));
    assert!(app.overlay.is_none());
}

#[test]
fn columns_keep_independent_pages_confirm_close_and_rollback_refused_favourites() {
    use crate::tui::columns::Source;
    use crate::tui::worker::ColumnPage;
    let mut app = with_home(&["home"]);
    let jobs = app.add_column(Source::Tag("rust".into()));
    let id = app.deck.columns[1].id;
    app.handle_answer(
        jobs[0].0,
        Event::Column {
            id,
            first: true,
            result: Ok(ColumnPage::Posts((
                vec![status("tag")],
                Some("next".into()),
            ))),
        },
    );
    assert_eq!(app.selected().unwrap().id, "tag");
    let page = press(&mut app, KeyCode::Char('j'));
    assert!(matches!(&page[0].1,Job::Column { next:Some(n),.. } if n == "next"));
    let write = press(&mut app, KeyCode::Char('l'));
    assert_eq!(app.selected().unwrap().favourited, Some(true));
    app.handle_answer(
        write[0].0,
        Event::Favourited {
            id: "tag".into(),
            on: true,
            result: Err(Error::api("refused")),
        },
    );
    assert_eq!(app.selected().unwrap().favourited, Some(false));
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.deck.columns.len(), 2);
    press(&mut app, KeyCode::Char('H'));
    assert_eq!(app.selected().unwrap().id, "home");
    press(&mut app, KeyCode::Char('L'));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.deck.columns.len(), 1);
    app.handle_answer(
        page[0].0,
        Event::Column {
            id,
            first: false,
            result: Ok(ColumnPage::Posts((vec![status("late")], None))),
        },
    );
    assert_eq!(app.selected().unwrap().id, "home");
}

#[test]
fn column_picker_queries_and_saved_sources_are_separated_by_account() {
    let mut app = with_home(&[]);
    app.apply_settings(Some(Settings::default()), ColorDepth::TrueColor);
    let jobs = press(&mut app, KeyCode::Char('+'));
    app.handle_answer(jobs[0].0, Event::ColumnChoices(Ok(vec![])));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    app.handle_paste("#rust");
    let jobs = press(&mut app, KeyCode::Enter);
    assert!(
        matches!(&jobs[0].1,Job::Column { source:crate::tui::columns::Source::Tag(tag),.. } if tag == "rust")
    );
    let settings = app.take_settings_save().unwrap();
    assert_eq!(settings.decks.len(), 1);
    let mut other = me();
    other.username = "other".into();
    other.user_id = "8".into();
    app.switched(other);
    assert!(app.deck.columns.is_empty());
    app.switched(me());
    assert_eq!(app.deck.columns.len(), 2);
}

#[test]
fn chat_marks_read_only_after_open_and_load_and_rolls_back_a_refusal() {
    use crate::api::types::Conversation;
    let mut app = with_home(&[]);
    let mut dm = status("dm");
    dm.visibility = "direct".into();
    let jobs = press(&mut app, KeyCode::Char('7'));
    assert!(matches!(jobs[0].1, Job::Conversations { .. }));
    app.handle_answer(
        jobs[0].0,
        Event::Conversations {
            first: true,
            result: Ok(Page {
                items: vec![Conversation {
                    id: "c".into(),
                    unread: true,
                    accounts: vec![account("alice")],
                    last_status: Some(dm.clone()),
                }],
                next: Next::End,
            }),
        },
    );
    assert!(app.chat.conversations.items[0].unread);
    let jobs = press(&mut app, KeyCode::Enter);
    assert!(matches!(jobs[0].1, Job::ChatThread { .. }));
    assert!(app.chat.conversations.items[0].unread);
    let mut other_branch = status("other-branch");
    other_branch.visibility = "direct".into();
    other_branch.account = account("bob");
    let jobs = app.handle_answer(
        jobs[0].0,
        Event::ChatThread(Ok(Box::new(Thread {
            status: dm,
            ancestors: vec![status("public")],
            descendants: vec![other_branch],
        }))),
    );
    assert_eq!(app.chat.open.as_ref().unwrap().posts.items.len(), 1);
    assert!(matches!(&jobs[0].1,Job::ReadConversation { id } if id == "c"));
    assert!(!app.chat.conversations.items[0].unread);
    app.handle_answer(
        jobs[0].0,
        Event::ConversationRead(Err(Error::api("refused"))),
    );
    assert!(app.chat.conversations.items[0].unread);
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('i'));
    typed(&mut app, "Hello");
    let jobs = press(&mut app, KeyCode::Enter);
    let Job::ChatPost(draft) = &jobs[0].1 else {
        panic!()
    };
    assert_eq!(draft.text, "@alice Hello");
    assert_eq!(draft.visibility, Visibility::Direct);
    assert_eq!(draft.reply.as_deref(), Some("dm"));
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    app.handle_answer(jobs[0].0, Event::ChatPosted(Err(Error::api("refused"))));
    let open = app.chat.open.as_ref().unwrap();
    assert_eq!(open.input.text(), "Hello");
    assert!(open.sending.is_none());
}

#[test]
fn messaging_a_profile_sends_nothing_until_enter_and_discards_old_account_answers() {
    let mut app = with_home(&[]);
    app.tab = Tab::Profile;
    app.profile.account = Some(account("alice"));
    assert!(press(&mut app, KeyCode::Char('m')).is_empty());
    assert_eq!(app.tab, Tab::Chat);
    app.handle_paste("Private hello");
    press(&mut app, KeyCode::Esc);
    assert_eq!(
        app.chat.open.as_ref().unwrap().input.text(),
        "Private hello"
    );
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    press(&mut app, KeyCode::Char('i'));
    let jobs = press(&mut app, KeyCode::Enter);
    let Job::ChatPost(d) = &jobs[0].1 else {
        panic!()
    };
    assert_eq!(d.text, "@alice Private hello");
    assert!(d.reply.is_none());
    app.switched(me());
    app.handle_answer(jobs[0].0, Event::ChatPosted(Ok(Box::new(status("late")))));
    assert!(app.chat.open.is_none());
}

#[test]
fn writing_while_an_adopted_thread_loads_reloads_after_the_write_finishes() {
    let mut app = with_home(&["s"]);
    let now = Instant::now();
    app.poll_read_ahead(now);
    let reads = app.poll_read_ahead(now + Duration::from_millis(400));
    press(&mut app, KeyCode::Char('v'));
    let writes = press(&mut app, KeyCode::Char('l'));
    app.handle_answer(
        reads[0].0,
        Event::Thread {
            id: "s".into(),
            result: Ok(Box::new(Thread {
                status: status("s"),
                ..Thread::default()
            })),
        },
    );
    assert!(!app.threads[0].rows.loaded);
    let reload = app.handle_answer(
        writes[0].0,
        Event::Favourited {
            id: "s".into(),
            on: true,
            result: Err(Error::api("refused")),
        },
    );
    assert!(matches!(&reload[0].1,Job::Thread { id } if id == "s"));
    assert_eq!(app.threads[0].rows.waiting, Some(reload[0].0));
}

#[test]
fn retry_notice_keeps_the_job_pending_and_ignores_completed_jobs() {
    let mut app = with_home(&["10"]);
    let jobs = press(&mut app, KeyCode::Char('R'));
    let seq = jobs[0].0;
    app.handle_answer(seq, Event::Retrying("server asks to wait".into()));
    assert!(app.waiting.contains(&seq));
    assert_eq!(app.message.as_ref().unwrap().text, "server asks to wait");
    app.handle_answer(seq, page(vec![status("11"), status("10")], Next::End));
    assert!(!app.waiting.contains(&seq));
    assert_eq!(app.timelines[0].items[0].id, "11");
    assert_eq!(app.selected().unwrap().id, "10");
    app.message = None;
    app.handle_answer(seq, Event::Retrying("stale notice".into()));
    assert!(app.message.is_none());
}

#[test]
fn login_cancel_logout_decline_and_account_removal_are_isolated() {
    let mut app = with_home(&["1"]);
    app.accounts = vec![me()];
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('a'));
    app.handle_paste("http://127.0.0.1:9");
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    assert_eq!(app.take_login().as_deref(), Some("http://127.0.0.1:9"));
    assert!(app.login_active());
    app.login_url("http://127.0.0.1:9/oauth/authorize".into());
    press(&mut app, KeyCode::Esc);
    assert!(!app.login_active());
    assert!(app.session.is_some());
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Char('n'));
    assert!(app.take_logout().is_none());
    assert_eq!(app.accounts.len(), 1);
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('x'));
    press(&mut app, KeyCode::Char('y'));
    let host = app.take_logout().unwrap();
    let old = app.job(Job::Identity).0;
    let jobs = app.logged_out(&host);
    assert!(app.session.is_none());
    assert!(app.accounts.is_empty());
    assert!(app.timelines[0].items.is_empty());
    assert!(jobs.iter().all(|(seq, _)| *seq > old));
    app.handle_answer(
        old,
        Event::Identity(Ok(Account {
            id: "stale".into(),
            ..Default::default()
        })),
    );
    assert!(app.session.is_none());
}

#[test]
fn login_refusal_keeps_server_for_retry_and_success_selects_new_account() {
    let mut app = with_home(&[]);
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('a'));
    app.handle_paste("http://127.0.0.1:9");
    press(&mut app, KeyCode::Enter);
    app.login_failed(&Error::api("access denied"));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Login { waiting: false, .. })
    ));
    assert!(!app.login_active());
    app.message = None;
    press(&mut app, KeyCode::Enter);
    assert!(app.login_active());
    app.logged_in(me());
    assert_eq!(app.take_switch(), Some(me()));
}

#[test]
fn settings_cancel_reset_and_save_affect_only_the_selected_row() {
    let mut app = with_home(&[]);
    app.apply_settings(Some(Settings::default()), ColorDepth::TrueColor);
    press(&mut app, KeyCode::Char('s'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    app.handle_paste("/tmp/mstdn-downloads");
    press(&mut app, KeyCode::Esc);
    assert!(app.take_settings_save().is_none());
    press(&mut app, KeyCode::Enter);
    app.handle_paste("/tmp/mstdn-downloads");
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        app.take_settings_save().unwrap().download_dir.as_deref(),
        Some("/tmp/mstdn-downloads")
    );
    press(&mut app, KeyCode::Char('j'));
    press(&mut app, KeyCode::Enter);
    app.handle_paste("firefox");
    press(&mut app, KeyCode::Enter);
    let saved = app.take_settings_save().unwrap();
    assert_eq!(saved.browser.as_deref(), Some("firefox"));
    assert!(saved.download_dir.is_some());
    press(&mut app, KeyCode::Char('x'));
    assert!(app.take_settings_save().unwrap().browser.is_none());
}

#[test]
fn server_markers_survive_restart_and_failed_read_writes_restore_unread_count() {
    let mut app = with_home(&[]);
    let now = Instant::now();
    let jobs = app.poll_read_ahead(now);
    let read = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::NotificationState))
        .unwrap()
        .0;
    app.handle_answer(
        read,
        Event::NotificationState(Ok((Some("8".into()), Some(42)))),
    );
    assert_eq!(app.unread, 42);
    let page = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::Notifications { .. }))
        .unwrap()
        .0;
    app.handle_answer(
        page,
        Event::Notifications {
            max_id: None,
            result: Ok(Page {
                items: vec![Notification {
                    id: "10".into(),
                    ..Default::default()
                }],
                next: Next::End,
            }),
        },
    );
    assert_eq!(app.unread, 42);
    press(&mut app, KeyCode::Char('4'));
    let jobs = app.poll_read_ahead(now);
    let mark = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::MarkNotifications { .. }))
        .unwrap()
        .0;
    assert_eq!(app.unread, 0);
    app.handle_answer(
        mark,
        Event::NotificationsMarked {
            id: "10".into(),
            result: Err(Error::api("refused")),
        },
    );
    assert_eq!(app.unread, 42);
    assert!(
        !app.poll_read_ahead(now)
            .iter()
            .any(|(_, j)| matches!(j, Job::MarkNotifications { .. }))
    );
    app.switched(me());
    app.handle_answer(
        mark,
        Event::NotificationsMarked {
            id: "10".into(),
            result: Ok(()),
        },
    );
    assert_eq!(app.unread, 0);
}

#[test]
fn successful_marker_save_clears_unread_and_old_sync_cannot_restore_it() {
    let mut app = with_home(&[]);
    let now = Instant::now();
    let jobs = app.poll_read_ahead(now);
    let read = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::NotificationState))
        .unwrap()
        .0;
    app.handle_answer(read, Event::NotificationState(Ok((None, Some(1)))));
    let page = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::Notifications { .. }))
        .unwrap()
        .0;
    app.handle_answer(
        page,
        Event::Notifications {
            max_id: None,
            result: Ok(Page {
                items: vec![Notification {
                    id: "10".into(),
                    ..Default::default()
                }],
                next: Next::End,
            }),
        },
    );
    press(&mut app, KeyCode::Char('4'));
    let jobs = app.poll_read_ahead(now);
    let mark = jobs
        .iter()
        .find(|(_, j)| matches!(j, Job::MarkNotifications { .. }))
        .unwrap()
        .0;
    app.handle_answer(
        mark,
        Event::NotificationsMarked {
            id: "10".into(),
            result: Ok(()),
        },
    );
    assert_eq!(app.unread, 0);
    app.handle_answer(read, Event::NotificationState(Ok((None, Some(1)))));
    assert_eq!(app.unread, 0);
}

#[test]
fn theme_page_keys_preview_and_escape_restores_the_original_theme() {
    let mut app = with_home(&[]);
    let original = app.theme_index;
    press(&mut app, KeyCode::Char('T'));
    press(&mut app, KeyCode::Home);
    press(&mut app, KeyCode::PageDown);
    assert_eq!(app.theme_index, PAGE_STEP);
    press(&mut app, KeyCode::PageUp);
    assert_eq!(app.theme_index, 0);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.theme_index, original);
}

#[test]
fn saved_browser_and_download_folder_are_carried_with_the_jobs() {
    let mut app = with_home(&["1"]);
    app.timelines[0].items[0].media_attachments = vec![Media::default()];
    app.timelines[0].items[0].url = Some("http://127.0.0.1:9/@alice/1".into());
    let mut settings = Settings::default();
    settings.browser = Some("test-browser".into());
    settings.download_dir = Some("test-downloads".into());
    app.apply_settings(Some(settings), ColorDepth::TrueColor);
    assert!(
        matches!(&press(&mut app, KeyCode::Char('o'))[0].1, Job::OpenLink { program: Some(program), .. } if program == "test-browser")
    );
    assert!(
        matches!(&press(&mut app, KeyCode::Char('d'))[0].1, Job::Download { directory: Some(directory), .. } if directory == "test-downloads")
    );
}
