use super::*;
use serde_json::{Value, json};

const ORIGIN: &str = "https://misskey.test";

fn session() -> Session {
    Session {
        service: ORIGIN.into(),
        user_id: "u-me".into(),
        username: "me".into(),
        token: "fake-token-for-tests".into(),
        permissions: Vec::new(),
    }
}

fn note(v: Value) -> Note {
    serde_json::from_value(v).unwrap()
}

fn notes(ids: &[&str]) -> Vec<Note> {
    ids.iter()
        .map(|id| {
            note(json!({"id": id, "text": format!("note {id}"), "user": {"username": "alice"}}))
        })
        .collect()
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn code(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

/// The number of the job `want` among `jobs`.
fn seq_of(jobs: &Jobs, want: &Job) -> u64 {
    jobs.iter()
        .find(|(_, j)| j == want)
        .map(|(s, _)| *s)
        .unwrap_or_else(|| panic!("{want:?} not in {jobs:?}"))
}

fn home(until: Option<&str>) -> Job {
    Job::Timeline {
        kind: TimelineKind::Home,
        until: until.map(str::to_string),
    }
}

/// A logged-in app with the home timeline loaded with `ids`.
fn logged_in(ids: &[&str]) -> App {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    let seq = seq_of(&jobs, &home(None));
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(ids)),
        },
    );
    app
}

/// The timeline of the tab shown, else Home's.
fn timeline(app: &App) -> &List<Note> {
    &app.timelines[app.kind().unwrap_or(TimelineKind::Home).index()]
}

// The client opens on the Home tab, as Misskey's does, and asks for the
// notifications in the background.
#[test]
fn a_logged_in_start_opens_on_home_and_reads_the_notifications_too() {
    let (app, jobs) = App::new(Some(session()), ORIGIN);
    assert_eq!(app.tab, Tab::Timeline(TimelineKind::Home));
    assert_eq!(app.tabs, DEFAULT_TABS);
    let asked: Vec<&Job> = jobs.iter().map(|(_, j)| j).collect();
    assert_eq!(
        asked,
        [
            &home(None),
            &Job::Notifications { until: None },
            &Job::Unread
        ]
    );
    assert_eq!(app.pending, 2, "the unread count is not waited for");
    assert!(timeline(&app).loading);
}

// Without an account Home says how to log in and asks nothing; the Local
// tab (2) is read as a visitor; Notifications and Profile say how to log
// in too.
#[test]
fn a_visitor_is_told_how_to_log_in_and_reads_local_and_global() {
    let (mut app, jobs) = App::new(None, ORIGIN);
    assert!(jobs.is_empty(), "{jobs:?}");
    assert_eq!(app.kind(), Some(TimelineKind::Home));
    let why = timeline(&app).error.clone().unwrap();
    assert!(
        why.contains("run `msky login --host misskey.test`"),
        "{why}"
    );
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("Local and Global tabs")
    );
    let jobs = app.handle_key(key('2'));
    assert_eq!(app.kind(), Some(TimelineKind::Local));
    assert_eq!(
        jobs.iter().map(|(_, j)| j.clone()).collect::<Vec<_>>(),
        [Job::Timeline {
            kind: TimelineKind::Local,
            until: None
        }]
    );
    let jobs = app.handle_key(key('3'));
    assert_eq!(app.kind(), Some(TimelineKind::Global));
    assert_eq!(jobs.len(), 1);
    assert!(app.handle_key(key('1')).is_empty());
    assert_eq!(app.handle_key(key('R')), []);
    assert!(app.handle_key(key('4')).is_empty());
    assert!(
        app.notifications
            .error
            .as_ref()
            .unwrap()
            .contains("msky login")
    );
    assert!(app.handle_key(key('6')).is_empty());
    assert!(app.profile.error.as_ref().unwrap().contains("log in"));
}

// Each timeline is a tab that keeps its notes, its place and its paging:
// going to another and back asks for nothing again, R reloads the tab
// shown only, and tab and shift+tab go round the tabs in their order.
#[test]
fn each_timeline_tab_keeps_its_place_and_tab_goes_round() {
    let mut app = logged_in(&["c", "b", "a"]);
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('2'));
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Local,
            until: None
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Timeline {
            kind: TimelineKind::Local,
            until: None,
            result: Ok(notes(&["x"])),
        },
    );
    assert_eq!(timeline(&app).items.len(), 1);
    assert!(app.handle_key(key('1')).is_empty());
    assert_eq!(timeline(&app).selected, 1);
    assert!(app.handle_key(key('2')).is_empty(), "loaded once");
    let jobs = app.handle_key(key('R'));
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Local,
            until: None
        }
    );
    assert!(
        app.timelines[TimelineKind::Home.index()].loaded,
        "Home is left as it is"
    );
    // Round the tabs, both ways, wrapping.
    app.handle_key(key('1'));
    let order: Vec<Tab> = (0..DEFAULT_TABS.len())
        .map(|_| {
            app.handle_key(code(KeyCode::Tab));
            app.tab
        })
        .collect();
    assert_eq!(
        order,
        DEFAULT_TABS[1..]
            .iter()
            .chain(&DEFAULT_TABS[..1])
            .copied()
            .collect::<Vec<_>>()
    );
    app.handle_key(key('1'));
    app.handle_key(code(KeyCode::BackTab));
    assert_eq!(app.tab, Tab::Chat);
    // ← → H L switch nothing.
    app.handle_key(key('1'));
    for k in [
        code(KeyCode::Right),
        code(KeyCode::Left),
        key('H'),
        key('L'),
    ] {
        app.handle_key(k);
        assert_eq!(app.tab, Tab::Timeline(TimelineKind::Home));
        assert_eq!(app.overlay, None);
    }
}

// The tabs setting chooses the tabs and their order, social among them;
// what is not a tab is said, and none at all is the default.
#[test]
fn the_tabs_setting_orders_the_tabs_and_numbers_follow_it() {
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut app = logged_in(&["a"]);
    let mut tabs_of = |n: &[String]| app.tabs_for_host(n);
    let (tabs, warn) = tabs_of(&names(&[
        "Social",
        "notifications",
        "home",
        "home",
        "feeds",
    ]));
    assert_eq!(
        tabs,
        [
            Tab::Timeline(TimelineKind::Social),
            Tab::Notifications,
            Tab::Timeline(TimelineKind::Home)
        ]
    );
    assert!(warn.unwrap().contains("feeds is not a tab"));
    assert_eq!(tabs_of(&[]).0, DEFAULT_TABS);
    assert_eq!(tabs_of(&names(&["nope"])).0, DEFAULT_TABS);
    let jobs = app.set_tabs(tabs);
    assert_eq!(app.tab, Tab::Timeline(TimelineKind::Social));
    assert_eq!(
        jobs[0].1,
        Job::Timeline {
            kind: TimelineKind::Social,
            until: None
        }
    );
    app.handle_key(key('2'));
    assert_eq!(app.tab, Tab::Notifications);
    assert!(app.handle_key(key('5')).is_empty(), "no fifth tab");
    assert_eq!(app.tab, Tab::Notifications);
    app.handle_key(key('/'));
    assert_eq!(app.tab, Tab::Notifications, "no Search tab to go to");
}

// Near the end the next page is asked for once, older than the last note;
// a page repeating notes adds only the new ones, and one with nothing new
// ends the list.
#[test]
fn pages_continue_from_the_last_note_and_end_when_nothing_is_new() {
    let ids: Vec<String> = (0..10).map(|i| format!("n{:02}", 99 - i)).collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut app = logged_in(&ids);
    let jobs = app.handle_key(key('j'));
    assert_eq!(jobs.len(), 1);
    let (seq, job) = jobs[0].clone();
    assert_eq!(job, home(Some("n90")));
    assert!(app.handle_key(key('j')).is_empty(), "asked once");
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: Some("n90".into()),
            result: Ok(notes(&["n90", "n89"])),
        },
    );
    assert_eq!(timeline(&app).items.len(), 11);
    assert_eq!(timeline(&app).cursor.as_deref(), Some("n89"));
    let jobs = app.handle_key(key('G'));
    let (seq, job) = jobs[0].clone();
    assert_eq!(job, home(Some("n89")));
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: Some("n89".into()),
            result: Ok(notes(&["n89"])),
        },
    );
    assert_eq!(timeline(&app).cursor, None);
    assert!(app.handle_key(key('k')).is_empty());
    assert_eq!(app.pending, 1, "the notifications are still out");
}

// A failed next page is asked for again on the next move, and says why.
#[test]
fn a_failed_next_page_is_asked_again() {
    let ids: Vec<String> = (0..5).map(|i| format!("n{i}")).collect();
    let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut app = logged_in(&ids);
    let (seq, _) = app.handle_key(key('j'))[0].clone();
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: Some("n4".into()),
            result: Err(Error::api("boom").with_hint("try again")),
        },
    );
    assert_eq!(app.status.as_ref().unwrap().text, "boom: try again");
    assert!(app.status.as_ref().unwrap().error);
    assert_eq!(app.handle_key(key('j')).len(), 1);
}

// R asks for the first page again; the answer to the earlier one, coming
// late, is not applied over it.
#[test]
fn a_first_page_answered_after_a_newer_one_was_asked_is_dropped() {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    let old = seq_of(&jobs, &home(None));
    let new = app.handle_key(key('R'))[0].0;
    app.handle_answer(
        new,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(&["new"])),
        },
    );
    app.handle_answer(
        old,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(&["old"])),
        },
    );
    assert_eq!(timeline(&app).items[0].id, "new");
    // R keeps the selection on the note it was on.
    let mut app = logged_in(&["c", "b", "a"]);
    app.handle_key(key('j'));
    let seq = app.handle_key(key('R'))[0].0;
    app.handle_answer(
        seq,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(&["d", "c", "b", "a"])),
        },
    );
    assert_eq!(timeline(&app).current().unwrap().id, "b");
}

fn renote_of(inner: Value) -> Note {
    note(
        json!({"id": "rn", "user": {"username": "carol"}, "renoteId": inner["id"], "renote": inner}),
    )
}

