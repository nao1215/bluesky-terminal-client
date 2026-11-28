use super::*;
use crate::{
    api::{event::Event, relay::Outcome},
    error::Error,
    tui::worker::Page,
};
use k256::schnorr::SigningKey;

fn fixture() -> (App, Event) {
    let note = Event::sign(
        &SigningKey::from_slice(&[1; 32]).unwrap(),
        100,
        1,
        vec![],
        "hello 猫".into(),
    )
    .unwrap();
    let mut app = App::new(
        note.pubkey.clone(),
        2,
        Settings::default(),
        ColorDepth::TrueColor,
    );
    app.language = "en";
    app.lists[0].notes.push(note.clone());
    app.lists[0].loaded = true;
    (app, note)
}
fn key(app: &mut App, code: KeyCode) -> Vec<Job> {
    app.key(KeyEvent::from(code))
}
fn ch(app: &mut App, c: char) -> Vec<Job> {
    key(app, KeyCode::Char(c))
}
fn send(app: &mut App) -> Vec<Job> {
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
}
fn written(seq: u64) -> Answer {
    Answer::Written(
        seq,
        Ok(Outcome {
            value: 1,
            answered: 1,
            warnings: vec![],
        }),
    )
}

#[test]
fn draft_enter_paste_cancel_and_empty_send_never_publish() {
    let (mut app, _) = fixture();
    assert!(ch(&mut app, 'n').is_empty());
    assert!(send(&mut app).is_empty());
    app.paste("draft q\n猫");
    assert!(key(&mut app, KeyCode::Enter).is_empty());
    assert!(ch(&mut app, 'l').is_empty());
    assert!(!app.quit);
    assert!(key(&mut app, KeyCode::Esc).is_empty());
    assert!(app.overlay.is_none());
    assert!(app.pending.is_empty());
}

#[test]
fn new_post_only_ctrl_s_submits_and_failed_send_preserves_editor() {
    let (mut app, _) = fixture();
    ch(&mut app, 'n');
    app.paste("hello\n世界");
    let jobs = send(&mut app);
    let Job::Write { seq, event } = &jobs[0] else {
        panic!()
    };
    assert_eq!(event.kind, 1);
    assert_eq!(event.content, "hello\n世界");
    assert!(event.tags.is_empty());
    assert!(send(&mut app).is_empty());
    app.answer(Answer::Written(
        *seq,
        Err(Error::network("no relay accepts")),
    ));
    assert!(
        matches!(&app.overlay, Some(Overlay::Compose { text, sending: None, .. }) if text.text() == "hello\n世界")
    );
    let jobs = send(&mut app);
    let Job::Write { seq, .. } = jobs[0] else {
        panic!()
    };
    app.answer(written(seq));
    assert!(app.overlay.is_none());
    assert_eq!(app.message, "Sent");
}

#[test]
fn reply_and_nested_reply_carry_nip10_root_reply_and_participants() {
    let (mut app, note) = fixture();
    ch(&mut app, 'r');
    app.paste("reply");
    let jobs = send(&mut app);
    let Job::Write { event, .. } = &jobs[0] else {
        panic!()
    };
    assert_eq!(
        event.tags,
        vec![vec!["e", &note.id, "", "root"], vec!["p", &note.pubkey]]
    );
    let parent = Event::sign(
        &SigningKey::from_slice(&[2; 32]).unwrap(),
        101,
        1,
        event.tags.clone(),
        "parent".into(),
    )
    .unwrap();
    let nested = note::draft(1, "nested".into(), Some(&parent));
    assert_eq!(nested.tags[0], vec!["e", &note.id, "", "root"]);
    assert_eq!(nested.tags[1], vec!["e", &parent.id, "", "reply"]);
    assert!(nested.tags.contains(&vec!["p".into(), note.pubkey.clone()]));
    assert!(
        nested
            .tags
            .contains(&vec!["p".into(), parent.pubkey.clone()])
    );
}

