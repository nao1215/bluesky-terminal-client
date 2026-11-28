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
            url: "https://m.test/@alice/1".into()
        }
    );
}

/// The keys the random sequences are made of.
fn any_key() -> impl Strategy<Value = KeyEvent> {
    prop_oneof![
        prop::sample::select(vec![
            'j', 'k', 'g', 'G', 'v', 'w', 'l', 'b', 'r', 'n', '.', 'T', '?', 'R', '1', '2', '3',
            '4', 'x', '👍',
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