// v opens the thread of the note shown, the renoted one for a renote; the
// selection starts on it, under what it answers; Esc closes it.
#[test]
fn v_opens_the_thread_of_the_note_shown_with_the_selection_on_it() {
    let mut app = logged_in(&[]);
    app.timelines[0].set(vec![renote_of(
        json!({"id": "orig", "user": {"username": "bob"}}),
    )]);
    let jobs = app.handle_key(key('v'));
    let (seq, job) = jobs[0].clone();
    assert_eq!(job, Job::Thread { id: "orig".into() });
    assert_eq!(app.threads.len(), 1);
    let t = Thread {
        ancestors: notes(&["root", "parent"]),
        note: note(json!({"id": "orig"})),
        replies: vec![(note(json!({"id": "r1"})), notes(&["r1a"]))],
    };
    app.handle_answer(
        seq,
        Event::Thread {
            id: "orig".into(),
            result: Ok(Box::new(t)),
        },
    );
    let th = app.threads.last().unwrap();
    let rows: Vec<(&str, u16)> = th
        .list
        .items
        .iter()
        .map(|r| (r.note.id.as_str(), r.depth))
        .collect();
    assert_eq!(
        rows,
        [
            ("root", 0),
            ("parent", 0),
            ("orig", 0),
            ("r1", 1),
            ("r1a", 2)
        ]
    );
    assert_eq!(th.list.current().unwrap().note.id, "orig");
    assert!(
        app.handle_key(key('j')).is_empty(),
        "a thread has no next page"
    );
    let jobs = app.handle_key(key('v'));
    assert_eq!(jobs[0].1, Job::Thread { id: "r1".into() });
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.threads.is_empty());
    // A thread that cannot be read says why, in the thread.
    let (seq, _) = app.handle_key(key('v'))[0].clone();
    app.handle_answer(
        seq,
        Event::Thread {
            id: "orig".into(),
            result: Err(Error::api("notes/show failed: NO_SUCH_NOTE")),
        },
    );
    let th = app.threads.last().unwrap();
    assert!(th.list.loaded);
    assert_eq!(
        th.list.error.as_deref(),
        Some("notes/show failed: NO_SUCH_NOTE")
    );
}

// w opens the text behind a warning, and closes it again; on a note
// without one it does nothing.
#[test]
fn w_shows_and_hides_the_text_behind_a_content_warning() {
    let mut app = logged_in(&[]);
    app.timelines[0].set(vec![
        note(json!({"id": "w1", "cw": "ネタバレ", "text": "結末"})),
        note(json!({"id": "plain", "text": "hi"})),
    ]);
    let n = app.timelines[0].items[0].clone();
    assert!(!app.text_shown(&n));
    app.handle_key(key('w'));
    assert!(app.text_shown(&n));
    app.handle_key(key('w'));
    assert!(!app.text_shown(&n));
    app.handle_key(key('j'));
    app.handle_key(key('w'));
    assert!(app.opened.is_empty());
}

fn with_files() -> Note {
    note(json!({
        "id": "f", "text": "見て",
        "files": [
            {"id": "a", "type": "image/png", "url": "https://cdn.test/a.png", "isSensitive": true},
            {"id": "b", "type": "video/mp4", "url": "https://cdn.test/b.mp4"}
        ],
        "renoteId": "q", "renote": {"id": "q", "files": [{"id": "c", "type": "image/jpeg", "url": "https://cdn.test/c.jpg"}]}
    }))
}

// space shows the note's files, its quote's too; ← → step through them,
// and d saves the one shown; d on the list saves them all, sensitive ones
// included, as `msky download` does.
#[test]
fn the_viewer_shows_every_file_and_d_saves_them() {
    let mut app = logged_in(&[]);
    app.timelines[0].set(vec![with_files()]);
    assert!(app.handle_key(key(' ')).is_empty());
    let Some(Overlay::Viewer {
        files, index: 0, ..
    }) = &app.overlay
    else {
        panic!("{:?}", app.overlay)
    };
    let named: Vec<(&str, usize, &str)> = files
        .iter()
        .map(|(n, i, f)| (n.as_str(), *i, f.id.as_str()))
        .collect();
    assert_eq!(named, [("f", 1, "a"), ("f", 2, "b"), ("q", 1, "c")]);
    for _ in 0..5 {
        app.handle_key(code(KeyCode::Right));
    }
    let jobs = app.handle_key(key('d'));
    let Job::Download { files, .. } = &jobs[0].1 else {
        panic!("{jobs:?}")
    };
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].2.id, "c");
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.overlay, None);
    let jobs = app.handle_key(key('d'));
    let Job::Download { files, .. } = &jobs[0].1 else {
        panic!("{jobs:?}")
    };
    assert_eq!(files.len(), 3);
    app.handle_answer(
        jobs[0].0,
        Event::Downloaded(Ok(vec!["/dl/f-1.png".into(), "/dl/f-2.mp4".into()])),
    );
    assert_eq!(app.status.as_ref().unwrap().text, "saved 2 files in /dl");
    // A note without files says so, and opens nothing.
    app.timelines[0].set(notes(&["x"]));
    assert!(app.handle_key(key(' ')).is_empty());
    assert_eq!(app.overlay, None);
    assert_eq!(app.status.as_ref().unwrap().text, "the note has no files");
}

// Without pictures space opens the note in the browser, as bsky opens a
// post on bsky.app; o does too, and c copies its address.
#[test]
fn without_pictures_space_opens_the_note_in_the_browser() {
    let mut app = logged_in(&[]);
    app.pictures = false;
    app.timelines[0].set(vec![renote_of(json!({"id": "orig"}))]);
    let url = format!("{ORIGIN}/notes/orig");
    let jobs = app.handle_key(key(' '));
    assert_eq!(
        jobs[0].1,
        Job::OpenLink {
            url: url.clone(),
            browser: None
        }
    );
    assert_eq!(
        app.handle_key(key('o'))[0].1,
        Job::OpenLink {
            url: url.clone(),
            browser: None
        }
    );
    app.handle_key(key('c'));
    assert_eq!(app.take_copy(), Some(url));
    assert_eq!(app.take_copy(), None);
}

// . lists what the keys do to the note, and Enter does the one chosen.
#[test]
fn the_actions_list_names_the_keys_and_runs_the_one_chosen() {
    let mut app = logged_in(&[]);
    app.timelines[0].set(vec![with_files()]);
    app.handle_key(key('.'));
    let names: Vec<&str> = keys::actions(&app).iter().map(|(k, _)| *k).collect();
    assert_eq!(
        names,
        ["r", "l", "e", "b", "Q", "space", "v", "d", "o", "c", "!"]
    );
    for _ in 0..6 {
        app.handle_key(key('j'));
    }
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(jobs[0].1, Job::Thread { id: "f".into() });
    assert_eq!(app.overlay, None);
}

// ? opens the help; a stray key does not close it, ? does.
#[test]
fn the_help_stays_open_until_closed() {
    let mut app = logged_in(&["a"]);
    app.handle_key(key('?'));
    app.handle_key(key('x'));
    app.handle_key(key('j'));
    assert_eq!(app.overlay, Some(Overlay::Help { scroll: 1 }));
    app.handle_key(key('?'));
    assert_eq!(app.overlay, None);
    app.handle_key(key('q'));
    assert!(app.quit);
}

// Answers to what was asked as the account before a switch are dropped.
#[test]
fn answers_for_the_account_before_a_switch_are_dropped() {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    let old = seq_of(&jobs, &home(None));
    let other = Session {
        service: "https://other.test".into(),
        username: "bob".into(),
        ..session()
    };
    let jobs = app.switched_to(Some(other), "https://other.test");
    assert_eq!(app.host(), "other.test");
    app.handle_answer(
        old,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(&["of-me"])),
        },
    );
    assert!(!timeline(&app).loaded);
    let new = seq_of(&jobs, &home(None));
    app.handle_answer(
        new,
        Event::Timeline {
            kind: TimelineKind::Home,
            until: None,
            result: Ok(notes(&["of-bob"])),
        },
    );
    assert_eq!(timeline(&app).items[0].id, "of-bob");
    assert_eq!(app.status.as_ref().unwrap().text, "now @bob@other.test");
}

// x asks before logging an account out; any key but y keeps it.
#[test]
fn logging_out_from_the_account_list_takes_a_y() {
    let mut app = logged_in(&["a"]);
    app.accounts = vec![Account {
        host: "misskey.test".into(),
        username: "me".into(),
    }];
    app.handle_key(key('A'));
    app.handle_key(key('x'));
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .starts_with("press y to log @me@misskey.test out")
    );
    app.handle_key(key('n'));
    assert_eq!(app.take_account_logout(), None);
    app.handle_key(key('x'));
    app.handle_key(key('y'));
    assert_eq!(app.take_account_logout().as_deref(), Some("misskey.test"));
    // Logging out the one in use reads the server as a visitor.
    app.logged_out("misskey.test", None);
    assert!(app.session.is_none());
    assert!(app.accounts.is_empty());
    assert!(timeline(&app).error.is_some());
}

// The notifications page as a timeline does; v and space act on the note
// a notification is about.
#[test]
fn notifications_page_and_act_on_their_note() {
    let (mut app, jobs) = App::new(Some(session()), ORIGIN);
    let seq = seq_of(&jobs, &Job::Notifications { until: None });
    let items: Vec<Notification> = (0..3)
        .map(|i| {
            serde_json::from_value(json!({
                "id": format!("n{i}"), "type": "reply",
                "note": {"id": format!("note{i}"), "files": [{"id": "f", "type": "image/png", "url": "https://cdn.test/f.png"}]}
            }))
            .unwrap()
        })
        .collect();
    app.handle_answer(
        seq,
        Event::Notifications {
            until: None,
            result: Ok(items),
        },
    );
    app.handle_key(key('4'));
    assert_eq!(app.tab, Tab::Notifications);
    let jobs = app.handle_key(key('j'));
    assert_eq!(
        jobs[0].1,
        Job::Notifications {
            until: Some("n2".into())
        }
    );
    let jobs = app.handle_key(key('v'));
    assert_eq!(jobs[0].1, Job::Thread { id: "note1".into() });
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key(' '));
    assert!(app.viewer_open());
}