#[test]
fn reactions_are_optimistic_and_no_acceptance_rolls_back_even_after_switching_tabs() {
    for (c, kind) in [('l', 7), ('b', 6)] {
        let (mut app, note) = fixture();
        let jobs = ch(&mut app, c);
        let Job::Write { seq, event } = &jobs[0] else {
            panic!()
        };
        assert_eq!(event.kind, kind);
        if kind == 7 {
            assert_eq!(event.content, "+");
            assert_eq!(event.tags.last().unwrap(), &vec!["k", "1"]);
        } else {
            assert_eq!(serde_json::from_str::<Event>(&event.content).unwrap(), note);
        }
        assert_eq!(event.tags[0], vec!["e", &note.id]);
        assert_eq!(event.tags[1], vec!["p", &note.pubkey]);
        assert!(app.marks.contains(&(note.id.clone(), kind)));
        assert!(ch(&mut app, c).is_empty());
        ch(&mut app, '2');
        app.answer(Answer::Written(
            *seq,
            Ok(Outcome {
                value: 0,
                answered: 0,
                warnings: vec![],
            }),
        ));
        assert!(!app.marks.contains(&(note.id.clone(), kind)));
        ch(&mut app, '1');
        let retry = ch(&mut app, c);
        let Job::Write { seq, .. } = retry[0] else {
            panic!()
        };
        app.answer(written(seq));
        assert!(app.marks.contains(&(note.id.clone(), kind)));
    }
}

#[test]
fn independent_concurrent_reactions_do_not_rollback_each_other() {
    let (mut app, note) = fixture();
    let likes = ch(&mut app, 'l');
    let boosts = ch(&mut app, 'b');
    let Job::Write { seq: like, .. } = likes[0] else {
        panic!()
    };
    let Job::Write { seq: boost, .. } = boosts[0] else {
        panic!()
    };
    app.answer(written(boost));
    app.answer(Answer::Written(like, Err(Error::network("rejected"))));
    assert_eq!(app.marks, HashSet::from([(note.id, 6)]));
}

#[test]
fn tabs_thread_stack_paging_and_stale_answers() {
    let (mut app, note) = fixture();
    assert!(matches!(
        &ch(&mut app, '2')[0],
        Job::Load {
            target: Target::Tab(Tab::Notifications),
            count: 20,
            ..
        }
    ));
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.tab, Tab::Profile);
    key(&mut app, KeyCode::BackTab);
    assert_eq!(app.tab, Tab::Notifications);
    ch(&mut app, '1');
    app.lists[0].more = true;
    let jobs = ch(&mut app, 'G');
    let Job::Load { seq, count, .. } = jobs[0] else {
        panic!()
    };
    assert_eq!(count, 20);
    assert!(ch(&mut app, 'j').is_empty());
    app.answer(Answer::Loaded(
        seq,
        Ok(Outcome {
            answered: 1,
            warnings: vec!["NOTICE test".into()],
            value: Page {
                notes: vec![note.clone()],
                profile: None,
                more: false,
                append: false,
                until: Some(100),
                boundary: 1,
            },
        }),
    ));
    assert!(!app.lists[0].more);
    assert_eq!(app.answered, 1);
    assert_eq!(app.message, "NOTICE test");
    assert!(ch(&mut app, 'G').is_empty());
    let jobs = ch(&mut app, 'v');
    let Job::Load {
        seq,
        target: Target::Thread(ref selected),
        ..
    } = jobs[0]
    else {
        panic!()
    };
    assert_eq!(selected, &note);
    assert_eq!(app.threads.len(), 1);
    key(&mut app, KeyCode::Esc);
    app.answer(Answer::Loaded(seq, Err(Error::network("stale"))));
    assert_ne!(app.message, "stale");
    assert!(app.threads.is_empty());
}

#[test]
fn theme_preview_cancel_settings_language_and_copy() {
    let (mut app, note) = fixture();
    let original = app.theme;
    ch(&mut app, 'T');
    ch(&mut app, 'j');
    assert_ne!(app.theme, original);
    key(&mut app, KeyCode::Esc);
    assert_eq!(app.theme, original);
    assert!(!app.save);
    ch(&mut app, 'T');
    ch(&mut app, 'j');
    key(&mut app, KeyCode::Enter);
    assert!(app.save);
    assert_eq!(app.settings.theme.as_deref(), Some(theme::THEMES[4].name));
    ch(&mut app, 's');
    ch(&mut app, 'j');
    key(&mut app, KeyCode::Enter);
    ch(&mut app, 'j');
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.language, "ja");
    key(&mut app, KeyCode::Esc);
    ch(&mut app, 'c');
    let copy = app.copy.as_ref().unwrap();
    assert!(copy.starts_with("nostr:nevent1"));
    let Identifier::Event {
        id, author, kind, ..
    } = Identifier::decode(copy.strip_prefix("nostr:").unwrap()).unwrap()
    else {
        panic!()
    };
    assert_eq!(hex::encode(id), note.id);
    assert_eq!(hex::encode(author.unwrap()), note.pubkey);
    assert_eq!(kind, Some(1));
}