/// A random walk over every key and every kind of answer: whatever comes
/// in whatever order, nothing panics, the selection stays in its list, and
/// once every job is answered nothing says it still waits.
#[test]
fn random_keys_and_answers_leave_the_state_consistent() {
    let keys = [
        key('j'),
        key('k'),
        key('g'),
        key('G'),
        key('v'),
        key(' '),
        key('w'),
        key('d'),
        key('o'),
        key('c'),
        key('.'),
        key('?'),
        key('+'),
        key('A'),
        key('x'),
        key('y'),
        key('1'),
        key('4'),
        key('R'),
        key('L'),
        key('H'),
        key('a'),
        key('/'),
        code(KeyCode::Esc),
        code(KeyCode::Enter),
        code(KeyCode::Tab),
        code(KeyCode::BackTab),
        code(KeyCode::PageDown),
        code(KeyCode::Right),
        code(KeyCode::Left),
        key('n'),
        key('r'),
        key('Q'),
        key('l'),
        key('e'),
        key('b'),
        key('D'),
        key('3'),
        key('5'),
        key('6'),
        key('2'),
        key('M'),
        key('B'),
        key('7'),
        key('i'),
        key('s'),
        key('T'),
        key('+'),
        key('!'),
        key('x'),
        key('y'),
        key('e'),
        code(KeyCode::Tab),
        KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL),
        key('t'),
        key('i'),
        key('f'),
        key('x'),
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
    ];
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut rand = move |n: usize| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n as u64) as usize
    };
    for run in 0..200 {
        let (mut app, jobs) = if run % 3 == 0 {
            App::new(None, ORIGIN)
        } else {
            App::new(Some(session()), ORIGIN)
        };
        app.accounts = vec![Account {
            host: "misskey.test".into(),
            username: "me".into(),
        }];
        let mut out: Vec<(u64, Job)> = jobs;
        for step in 0..120 {
            if rand(3) == 0 && !out.is_empty() {
                let (seq, job) = out.remove(rand(out.len()));
                let fail = rand(4) == 0;
                let size = rand(25);
                let page = |p: &str| -> Vec<Note> {
                    (0..size).map(|i| note(json!({"id": format!("{p}{step}-{i}"), "cw": if i % 3 == 0 { json!("cw") } else { Value::Null }, "files": [{"id":"f","type":"image/png"}]}))).collect()
                };
                let ev = match job {
                    Job::Timeline { kind, until } => Event::Timeline {
                        kind,
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(page("t"))
                        },
                    },
                    Job::Notifications { until } => Event::Notifications {
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(page("n")
                                .into_iter()
                                .map(|n| Notification {
                                    id: n.id.clone(),
                                    note: Some(n),
                                    ..Default::default()
                                })
                                .collect())
                        },
                    },
                    Job::Thread { id } => Event::Thread {
                        id,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(Box::new(Thread {
                                note: note(json!({"id": "t"})),
                                ancestors: page("a"),
                                replies: vec![],
                            }))
                        },
                    },
                    Job::Download { .. } => Event::Downloaded(Err(Error::io("x"))),
                    Job::OpenLink { .. } => Event::Opened(Ok(())),
                    Job::Servers => Event::Servers {
                        list: crate::servers::builtin(),
                        failed: None,
                    },
                    Job::Post { .. } => Event::Posted(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok(Box::new(note(json!({"id": format!("new{step}")}))))
                    }),
                    Job::React { id, reaction, had } => Event::Reacted {
                        id,
                        reaction,
                        had,
                        result: if fail { Err(Error::api("x")) } else { Ok(()) },
                    },
                    Job::Renote { id, on } => Event::Renoted {
                        id,
                        on,
                        result: if fail { Err(Error::api("x")) } else { Ok(()) },
                    },
                    Job::Delete { id } => Event::Deleted {
                        id,
                        result: if fail { Err(Error::api("x")) } else { Ok(()) },
                    },
                    Job::MarkRead => Event::MarkedRead(Ok(())),
                    Job::FeedNotes { feed, until } => Event::FeedNotes {
                        feed,
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(page("f"))
                        },
                    },
                    Job::ReadAhead { id } => Event::ReadAhead {
                        id,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(Box::default())
                        },
                    },
                    Job::Unread => Event::Unread(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok((Some(size as u64), size > 0))
                    }),
                    Job::Report { .. } => {
                        Event::Reported(if fail { Err(Error::api("x")) } else { Ok(()) })
                    }
                    Job::FeedChoices => Event::FeedChoices(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok((0..size)
                            .map(|i| Feed {
                                kind: FeedKind::ALL[i % 3],
                                host: "misskey.test".into(),
                                id: format!("f{step}-{i}"),
                                name: "名".into(),
                            })
                            .collect())
                    }),
                    Job::LoadProfileEditor => Event::ProfileEditor(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok(Box::new(
                            serde_json::from_value(
                                json!({"id": "u-me", "username": "me", "name": "名前"}),
                            )
                            .unwrap(),
                        ))
                    }),
                    Job::SaveProfile(_) => Event::ProfileSaved(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok(Box::new(
                            serde_json::from_value(
                                json!({"id": "u-me", "username": "me", "name": "新"}),
                            )
                            .unwrap(),
                        ))
                    }),
                    Job::ChatHistory => Event::ChatHistory(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok((0..size)
                            .map(|i| serde_json::from_value(json!({"id": format!("c{step}-{i}"), "fromUserId": format!("u{i}"), "fromUser": {"username": "x"}, "text": "hi"})).unwrap())
                            .collect())
                    }),
                    Job::ChatMessages { with, until } => Event::ChatMessages {
                        with,
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok((0..size)
                                .map(|i| serde_json::from_value(json!({"id": format!("m{step}-{i}"), "fromUserId": "u", "text": "hi"})).unwrap())
                                .collect())
                        },
                    },
                    Job::ChatSend { with, text } => Event::ChatSent {
                        with,
                        text,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(Box::new(serde_json::from_value(json!({"id": format!("s{step}"), "fromUserId": "u-me", "text": "sent"})).unwrap()))
                        },
                    },
                    Job::Emojis => Event::Emojis(Ok(vec![(
                        "blobcat".into(),
                        "http://127.0.0.1:9/b.png".into(),
                    )])),
                    Job::Profile { .. } => Event::Profile(if fail {
                        Err(Error::api("x"))
                    } else {
                        Ok(Box::new((
                            serde_json::from_value(
                                json!({"id": format!("u{step}"), "username": "u"}),
                            )
                            .unwrap(),
                            page("p"),
                        )))
                    }),
                    Job::UserNotes { user_id, until } => Event::UserNotes {
                        user_id,
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(page("q"))
                        },
                    },
                    Job::SearchNotes { query, until } => Event::SearchNotes {
                        query,
                        until,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(page("s"))
                        },
                    },
                    Job::SearchUsers { query, offset } => Event::SearchUsers {
                        query,
                        offset,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok((0..size)
                                .map(|i| {
                                    serde_json::from_value(
                                        json!({"id": format!("su{step}-{i}"), "username": "s"}),
                                    )
                                    .unwrap()
                                })
                                .collect())
                        },
                    },
                    Job::Follow { user_id, .. } | Job::Guard { user_id, .. } => {
                        Event::Followed {
                            user_id: user_id.clone(),
                            result: if fail {
                                Err(Error::api("x"))
                            } else {
                                Ok(Box::new(serde_json::from_value(json!({"id": user_id, "username": "f", "isFollowing": true})).unwrap()))
                            },
                        }
                    }
                    Job::Login { session, .. } => Event::LoggedIn {
                        session,
                        result: if fail {
                            Err(Error::api("x"))
                        } else {
                            Ok(session_ok())
                        },
                    },
                };
                out.extend(app.handle_answer(seq, ev));
            } else {
                out.extend(app.handle_key(keys[rand(keys.len())]));
            }
            let lists = app
                .timelines
                .iter()
                .map(|l| (l.selected, l.items.len()))
                .chain([(app.notifications.selected, app.notifications.items.len())])
                .chain([(app.search.notes.selected, app.search.notes.items.len())])
                .chain([(app.search.users.selected, app.search.users.items.len())])
                .chain([(app.profile.notes.selected, app.profile.notes.items.len())])
                .chain(
                    app.feeds
                        .iter()
                        .map(|f| (f.list.selected, f.list.items.len())),
                )
                .chain(
                    app.threads
                        .iter()
                        .map(|t| (t.list.selected, t.list.items.len())),
                );
            for (sel, len) in lists {
                assert!(sel < len.max(1), "run {run} step {step}: {sel} of {len}");
            }
            // The unread count is asked in the background, not waited for.
            let waited = out.iter().filter(|(_, j)| *j != Job::Unread).count();
            assert_eq!(app.pending, waited, "run {run} step {step}");
            if out.is_empty() {
                assert!(
                    app.in_flight.is_empty(),
                    "run {run} step {step}: {:?}",
                    app.in_flight
                );
            }
            if app.quit {
                break;
            }
        }
    }
}

fn session_ok() -> Session {
    Session {
        service: "https://mi.test".into(),
        username: "new".into(),
        ..session()
    }
}

fn form(app: &App) -> &LoginForm {
    match &app.overlay {
        Some(Overlay::Login(f)) => f,
        other => panic!("{other:?}"),
    }
}

// a in the account list opens the login form with the servers msky knows,
// and asks for Misskey Hub's list, which takes their place, the selection
// staying on the server it was on.
#[test]
fn the_login_form_offers_servers_and_takes_the_hubs_list() {
    let mut app = logged_in(&["x"]);
    app.handle_key(key('A'));
    let jobs = app.handle_key(key('a'));
    assert_eq!(jobs[0].1, Job::Servers);
    assert_eq!(form(&app).servers, crate::servers::builtin());
    assert!(form(&app).loading);
    app.handle_key(key('j'));
    let hub: Vec<Server> = ["big.test", "takusuki.com", "misskey-square.net"]
        .into_iter()
        .map(|h| Server {
            host: h.into(),
            name: h.into(),
            users: Some(1),
            langs: vec![],
            open: None,
        })
        .collect();
    app.handle_answer(
        jobs[0].0,
        Event::Servers {
            list: hub,
            failed: None,
        },
    );
    let f = form(&app);
    assert!(!f.loading);
    assert_eq!(f.servers[f.selected].host, "misskey-square.net");
    // A list that cannot be read leaves the built-in one, and says why.
    let jobs = app.open_login();
    app.handle_answer(
        jobs[0].0,
        Event::Servers {
            list: crate::servers::builtin(),
            failed: Some(Error::api("cannot read the server list at X: HTTP 503")),
        },
    );
    assert!(form(&app).error.as_ref().unwrap().contains("HTTP 503"));
}

// Enter asks for access on the server chosen: the page that allows it is
// opened, the wait for it is a job, and the login it makes is handed to
// the event loop; one that is not allowed says why, in the form.
#[test]
fn a_login_opens_the_page_waits_and_hands_the_login_over() {
    let (mut app, _) = App::new(None, ORIGIN);
    app.open_login();
    app.handle_key(key('j'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    let w = form(&app).waiting.clone().unwrap();
    assert_eq!(w.origin, "https://misskey-square.net");
    assert!(
        w.url.starts_with(&format!(
            "https://misskey-square.net/miauth/{}?name=msky&permission=read:account,",
            w.session
        )),
        "{}",
        w.url
    );
    assert_eq!(
        jobs[0].1,
        Job::OpenLink {
            url: w.url.clone(),
            browser: None
        }
    );
    assert_eq!(
        jobs[1].1,
        Job::Login {
            origin: w.origin.clone(),
            session: w.session.clone()
        }
    );
    // A browser that cannot be opened is no error: the address is shown.
    app.handle_answer(jobs[0].0, Event::Opened(Err(Error::io("no browser"))));
    assert!(app.status.as_ref().is_none_or(|s| !s.error));
    // An answer for another session is not this login's.
    app.handle_answer(
        99,
        Event::LoggedIn {
            session: "other".into(),
            result: Ok(session_ok()),
        },
    );
    assert_eq!(app.take_logged_in(), None);
    app.handle_answer(
        jobs[1].0,
        Event::LoggedIn {
            session: w.session.clone(),
            result: Err(Error::usage(
                "misskey-square.net did not give msky the access: ACCESS_DENIED",
            )),
        },
    );
    assert!(form(&app).waiting.is_none());
    assert!(form(&app).error.as_ref().unwrap().contains("ACCESS_DENIED"));
    // A host typed after / is a server not listed.
    app.handle_key(key('/'));
    for c in "Mi.Test".chars() {
        app.handle_key(key(c));
    }
    let jobs = app.handle_key(code(KeyCode::Enter));
    let w = form(&app).waiting.clone().unwrap();
    assert_eq!(w.origin, "https://mi.test");
    app.handle_answer(
        jobs[1].0,
        Event::LoggedIn {
            session: w.session,
            result: Ok(session_ok()),
        },
    );
    assert_eq!(app.overlay, None);
    assert_eq!(app.take_logged_in().unwrap().username, "new");
}

// esc while waiting calls the login off; a host that is not one stays in
// the form with why.
#[test]
fn a_login_can_be_called_off_and_a_bad_host_is_refused() {
    let (mut app, _) = App::new(None, ORIGIN);
    app.open_login();
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(code(KeyCode::Esc));
    assert!(form(&app).waiting.is_none());
    assert_eq!(
        form(&app).error.as_deref(),
        Some("the login was called off")
    );
    app.handle_key(key('/'));
    for c in "a/b".chars() {
        app.handle_key(key(c));
    }
    assert!(app.handle_key(code(KeyCode::Enter)).is_empty());
    assert!(
        form(&app)
            .error
            .as_ref()
            .unwrap()
            .starts_with("invalid host")
    );
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.overlay, None);
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        app.handle_key(key(c));
    }
}

fn compose(app: &App) -> &Compose {
    match &app.overlay {
        Some(Overlay::Compose(c)) => c,
        other => panic!("{other:?}"),
    }
}

/// A logged-in app with the permissions `msky login` asks for now.
fn writer(notes: Vec<Note>) -> App {
    let mut app = logged_in(&[]);
    app.session.as_mut().unwrap().permissions = crate::miauth::PERMISSIONS
        .iter()
        .map(|p| p.to_string())
        .collect();
    app.timelines[0].set(notes);
    app
}

// n opens the composer; the text, a content warning (ctrl+w), the
// visibility (ctrl+p) and the files go to the post job, sent once; the
// note made goes on top of home, and the composer closes.
#[test]
fn n_composes_and_ctrl_s_posts_once() {
    let mut app = writer(notes(&["a"]));
    app.handle_key(key('n'));
    type_str(&mut app, "こんにちは 👨‍👩‍👧 🇯🇵");
    app.handle_key(ctrl('w'));
    assert!(compose(&app).cw_focused());
    type_str(&mut app, "ネタバレ");
    app.handle_key(ctrl('p'));
    app.handle_key(ctrl('p'));
    let jobs = app.handle_key(ctrl('s'));
    assert_eq!(
        jobs[0].1,
        Job::Post {
            text: "こんにちは 👨‍👩‍👧 🇯🇵".into(),
            cw: Some("ネタバレ".into()),
            visibility: crate::cli::Visibility::Followers,
            reply: None,
            renote: None,
            files: vec![],
        }
    );
    assert!(compose(&app).sending);
    assert!(app.handle_key(ctrl('s')).is_empty(), "sent once");
    app.handle_answer(
        jobs[0].0,
        Event::Posted(Ok(Box::new(note(
            json!({"id": "mine", "text": "こんにちは"}),
        )))),
    );
    assert_eq!(app.overlay, None);
    assert_eq!(timeline(&app).items[0].id, "mine");
    assert_eq!(
        timeline(&app).current().unwrap().id,
        "a",
        "the selection stays"
    );
}

// What can be told is wrong is said before anything is sent, and the
// composer stays with its text; a post the server refuses keeps it too.
#[test]
fn a_note_that_cannot_be_sent_says_why_and_keeps_the_text() {
    let mut app = writer(notes(&["a"]));
    app.handle_key(key('n'));
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert_eq!(app.status.as_ref().unwrap().text, "the note is empty");
    type_str(&mut app, "x");
    app.handle_key(ctrl('w'));
    assert!(app.handle_key(ctrl('s')).is_empty());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .starts_with("the content warning is empty")
    );
    app.handle_key(ctrl('w'));
    let jobs = app.handle_key(ctrl('s'));
    app.handle_answer(
        jobs[0].0,
        Event::Posted(Err(Error::usage(
            "the note is 3001 characters; misskey.test takes 3000",
        ))),
    );
    assert!(!compose(&app).sending);
    assert_eq!(compose(&app).input.text(), "x");
    assert!(app.status.as_ref().unwrap().error);
}

// r replies to the note shown, Q quotes it, each named in the composer;
// a reply to a home note is home too.
#[test]
fn r_replies_and_q_quotes_the_note_shown() {
    let mut app = writer(vec![renote_of(
        json!({"id": "orig", "user": {"username": "bob", "host": "x.test"}, "text": "$[x2 元の]ノート\n二行目", "visibility": "home"}),
    )]);
    app.handle_key(key('r'));
    let c = compose(&app);
    assert_eq!(c.reply.as_ref().unwrap().id, "orig");
    assert_eq!(c.reply.as_ref().unwrap().who, "@bob@x.test");
    assert_eq!(c.reply.as_ref().unwrap().excerpt, "元のノート");
    assert_eq!(c.visibility, crate::cli::Visibility::Home);
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('Q'));
    type_str(&mut app, "見て");
    let jobs = app.handle_key(ctrl('s'));
    let Job::Post { renote, reply, .. } = &jobs[0].1 else {
        panic!()
    };
    assert_eq!((renote.as_deref(), reply.as_deref()), (Some("orig"), None));
}