#[test]
fn image_urls_and_imeta_are_http_only_and_deduplicated() {
    let (_, mut note) = fixture();
    note.content = "https://example.invalid/a.JPG?q=1 https://example.invalid/a.JPG?q=1 file:///tmp/a.png https://example.invalid/page".into();
    note.tags = vec![
        vec![
            "imeta".into(),
            "url https://example.invalid/photo".into(),
            "m image/png".into(),
        ],
        vec!["imeta".into(), "url javascript:bad".into()],
    ];
    assert_eq!(
        note::pictures(&note),
        vec![
            "https://example.invalid/a.JPG?q=1",
            "https://example.invalid/photo"
        ]
    );
}

#[test]
fn inclusive_pages_keep_selection_deduplicate_and_request_the_whole_boundary() {
    let (mut app, note) = fixture();
    let job = app.load(true).pop().unwrap();
    let Job::Load { seq, until, .. } = job else {
        panic!()
    };
    assert_eq!(until, None);
    let events: Vec<_> = (0..20)
        .map(|i| {
            Event::sign(
                &SigningKey::from_slice(&[1; 32]).unwrap(),
                100,
                1,
                vec![],
                format!("boundary {i}"),
            )
            .unwrap()
        })
        .collect();
    app.answer(Answer::Loaded(
        seq,
        Ok(Outcome {
            answered: 2,
            warnings: vec![],
            value: Page {
                notes: events.clone(),
                profile: None,
                more: true,
                append: false,
                until: Some(100),
                boundary: 20,
            },
        }),
    ));
    let job = ch(&mut app, 'G').pop().unwrap();
    let Job::Load {
        seq, until, count, ..
    } = job
    else {
        panic!()
    };
    assert_eq!(until, Some(100));
    assert_eq!(count, 40);
    let selected = app.selected().unwrap().id.clone();
    let mut more = events;
    more.push(note.clone());
    app.answer(Answer::Loaded(
        seq,
        Ok(Outcome {
            answered: 2,
            warnings: vec![],
            value: Page {
                notes: more,
                profile: None,
                more: false,
                append: true,
                until: Some(100),
                boundary: 21,
            },
        }),
    ));
    assert_eq!(app.list().notes.len(), 21);
    assert_eq!(app.selected().unwrap().id, selected);
    assert!(ch(&mut app, 'G').is_empty());
    assert_eq!(app.selected(), Some(&note));
}

#[test]
fn a_failed_thread_fetch_keeps_the_known_note_and_authorization_is_a_message() {
    let (mut app, note) = fixture();
    let job = ch(&mut app, 'v').pop().unwrap();
    let Job::Load { seq, .. } = job else { panic!() };
    app.answer(Answer::Loaded(
        seq,
        Err(Error::network("relays unavailable")),
    ));
    assert_eq!(app.selected(), Some(&note));
    app.answer(Answer::Authorize(
        "https://signer.invalid/authorize?id=test".into(),
    ));
    assert_eq!(
        app.message,
        "Remote signer authorization: https://signer.invalid/authorize?id=test"
    );
}

#[test]
fn esc_closes_the_settings_after_a_language_is_chosen() {
    let (mut app, _) = fixture();
    ch(&mut app, 's');
    ch(&mut app, 'j');
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.overlay, Some(Overlay::Languages(_))));
    ch(&mut app, 'j');
    key(&mut app, KeyCode::Enter);
    assert!(matches!(app.overlay, Some(Overlay::Settings(_))));
    assert_eq!(app.language, "ja");
    key(&mut app, KeyCode::Esc);
    assert!(app.overlay.is_none());
}