// l reacts ❤ and takes it back; e chooses another, which replaces the one
// given; the counts follow what the server took. A second press while
// one is out is refused.
#[test]
fn l_reacts_and_e_chooses_a_reaction() {
    let mut app = writer(vec![note(json!({"id": "a", "reactions": {"👍": 2}}))]);
    let jobs = app.handle_key(key('l'));
    assert_eq!(
        jobs[0].1,
        Job::React {
            id: "a".into(),
            reaction: Some("❤".into()),
            had: None
        }
    );
    assert!(app.handle_key(key('l')).is_empty(), "one at a time");
    app.handle_answer(
        jobs[0].0,
        Event::Reacted {
            id: "a".into(),
            reaction: Some("❤".into()),
            had: None,
            result: Ok(()),
        },
    );
    let n = &timeline(&app).items[0];
    assert_eq!(n.my_reaction.as_deref(), Some("❤"));
    assert_eq!(n.reactions.get("❤"), Some(&1));
    app.handle_key(key('e'));
    let jobs = app.handle_key(key('1'));
    assert_eq!(
        jobs[0].1,
        Job::React {
            id: "a".into(),
            reaction: Some("👍".into()),
            had: Some("❤".into())
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Reacted {
            id: "a".into(),
            reaction: Some("👍".into()),
            had: Some("❤".into()),
            result: Ok(()),
        },
    );
    let n = &timeline(&app).items[0];
    assert_eq!(n.reactions.get("👍"), Some(&3));
    assert_eq!(n.reactions.get("❤"), None);
    let jobs = app.handle_key(key('l'));
    assert_eq!(
        jobs[0].1,
        Job::React {
            id: "a".into(),
            reaction: None,
            had: Some("👍".into())
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Reacted {
            id: "a".into(),
            reaction: None,
            had: Some("👍".into()),
            result: Err(Error::api("x")),
        },
    );
    assert_eq!(
        timeline(&app).items[0].my_reaction.as_deref(),
        Some("👍"),
        "a refusal changes nothing"
    );
    assert!(app.in_flight.is_empty());
}

// b renotes the note shown and takes it back; your own renote selected
// takes it back and goes from the list.
#[test]
fn b_renotes_and_takes_it_back() {
    let mine = note(
        json!({"id": "rn", "userId": "u-me", "user": {"id": "u-me", "username": "me"},
                           "renoteId": "o", "renote": {"id": "o", "renoteCount": 1}}),
    );
    let mut app = writer(vec![note(json!({"id": "x", "renoteCount": 0})), mine]);
    let jobs = app.handle_key(key('b'));
    assert_eq!(
        jobs[0].1,
        Job::Renote {
            id: "x".into(),
            on: true
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Renoted {
            id: "x".into(),
            on: true,
            result: Ok(()),
        },
    );
    assert_eq!(timeline(&app).items[0].renote_count, 1);
    let jobs = app.handle_key(key('b'));
    assert_eq!(
        jobs[0].1,
        Job::Renote {
            id: "x".into(),
            on: false
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Renoted {
            id: "x".into(),
            on: false,
            result: Ok(()),
        },
    );
    app.handle_key(key('j'));
    let jobs = app.handle_key(key('b'));
    assert_eq!(
        jobs[0].1,
        Job::Renote {
            id: "o".into(),
            on: false
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Renoted {
            id: "o".into(),
            on: false,
            result: Ok(()),
        },
    );
    let ids: Vec<&str> = timeline(&app).items.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["x"]);
}

// D asks, and y deletes your own note; any other key keeps it; another's
// note is not offered.
#[test]
fn d_deletes_your_own_note_after_y() {
    let mut app = writer(vec![
        note(json!({"id": "m", "userId": "u-me", "user": {"id": "u-me", "username": "me"}})),
        note(json!({"id": "o", "userId": "u-bob", "user": {"id": "u-bob", "username": "bob"}})),
    ]);
    app.handle_key(key('D'));
    assert!(app.handle_key(key('n')).is_empty());
    assert_eq!(app.overlay, None, "the n answered the question");
    assert_eq!(app.status.as_ref().unwrap().text, "not deleted");
    app.handle_key(key('D'));
    let jobs = app.handle_key(key('y'));
    assert_eq!(jobs[0].1, Job::Delete { id: "m".into() });
    app.handle_answer(
        jobs[0].0,
        Event::Deleted {
            id: "m".into(),
            result: Ok(()),
        },
    );
    assert_eq!(timeline(&app).items.len(), 1);
    app.handle_key(key('D'));
    assert!(app.confirm_delete.is_none());
    assert!(app.handle_key(key('y')).is_empty());
}

// The notifications are marked read once their tab shows them, with a
// login that may; one made before msky asked for it is left alone.
#[test]
fn the_notifications_are_marked_read_once_shown() {
    let (mut app, jobs) = App::new(
        Some(Session {
            permissions: crate::miauth::PERMISSIONS
                .iter()
                .map(|p| p.to_string())
                .collect(),
            ..session()
        }),
        ORIGIN,
    );
    let seq = seq_of(&jobs, &Job::Notifications { until: None });
    let items: Vec<Notification> =
        serde_json::from_value(json!([{"id": "n1", "type": "follow"}])).unwrap();
    assert!(
        app.handle_answer(
            seq,
            Event::Notifications {
                until: None,
                result: Ok(items.clone())
            }
        )
        .is_empty(),
        "not shown yet"
    );
    let jobs = app.handle_key(key('4'));
    assert_eq!(jobs[0].1, Job::MarkRead);
    assert!(app.handle_key(key('1')).is_empty());
    assert!(app.handle_key(key('4')).is_empty(), "once a load");
    let mut old = logged_in(&[]);
    old.session.as_mut().unwrap().permissions = vec!["read:account".into()];
    old.notifications.set(items);
    assert!(old.handle_key(key('4')).is_empty());
    // A write with such a login says what to do.
    old.handle_key(key('1'));
    old.timelines[0].set(notes(&["a"]));
    assert!(old.handle_key(key('l')).is_empty());
    assert!(
        old.status
            .as_ref()
            .unwrap()
            .text
            .contains("lacks write:reactions")
    );
}

// ctrl+o opens the browser; files chosen are attached with an alt text
// each, and go with the post.
#[test]
fn files_chosen_in_the_browser_go_with_the_post() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("猫🐈.png"), b"x").unwrap();
    std::fs::write(dir.path().join("b.jpg"), b"x").unwrap();
    let mut app = writer(notes(&["a"]));
    app.browse_from = Some(dir.path().to_path_buf());
    app.handle_key(key('n'));
    app.handle_key(ctrl('o'));
    app.handle_key(key(' '));
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(compose(&app).media.len(), 2);
    app.handle_key(code(KeyCode::Tab));
    type_str(&mut app, "黒猫");
    let jobs = app.handle_key(ctrl('s'));
    let Job::Post { files, .. } = &jobs[0].1 else {
        panic!()
    };
    let got: Vec<(String, &str)> = files
        .iter()
        .map(|(p, a)| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                a.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [("b.jpg".to_string(), "黒猫"), ("猫🐈.png".to_string(), "")]
    );
}

fn user(v: Value) -> crate::api::types::User {
    serde_json::from_value(v).unwrap()
}

// 3 opens Search with the box focused; enter searches notes, t switches
// to accounts and searches them; the accounts page by offset, the notes
// by untilId, and an answer for another query is dropped.
#[test]
fn the_search_tab_searches_notes_and_accounts_and_pages_them() {
    let mut app = logged_in(&[]);
    app.handle_key(key('5'));
    assert_eq!(app.tab, Tab::Search);
    assert!(app.search.editing);
    type_str(&mut app, "猫 🐈");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(
        jobs[0].1,
        Job::SearchNotes {
            query: "猫 🐈".into(),
            until: None
        }
    );
    assert!(!app.search.editing);
    app.handle_answer(
        jobs[0].0,
        Event::SearchNotes {
            query: "猫 🐈".into(),
            until: None,
            result: Ok(notes(&["s2", "s1"])),
        },
    );
    let jobs = app.handle_key(key('j'));
    assert_eq!(
        jobs[0].1,
        Job::SearchNotes {
            query: "猫 🐈".into(),
            until: Some("s1".into())
        }
    );
    let jobs = app.handle_key(key('t'));
    assert_eq!(
        jobs[0].1,
        Job::SearchUsers {
            query: "猫 🐈".into(),
            offset: 0
        }
    );
    let first: Vec<_> = (0..3)
        .map(|i| user(json!({"id": format!("u{i}"), "username": format!("cat{i}")})))
        .collect();
    app.handle_answer(
        jobs[0].0,
        Event::SearchUsers {
            query: "猫 🐈".into(),
            offset: 0,
            result: Ok(first),
        },
    );
    let jobs = app.handle_key(key('j'));
    assert_eq!(
        jobs[0].1,
        Job::SearchUsers {
            query: "猫 🐈".into(),
            offset: 3
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::SearchUsers {
            query: "猫 🐈".into(),
            offset: 3,
            result: Ok(vec![]),
        },
    );
    assert_eq!(
        app.search.users.cursor, None,
        "a page with nothing new ends it"
    );
    assert!(app.handle_key(key('G')).is_empty());
    // / types again; esc leaves the box.
    app.handle_key(key('/'));
    assert!(app.search.editing);
    app.handle_key(code(KeyCode::Esc));
    assert!(!app.search.editing);
}

// enter on a note opens its author's profile, which esc leaves for where
// it was opened from; 5 opens your own; the profile's notes page.
#[test]
fn enter_opens_the_authors_profile_and_esc_goes_back() {
    let mut app = logged_in(&[]);
    app.timelines[0].set(vec![renote_of(
        json!({"id": "o", "userId": "u-bob", "user": {"id": "u-bob", "username": "bob"}}),
    )]);
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(
        jobs[0].1,
        Job::Profile {
            who: crate::tui::worker::Who::Id("u-bob".into())
        }
    );
    assert_eq!(app.tab, Tab::Profile);
    let bob = user(json!({"id": "u-bob", "username": "bob", "notesCount": 3}));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((bob, notes(&["b2", "b1"]))))),
    );
    assert_eq!(app.profile.user.as_ref().unwrap().id, "u-bob");
    let jobs = app.handle_key(key('j'));
    assert_eq!(
        jobs[0].1,
        Job::UserNotes {
            user_id: "u-bob".into(),
            until: "b1".into()
        }
    );
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.tab, Tab::Timeline(TimelineKind::Home));
    let jobs = app.handle_key(key('6'));
    assert_eq!(
        jobs[0].1,
        Job::Profile {
            who: crate::tui::worker::Who::Me
        }
    );
    // A visitor's 5 says how to log in, and asks nothing.
    let (mut visitor, _) = App::new(None, ORIGIN);
    assert!(visitor.handle_key(key('6')).is_empty());
    assert!(visitor.profile.error.as_ref().unwrap().contains("log in"));
}

// f follows the account shown, or unfollows it, or takes back a request;
// what the server says after is shown, pending for one that approves.
#[test]
fn f_follows_and_unfollows_the_account_shown() {
    let mut app = writer(vec![]);
    let jobs = app.open_profile(crate::tui::worker::Who::Id("u-bob".into()));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            user(json!({"id": "u-bob", "username": "bob", "host": "x.test"})),
            vec![],
        )))),
    );
    let jobs = app.handle_key(key('f'));
    assert_eq!(
        jobs[0].1,
        Job::Follow {
            user_id: "u-bob".into(),
            on: true,
            requested: false
        }
    );
    assert!(app.handle_key(key('f')).is_empty(), "one at a time");
    app.handle_answer(jobs[0].0, Event::Followed { user_id: "u-bob".into(), result: Ok(Box::new(user(json!({"id": "u-bob", "username": "bob", "host": "x.test", "hasPendingFollowRequestFromYou": true})))) });
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "asked to follow @bob@x.test; pending until it is accepted"
    );
    let jobs = app.handle_key(key('f'));
    assert_eq!(
        jobs[0].1,
        Job::Follow {
            user_id: "u-bob".into(),
            on: false,
            requested: true
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Followed {
            user_id: "u-bob".into(),
            result: Ok(Box::new(user(
                json!({"id": "u-bob", "username": "bob", "host": "x.test"}),
            ))),
        },
    );
    assert_eq!(app.status.as_ref().unwrap().text, "unfollowed @bob@x.test");
    // Your own profile cannot be followed.
    let jobs = app.open_profile(crate::tui::worker::Who::Me);
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            user(json!({"id": "u-me", "username": "me"})),
            vec![],
        )))),
    );
    assert!(app.handle_key(key('f')).is_empty());
}

// M mutes the account shown and unmutes it; B asks, and y blocks; a
// blocked account is unblocked at once.
#[test]
fn m_mutes_and_b_blocks_after_y() {
    let mut app = writer(vec![]);
    let jobs = app.open_profile(crate::tui::worker::Who::Id("u-bob".into()));
    app.handle_answer(
        jobs[0].0,
        Event::Profile(Ok(Box::new((
            user(json!({"id": "u-bob", "username": "bob"})),
            vec![],
        )))),
    );
    let jobs = app.handle_key(key('M'));
    assert_eq!(
        jobs[0].1,
        Job::Guard {
            user_id: "u-bob".into(),
            block: false,
            on: true
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Followed {
            user_id: "u-bob".into(),
            result: Ok(Box::new(user(
                json!({"id": "u-bob", "username": "bob", "isMuted": true}),
            ))),
        },
    );
    assert!(app.status.as_ref().unwrap().text.starts_with("muted @bob"));
    assert_eq!(
        app.handle_key(key('M'))[0].1,
        Job::Guard {
            user_id: "u-bob".into(),
            block: false,
            on: false
        }
    );
    app.in_flight.clear();
    assert!(app.handle_key(key('B')).is_empty());
    assert!(app.handle_key(key('n')).is_empty());
    assert_eq!(app.status.as_ref().unwrap().text, "not blocked");
    app.handle_key(key('B'));
    let jobs = app.handle_key(key('y'));
    assert_eq!(
        jobs[0].1,
        Job::Guard {
            user_id: "u-bob".into(),
            block: true,
            on: true
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Followed {
            user_id: "u-bob".into(),
            result: Ok(Box::new(user(
                json!({"id": "u-bob", "username": "bob", "isBlocking": true}),
            ))),
        },
    );
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .starts_with("blocked @bob")
    );
    assert_eq!(
        app.handle_key(key('B'))[0].1,
        Job::Guard {
            user_id: "u-bob".into(),
            block: true,
            on: false
        }
    );
}

// e offers the recent reactions and Misskey's default ones; typing
// searches the server's custom emoji, the names that start with it
// first; enter reacts with the one chosen, which becomes recent.
#[test]
fn the_reaction_picker_searches_the_servers_emoji() {
    let mut app = writer(vec![note(json!({"id": "a"}))]);
    app.emojis = [
        ("blobcat", "u1"),
        ("blobcat_yay", "u2"),
        ("ablobcat_dance", "u3"),
        ("neko", "u4"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(
        app.reaction_choices("")[..2],
        ["👍".to_string(), "❤".to_string()]
    );
    assert_eq!(
        app.reaction_choices("blob"),
        [":blobcat:", ":blobcat_yay:", ":ablobcat_dance:"]
    );
    app.handle_key(key('e'));
    type_str(&mut app, "blob");
    app.handle_key(code(KeyCode::Down));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(
        jobs[0].1,
        Job::React {
            id: "a".into(),
            reaction: Some(":blobcat_yay:".into()),
            had: None
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::Reacted {
            id: "a".into(),
            reaction: Some(":blobcat_yay:".into()),
            had: None,
            result: Ok(()),
        },
    );
    let n = &timeline(&app).items[0];
    assert_eq!(
        n.my_reaction.as_deref(),
        Some(":blobcat_yay@.:"),
        "as the server counts it"
    );
    assert_eq!(n.reactions.get(":blobcat_yay@.:"), Some(&1));
    assert_eq!(app.reaction_choices("")[0], ":blobcat_yay:");
    // The same one again is said to be given already.
    app.handle_key(key('e'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(jobs.is_empty());
    assert!(app.status.as_ref().unwrap().text.contains("already"));
}

fn chat_message(v: Value) -> crate::api::types::ChatMessage {
    serde_json::from_value(v).unwrap()
}

// The Chat tab (7) lists the conversations, one each, with whom; enter
// opens one and reads its messages; i writes and enter sends once; what
// could not be sent stays typed. The tab is read again every 15 s.
#[test]
fn the_chat_tab_lists_opens_and_sends() {
    let mut app = writer(vec![]);
    let jobs = app.handle_key(key('7'));
    assert_eq!(app.tab, Tab::Chat);
    assert_eq!(jobs[0].1, Job::ChatHistory);
    app.handle_answer(
        jobs[0].0,
        Event::ChatHistory(Ok(vec![
            chat_message(json!({"id": "m3", "createdAt": "3", "fromUserId": "u-bob", "fromUser": {"id": "u-bob", "username": "bob"}, "toUserId": "u-me", "text": "やあ 👋"})),
            chat_message(json!({"id": "m2", "createdAt": "2", "fromUserId": "u-me", "toUserId": "u-bob", "toUser": {"id": "u-bob", "username": "bob"}, "text": "前の"})),
            chat_message(json!({"id": "r1", "createdAt": "1", "fromUserId": "u-cat", "toRoomId": "room1", "toRoom": {"id": "room1", "name": "雑談"}, "text": "部屋"})),
        ])),
    );
    let ids: Vec<&str> = app
        .chat
        .convos
        .items
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids, ["m3", "r1"], "one entry a conversation");
    let jobs = app.handle_key(code(KeyCode::Enter));
    let with = crate::tui::worker::ChatWith::User("u-bob".into());
    assert_eq!(
        jobs[0].1,
        Job::ChatMessages {
            with: with.clone(),
            until: None
        }
    );
    app.handle_answer(
        jobs[0].0,
        Event::ChatMessages {
            with: with.clone(),
            until: None,
            result: Ok(vec![]),
        },
    );
    assert_eq!(app.chat.open.as_ref().unwrap().title, "@bob");
    app.handle_key(key('i'));
    type_str(&mut app, "こんにちは 🇯🇵");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(
        jobs[0].1,
        Job::ChatSend {
            with: with.clone(),
            text: "こんにちは 🇯🇵".into()
        }
    );
    assert!(app.handle_key(code(KeyCode::Enter)).is_empty(), "sent once");
    app.handle_answer(
        jobs[0].0,
        Event::ChatSent {
            with: with.clone(),
            text: "こんにちは 🇯🇵".into(),
            result: Err(Error::api("x")),
        },
    );
    assert_eq!(
        app.chat.open.as_ref().unwrap().input.text(),
        "こんにちは 🇯🇵"
    );
    let jobs = app.handle_key(code(KeyCode::Enter));
    app.handle_answer(
        jobs[0].0,
        Event::ChatSent {
            with,
            text: "こんにちは 🇯🇵".into(),
            result: Ok(Box::new(chat_message(
                json!({"id": "m9", "fromUserId": "u-me", "text": "こんにちは 🇯🇵"}),
            ))),
        },
    );
    let o = app.chat.open.as_ref().unwrap();
    assert!(o.input.text().is_empty());
    assert_eq!(o.messages.items[0].id, "m9");
    // Every 15 s the conversation is read again; not sooner.
    let now = Instant::now();
    assert!(app.poll_chat(now).is_empty());
    assert_eq!(app.poll_chat(now + Duration::from_secs(16)).len(), 1);
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.chat.open.is_none());
}

// A server without chat, or a role that may not chat, takes the tab away.
#[test]
fn a_server_without_chat_takes_the_tab_away() {
    let mut app = writer(vec![]);
    let jobs = app.handle_key(key('7'));
    app.handle_answer(
        jobs[0].0,
        Event::ChatHistory(Err(Error::api(
            "chat/history failed: NO_SUCH_ENDPOINT: No such endpoint.",
        ))),
    );
    assert!(!app.tabs.contains(&Tab::Chat));
    assert_ne!(app.tab, Tab::Chat);
    assert!(app.status.as_ref().unwrap().text.contains("has no chat"));
    // A visitor's Chat tab says how to log in.
    let (mut v, _) = App::new(None, ORIGIN);
    assert!(v.handle_key(key('7')).is_empty());
    assert!(v.chat.convos.error.as_ref().unwrap().contains("log in"));
}

// s opens the settings; the theme row opens bsky's themes, previewed as
// the selection moves, put back on esc and kept on enter; the language
// row switches the client to Japanese at once; the tabs row takes names
// in order; x puts a row back. What is kept goes to the event loop to
// write.
#[test]
fn the_settings_screen_changes_and_keeps_theme_language_and_tabs() {
    use crate::tui::theme::{ColorDepth, THEMES};
    let mut app = logged_in(&[]);
    app.apply_settings(
        crate::config::Settings::default(),
        ColorDepth::TrueColor,
        Environment::default(),
        None,
    );
    assert_eq!(THEMES[app.theme_index].name, "terminal");
    let start = app.theme_index;
    app.handle_key(key('s'));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
    app.handle_key(key('j'));
    assert_ne!(app.theme_index, start, "previewed");
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.theme_index, start, "put back");
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings { selected: 0, .. })
    ));
    assert!(app.take_settings_save().is_none());
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    let saved = app.take_settings_save().expect("kept");
    assert_eq!(
        saved.theme.as_deref(),
        Some(THEMES[(start + 1) % THEMES.len()].name)
    );
    app.settings_saved(Ok(()));
    assert!(app.status.as_ref().unwrap().text.starts_with("theme: "));
    // The language, at once.
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(app.overlay, Some(Overlay::Languages { .. })));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(
        app.take_settings_save().unwrap().language.as_deref(),
        Some("ja")
    );
    assert_eq!(crate::i18n::t(Tab::Profile.title()), "プロフィール");
    assert_eq!(app.settings_rows()[1].value, "日本語");
    // x: back to the language the environment asks for.
    app.handle_key(key('x'));
    assert_eq!(app.take_settings_save().unwrap().language, None);
    assert_eq!(crate::i18n::current(), crate::i18n::Lang::En);
    // The tabs, by name; a name that is no tab is refused.
    for _ in 0..3 {
        app.handle_key(key('j'));
    }
    app.handle_key(code(KeyCode::Enter));
    assert!(matches!(
        app.overlay,
        Some(Overlay::Settings {
            selected: 4,
            edit: Some(_)
        })
    ));
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    type_str(&mut app, "chat, home, nope");
    app.handle_key(code(KeyCode::Enter));
    assert!(app.status.as_ref().unwrap().text.contains("nope"));
    assert!(app.take_settings_save().is_none());
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    type_str(&mut app, "chat, home");
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.tabs, [Tab::Chat, Tab::Timeline(TimelineKind::Home)]);
    assert_eq!(app.tab, Tab::Chat);
    assert_eq!(
        app.take_settings_save().unwrap().tabs,
        Some(vec!["chat".to_string(), "home".to_string()])
    );
    app.handle_key(key('x'));
    assert_eq!(app.tabs, DEFAULT_TABS);
    app.handle_key(code(KeyCode::Esc));
    assert!(app.overlay.is_none());
}

// A variable decides its setting for the run: the row says so and enter
// does not change it; the folder and browser the settings name reach the
// jobs only without one.
#[test]
fn a_variable_decides_its_setting_and_the_settings_reach_the_jobs() {
    use crate::tui::theme::ColorDepth;
    let mut app = writer(vec![]);
    let settings = crate::config::Settings {
        download_dir: Some("/tmp/saved".into()),
        browser: Some("firefox".into()),
        recent_reactions: Some(vec![":blobcat:".into()]),
        ..Default::default()
    };
    app.apply_settings(
        settings.clone(),
        ColorDepth::TrueColor,
        Environment::default(),
        None,
    );
    assert_eq!(app.recent_reactions, [":blobcat:"]);
    assert_eq!(
        app.download_dir(),
        Some(std::path::PathBuf::from("/tmp/saved"))
    );
    assert_eq!(app.browser().as_deref(), Some("firefox"));
    let env = Environment {
        browser: Some("lynx".into()),
        ..Default::default()
    };
    app.apply_settings(settings, ColorDepth::TrueColor, env, None);
    assert_eq!(app.browser(), None, "MSKY_BROWSER wins");
    let row = &app.settings_rows()[3];
    assert!(!row.editable);
    assert!(row.note.contains("MSKY_BROWSER"), "{row:?}");
    // NO_COLOR: monochrome, and T says why it does nothing.
    app.apply_settings(
        Default::default(),
        ColorDepth::None,
        Environment::default(),
        None,
    );
    assert_eq!(
        crate::tui::theme::THEMES[app.theme_index].name,
        "monochrome"
    );
    app.handle_key(key('T'));
    assert!(app.overlay.is_none());
    // A settings file that could not be read is left as it is.
    app.apply_settings(
        Default::default(),
        ColorDepth::TrueColor,
        Environment::default(),
        Some("broken".into()),
    );
    app.handle_key(key('T'));
    app.handle_key(code(KeyCode::Enter));
    assert!(app.take_settings_save().is_none());
    assert!(app.status.as_ref().unwrap().text.contains("not kept"));
}

// The language: the settings', else the locale's, else English.
#[test]
fn the_language_is_the_settings_then_the_locale() {
    use super::settings::language;
    use crate::i18n::Lang;
    let env = |l: &str| Environment::read(|k| (k == "LANG").then(|| l.to_string()));
    let set = |l: Option<&str>| crate::config::Settings {
        language: l.map(str::to_string),
        ..Default::default()
    };
    assert_eq!(language(&set(None), &env("ja_JP.UTF-8")), Lang::Ja);
    assert_eq!(language(&set(Some("en")), &env("ja_JP.UTF-8")), Lang::En);
    assert_eq!(language(&set(Some("ja")), &env("C.UTF-8")), Lang::Ja);
    assert_eq!(language(&set(Some("klingon")), &env("fr_FR")), Lang::En);
    let lc_all = Environment::read(|k| match k {
        "LC_ALL" => Some("ja_JP.UTF-8".into()),
        "LANG" => Some("en_US.UTF-8".into()),
        _ => None,
    });
    assert_eq!(language(&set(None), &lc_all), Lang::Ja);
}

// e on your own Profile tab opens the editor, which starts from the
// profile read; ctrl+s sends only what changed, once; a failure keeps the
// editor open; esc closes it. On another account's profile e reacts.
#[test]
fn e_on_your_own_profile_edits_it() {
    let mut app = writer(vec![]);
    let jobs = app.handle_key(key('6'));
    assert_eq!(app.tab, Tab::Profile);
    let seq = jobs[0].0;
    app.handle_answer(
        seq,
        Event::Profile(Ok(Box::new((
            serde_json::from_value(json!({"id": "u-me", "username": "me"})).unwrap(),
            vec![],
        )))),
    );
    let jobs = app.handle_key(key('e'));
    assert_eq!(jobs[0].1, Job::LoadProfileEditor);
    // Keys before the profile came are not typed.
    app.handle_key(key('x'));
    app.handle_answer(
        jobs[0].0,
        Event::ProfileEditor(Ok(Box::new(
            serde_json::from_value(json!({"id": "u-me", "username": "me", "name": "猫", "description": "説明\n二行目"})).unwrap(),
        ))),
    );
    let Some(Overlay::EditProfile(e)) = &app.overlay else {
        panic!("{:?}", app.overlay)
    };
    assert_eq!(e.fields[0].text(), "猫");
    assert_eq!(e.fields[1].text(), "説明\n二行目");
    // Unchanged: nothing is sent.
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
            .is_empty()
    );
    assert!(app.overlay.is_none());
    let jobs = app.handle_key(key('e'));
    app.handle_answer(
        jobs[0].0,
        Event::ProfileEditor(Ok(Box::new(
            serde_json::from_value(json!({"id": "u-me", "username": "me", "name": "猫"})).unwrap(),
        ))),
    );
    type_str(&mut app, " 🐈‍⬛");
    app.handle_key(code(KeyCode::Tab));
    app.handle_key(code(KeyCode::Tab));
    app.handle_key(code(KeyCode::Tab));
    type_str(&mut app, "/tmp/banner.txt");
    let none = app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(none.is_empty(), "not a picture");
    assert!(app.status.as_ref().unwrap().text.contains("not a picture"));
    for _ in 0..4 {
        app.handle_key(code(KeyCode::Backspace));
    }
    type_str(&mut app, ".png");
    let jobs = app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    let Job::SaveProfile(change) = &jobs[0].1 else {
        panic!("{jobs:?}")
    };
    assert_eq!(change.name.as_deref(), Some("猫 🐈‍⬛"));
    assert_eq!(change.description, None);
    assert_eq!(
        change.banner,
        Some(std::path::PathBuf::from("/tmp/banner.png"))
    );
    assert!(
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
            .is_empty(),
        "once"
    );
    app.handle_answer(
        jobs[0].0,
        Event::ProfileSaved(Err(Error::api("i/update failed: INVALID_PARAM"))),
    );
    assert!(
        matches!(app.overlay, Some(Overlay::EditProfile(_))),
        "kept open"
    );
    let jobs = app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_answer(
        jobs[0].0,
        Event::ProfileSaved(Ok(Box::new(
            serde_json::from_value(json!({"id": "u-me", "username": "me", "name": "猫 🐈‍⬛"}))
                .unwrap(),
        ))),
    );
    assert!(app.overlay.is_none());
    assert_eq!(
        app.profile.user.as_ref().unwrap().name.as_deref(),
        Some("猫 🐈‍⬛")
    );
    assert_eq!(app.status.as_ref().unwrap().text, "profile saved");
}

fn feed(kind: FeedKind, id: &str, name: &str) -> Feed {
    Feed {
        kind,
        host: crate::api::authority(ORIGIN).to_string(),
        id: id.into(),
        name: name.into(),
    }
}

// The tabs setting names antennas, lists and channels as kind:host:id:name;
// those of another server are kept in it but not shown here.
#[test]
fn feed_tabs_come_from_the_setting_for_their_server_only() {
    let mut app = logged_in(&["a"]);
    let here = crate::api::authority(ORIGIN).to_string();
    let names = vec![
        "home".to_string(),
        format!("list/{here}/l1/猫, 犬"),
        "antenna/other.example/a9/よそ".to_string(),
        format!("channel/{here}/c1/お絵描き 🎨"),
        "list/broken".to_string(),
    ];
    let (tabs, warn) = app.tabs_for_host(&names);
    assert_eq!(
        tabs,
        [
            Tab::Timeline(TimelineKind::Home),
            Tab::Feed(0),
            Tab::Feed(1)
        ]
    );
    assert!(warn.unwrap().contains("list/broken"));
    assert_eq!(app.feeds[0].feed.as_ref().unwrap().name, "猫, 犬");
    assert_eq!(app.tab_label(Tab::Feed(1)), "# お絵描き 🎨");
    app.settings.tabs = Some(names);
    app.tabs = tabs;
    let kept = app.tabs_setting();
    assert_eq!(
        kept,
        [
            "home".to_string(),
            format!("list/{here}/l1/猫  犬"),
            format!("channel/{here}/c1/お絵描き 🎨"),
            "antenna/other.example/a9/よそ".to_string(),
        ],
        "the comma goes, the other server's stays"
    );
    // A feed tab reads its endpoint, pages by untilId, and R reads again.
    let jobs = app.handle_key(key('2'));
    let f = app.feeds[0].feed.clone().unwrap();
    assert_eq!(
        jobs[0].1,
        Job::FeedNotes {
            feed: f.clone(),
            until: None
        }
    );
    assert_eq!(f.kind.endpoint(), ("notes/user-list-timeline", "listId"));
    app.handle_answer(
        jobs[0].0,
        Event::FeedNotes {
            feed: f.clone(),
            until: None,
            result: Ok(notes(&["n1", "n2"])),
        },
    );
    assert_eq!(app.selected_note().unwrap().id, "n1");
    let jobs = app.handle_key(key('j'));
    assert_eq!(
        jobs[0].1,
        Job::FeedNotes {
            feed: f.clone(),
            until: Some("n2".into())
        }
    );
    let jobs = app.handle_key(key('R'));
    assert_eq!(
        jobs[0].1,
        Job::FeedNotes {
            feed: f,
            until: None
        }
    );
}

// + offers the tabs not shown and the account's feeds once read; enter
// adds one as the last tab, shows it and keeps it; x closes the tab shown
// once y confirms, the last one staying.
#[test]
fn plus_adds_a_tab_and_x_closes_one() {
    let mut app = logged_in(&["a"]);
    let jobs = app.handle_key(key('+'));
    assert_eq!(jobs[0].1, Job::FeedChoices);
    let choices = app.tab_choices();
    assert_eq!(
        choices,
        [TabChoice::Builtin(Tab::Timeline(TimelineKind::Social))]
    );
    let ant = feed(FeedKind::Antenna, "a1", "ニュース 📰");
    app.handle_answer(
        jobs[0].0,
        Event::FeedChoices(Ok(vec![ant.clone(), feed(FeedKind::List, "l1", "友達")])),
    );
    assert_eq!(app.tab_choices().len(), 3);
    app.handle_key(key('j'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.tab, Tab::Feed(0));
    assert_eq!(*app.tabs.last().unwrap(), Tab::Feed(0));
    assert_eq!(
        jobs[0].1,
        Job::FeedNotes {
            feed: ant.clone(),
            until: None
        }
    );
    let saved = app.take_settings_save().unwrap().tabs.unwrap();
    assert_eq!(saved.last().unwrap(), &ant.setting());
    // Shown already, it is not offered again.
    let jobs = app.handle_key(key('+'));
    app.handle_answer(jobs[0].0, Event::FeedChoices(Ok(vec![ant.clone()])));
    assert!(!app.tab_choices().contains(&TabChoice::Feed(ant)));
    app.handle_key(code(KeyCode::Esc));
    // x, then anything but y: kept.
    app.handle_key(key('x'));
    app.handle_key(key('n'));
    assert!(app.tabs.contains(&Tab::Feed(0)));
    app.handle_key(key('x'));
    app.handle_key(key('y'));
    assert!(!app.tabs.contains(&Tab::Feed(0)));
    assert_eq!(app.tab, Tab::Chat, "the one before it");
    assert!(
        app.take_settings_save()
            .unwrap()
            .tabs
            .unwrap()
            .iter()
            .all(|t| !t.starts_with("antenna/"))
    );
    // The last tab stays.
    app.tabs = vec![Tab::Chat];
    app.handle_key(key('x'));
    assert!(app.confirm_close_tab.is_none());
    assert!(app.status.as_ref().unwrap().text.contains("last tab"));
}

// ! on another's note reports its author with the note: a reason, a
// comment, enter asks, and only y sends; any other key goes back.
#[test]
fn bang_reports_a_note_once_y_confirms() {
    let mut app = writer(vec![note(
        json!({"id": "n1", "userId": "u-bob", "user": {"id": "u-bob", "username": "bob"}, "text": "x"}),
    )]);
    app.handle_key(key('!'));
    let Some(Overlay::Report(r)) = &app.overlay else {
        panic!("{:?}", app.overlay)
    };
    assert_eq!(r.user_id, "u-bob");
    assert!(r.shown.starts_with("@bob@"), "{}", r.shown);
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Tab));
    type_str(&mut app, "荒らし 🚫");
    app.handle_key(code(KeyCode::Tab));
    assert!(
        app.handle_key(code(KeyCode::Enter)).is_empty(),
        "asked first"
    );
    assert!(app.status.as_ref().unwrap().text.contains("press y"));
    assert!(app.handle_key(key('n')).is_empty(), "not y: back");
    assert!(matches!(app.overlay, Some(Overlay::Report(_))));
    app.handle_key(code(KeyCode::Enter));
    let jobs = app.handle_key(key('y'));
    let Job::Report { user_id, comment } = &jobs[0].1 else {
        panic!("{jobs:?}")
    };
    assert_eq!(user_id, "u-bob");
    assert!(
        comment.starts_with("Breaks the server's rules or the law\nNote: "),
        "{comment}"
    );
    assert!(comment.ends_with("\n-----\n荒らし 🚫"), "{comment}");
    assert!(app.handle_key(key('y')).is_empty(), "sent once");
    app.handle_answer(jobs[0].0, Event::Reported(Ok(())));
    assert!(app.overlay.is_none());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .starts_with("reported @bob@")
    );
    // Your own note is not reported.
    let mut app = writer(vec![note(
        json!({"id": "n2", "userId": "u-me", "user": {"id": "u-me", "username": "me"}, "text": "x"}),
    )]);
    app.handle_key(key('!'));
    assert!(app.overlay.is_none());
}

fn thread_of(id: &str) -> Box<crate::tui::worker::Thread> {
    Box::new(crate::tui::worker::Thread {
        ancestors: notes(&["root"]),
        note: notes(&[id]).remove(0),
        replies: vec![(notes(&["re"]).remove(0), vec![])],
    })
}

// The selection resting 400 ms on a note reads its thread ahead, once and
// not counted as pending; v then shows it at once, without asking; one
// read before a write is not used.
#[test]
fn a_thread_is_read_ahead_where_the_selection_rests() {
    let mut app = logged_in(&["a", "b"]);
    let t0 = Instant::now();
    assert!(app.poll_read_ahead(t0).is_empty(), "just arrived");
    assert!(
        app.poll_read_ahead(t0 + Duration::from_millis(100))
            .is_empty()
    );
    let pending = app.pending;
    let jobs = app.poll_read_ahead(t0 + Duration::from_millis(450));
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].1, Job::ReadAhead { id: "a".into() });
    assert_eq!(app.pending, pending, "not waited for");
    assert!(
        app.poll_read_ahead(t0 + Duration::from_millis(900))
            .is_empty(),
        "once"
    );
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            id: "a".into(),
            result: Ok(thread_of("a")),
        },
    );
    assert_eq!(app.pending, pending);
    assert_eq!(app.read_ahead.kept(), 1);
    // v: shown at once, nothing asked.
    let jobs = app.handle_key(key('v'));
    assert!(jobs.is_empty(), "{jobs:?}");
    let th = app.threads.last().unwrap();
    assert!(th.list.loaded);
    assert_eq!(th.list.current().unwrap().note.id, "a");
    app.handle_key(code(KeyCode::Esc));
    // Moving on: b rests, is read, then a reaction is sent: b's thread
    // read before it is not used.
    app.handle_key(key('j'));
    let t1 = t0 + Duration::from_secs(2);
    app.poll_read_ahead(t1);
    let jobs = app.poll_read_ahead(t1 + Duration::from_millis(450));
    let seq = jobs[0].0;
    app.handle_key(key('l'));
    app.handle_answer(
        seq,
        Event::ReadAhead {
            id: "b".into(),
            result: Ok(thread_of("b")),
        },
    );
    assert_eq!(app.read_ahead.kept(), 0);
    let jobs = app.handle_key(key('v'));
    assert_eq!(jobs[0].1, Job::Thread { id: "b".into() });
}

// v while the thread is still being read ahead waits for that answer,
// counted as pending, instead of asking again.
#[test]
fn v_waits_for_a_thread_being_read_ahead() {
    let mut app = logged_in(&["a"]);
    let t0 = Instant::now();
    app.poll_read_ahead(t0);
    let jobs = app.poll_read_ahead(t0 + Duration::from_millis(450));
    let pending = app.pending;
    assert!(app.handle_key(key('v')).is_empty());
    assert_eq!(app.pending, pending + 1);
    assert!(!app.threads[0].list.loaded);
    app.handle_answer(
        jobs[0].0,
        Event::ReadAhead {
            id: "a".into(),
            result: Ok(thread_of("a")),
        },
    );
    assert_eq!(app.pending, pending);
    assert!(app.threads[0].list.loaded);
    assert_eq!(app.threads[0].list.items.len(), 3);
}

// The Notifications tab counts what is unread, as bsky's does: the
// server's count, (•) for a server that only says there are some, and
// nothing once they are marked read. It is asked at most once a minute.
#[test]
fn the_notifications_tab_counts_the_unread() {
    let mut app = writer(vec![]);
    let t0 = Instant::now();
    app.unread_polled = None;
    let jobs = app.ask_unread(t0);
    assert_eq!(jobs[0].1, Job::Unread);
    assert!(app.ask_unread(t0 + Duration::from_secs(30)).is_empty());
    assert_eq!(app.ask_unread(t0 + Duration::from_secs(61)).len(), 1);
    app.handle_answer(jobs[0].0, Event::Unread(Ok((Some(3), true))));
    assert_eq!(app.unread_label().as_deref(), Some("(3)"));
    app.handle_answer(0, Event::Unread(Ok((Some(120), true))));
    assert_eq!(app.unread_label().as_deref(), Some("(99+)"));
    app.handle_answer(0, Event::Unread(Ok((None, true))));
    assert_eq!(app.unread_label().as_deref(), Some("(•)"));
    app.handle_answer(0, Event::Unread(Ok((Some(2), true))));
    app.handle_answer(0, Event::MarkedRead(Ok(())));
    assert_eq!(app.unread_label(), None);
    // A visitor asks nothing.
    let (mut v, _) = App::new(None, ORIGIN);
    assert!(v.ask_unread(t0 + Duration::from_secs(600)).is_empty());
}
