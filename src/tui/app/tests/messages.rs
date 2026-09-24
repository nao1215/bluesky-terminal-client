use super::*;

#[test]
fn a_conversation_is_marked_read_when_it_is_opened_not_when_it_is_listed() {
    let mut app = chat_tab();
    assert_eq!(app.chat.unread(), 2);
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(&jobs[..], [Job::Messages { convo_id, cursor: None }, Job::ReadConvo { convo_id: r }] if convo_id == "a" && r == "a"),
        "{jobs:?}"
    );
    app.handle_event(Event::ConvoRead {
        convo_id: "a".into(),
        result: Ok(()),
    });
    assert_eq!(app.chat.unread(), 0);
    // Nothing unread: nothing marked.
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('j'));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::Messages { .. }]), "{jobs:?}");
}

#[test]
fn a_message_is_sent_once_and_a_failed_one_keeps_its_text() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(vec![a_message("m2", "second"), a_message("m1", "first")].into()),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert_eq!(
        o.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["m1", "m2"]
    );
    // Keys go to the box once i is pressed: q is a letter, not quit.
    app.handle_key(key('i'));
    type_str(&mut app, "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}");
    assert!(!app.quit);
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        matches!(&jobs[..], [Job::SendMessage { convo_id, text }] if convo_id == "a" && text == "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}"),
        "{jobs:?}"
    );
    assert!(app.handle_key(code(KeyCode::Enter)).is_empty(), "not twice");
    app.handle_event(Event::MessageSent {
        convo_id: "a".into(),
        text: "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}".into(),
        result: Err(Error::api("chat.bsky.convo.sendMessage failed: HTTP 502")),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert!(!o.sending);
    assert_eq!(o.input.text(), "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert_eq!(jobs.len(), 1);
    app.handle_event(Event::MessageSent {
        convo_id: "a".into(),
        text: "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}".into(),
        result: Ok(ChatMessage {
            sender: "did:plc:me".into(),
            ..a_message("m3", "quiet 👍🏽 🇯🇵 1️⃣ ❤️ e\u{301}")
        }),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert_eq!(o.input.text(), "");
    assert_eq!(o.messages.last().unwrap().id, "m3");
    // Esc stops writing, Esc again goes back to the list.
    app.handle_key(code(KeyCode::Esc));
    assert!(app.chat.open.as_ref().is_some_and(|o| !o.typing));
    app.handle_key(code(KeyCode::Esc));
    assert!(app.chat.open.is_none());
}

#[test]
fn a_page_for_a_conversation_left_is_dropped() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('j'));
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(vec![a_message("x", "from a")].into()),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert_eq!(o.convo.id, "b");
    assert!(o.messages.is_empty());
}

#[test]
fn the_chat_tab_is_read_again_only_while_it_is_shown() {
    let mut app = chat_tab();
    let now = Instant::now();
    assert!(app.poll_chat(now).is_empty(), "just read");
    let later = now + chat::POLL_EVERY + Duration::from_secs(1);
    let jobs = app.poll_chat(later);
    assert!(
        matches!(&jobs[..], [Job::Convos { cursor: None }]),
        "{jobs:?}"
    );
    app.handle_key(code(KeyCode::Enter));
    let jobs = app.poll_chat(later + chat::POLL_EVERY + Duration::from_secs(1));
    assert!(
        matches!(&jobs[..], [Job::Messages { cursor: None, .. }]),
        "{jobs:?}"
    );
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('1'));
    assert!(app.poll_chat(later + chat::POLL_EVERY * 10).is_empty());
}

#[test]
fn m_on_a_profile_opens_the_conversation_with_them() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    // Your own profile has nobody to message.
    assert!(app.handle_key(key('m')).is_empty());
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap(),
        Vec::new().into(),
    ))));
    let jobs = app.handle_key(key('m'));
    assert!(
        matches!(&jobs[..], [Job::ConvoFor { did }] if did == "did:plc:alice"),
        "{jobs:?}"
    );
    let jobs = app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("new", 0)),
    });
    assert_eq!(app.tab, Tab::Chat);
    // Its messages, and the list, never read yet, for Esc to go back to.
    assert!(
        matches!(&jobs[..], [Job::Messages { convo_id, .. }, Job::Convos { cursor: None }] if convo_id == "new"),
        "{jobs:?}"
    );
    assert_eq!(app.chat.open.as_ref().unwrap().convo.id, "new");
}

#[test]
fn an_app_password_without_access_to_messages_is_explained() {
    let mut app = logged_in();
    app.handle_key(key('2'));
    app.handle_event(Event::Convos {
        cursor: None,
        result: Err(Error::api(
            "chat.bsky.convo.listConvos failed: AuthRequired: Bad token scope",
        )),
    });
    assert!(
        app.chat
            .refused
            .as_deref()
            .unwrap()
            .contains("Allow access to your direct messages")
    );
    assert!(
        app.poll_chat(Instant::now() + chat::POLL_EVERY * 2)
            .is_empty()
    );
}

// What is typed while a message is on its way stays: the answer takes only
// the text that was sent out of the box, and nothing of a conversation
// opened again meanwhile.
#[test]
fn what_is_typed_while_a_message_is_sent_is_kept() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('i'));
    type_str(&mut app, "hello 👨\u{200d}👩\u{200d}👧");
    assert_eq!(app.handle_key(code(KeyCode::Enter)).len(), 1);
    type_str(&mut app, " and also 🇯🇵");
    app.handle_event(Event::MessageSent {
        convo_id: "a".into(),
        text: "hello 👨\u{200d}👩\u{200d}👧".into(),
        result: Ok(ChatMessage {
            sender: "did:plc:me".into(),
            ..a_message("m9", "hello 👨\u{200d}👩\u{200d}👧")
        }),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert_eq!(o.input.text(), "and also 🇯🇵");
    // Sent again, then the box cleared and a new draft begun before the
    // answer came.
    assert_eq!(app.handle_key(code(KeyCode::Enter)).len(), 1);
    app.handle_key(ctrl('u'));
    type_str(&mut app, "a new draft");
    app.handle_event(Event::MessageSent {
        convo_id: "a".into(),
        text: "and also 🇯🇵".into(),
        result: Ok(ChatMessage {
            sender: "did:plc:me".into(),
            ..a_message("m10", "and also 🇯🇵")
        }),
    });
    assert_eq!(app.chat.open.as_ref().unwrap().input.text(), "a new draft");
}

// The list read again every 15 seconds brings its first page only. The
// conversations loaded further down stay, and so does the one selected:
// Enter opens it, not the first one, which it would also mark read.
#[test]
fn reading_the_list_again_keeps_the_pages_loaded_and_the_selection() {
    let mut app = logged_in();
    app.handle_key(key('2'));
    let first: Vec<Convo> = (0..50).map(|i| a_convo(&format!("c{i}"), 1)).collect();
    app.handle_event(Event::Convos {
        cursor: None,
        result: Ok(Page {
            items: first.clone(),
            cursor: Some("p2".into()),
        }),
    });
    let more: Vec<Job> = (0..49).flat_map(|_| app.handle_key(key('j'))).collect();
    assert!(
        matches!(&more[..], [Job::Convos { cursor: Some(c) }] if c == "p2"),
        "{more:?}"
    );
    app.handle_event(Event::Convos {
        cursor: Some("p2".into()),
        result: Ok(Page {
            items: (50..60).map(|i| a_convo(&format!("c{i}"), 1)).collect(),
            cursor: Some("p3".into()),
        }),
    });
    for _ in 0..6 {
        app.handle_key(key('j'));
    }
    assert_eq!(app.chat.convos.current().unwrap().id, "c55");
    let later = Instant::now() + chat::POLL_EVERY * 2;
    let jobs = app.poll_chat(later);
    assert!(
        matches!(&jobs[..], [Job::Convos { cursor: None }]),
        "{jobs:?}"
    );
    // A new conversation comes first in the fresh page.
    let mut fresh = vec![a_convo("new", 1)];
    fresh.extend(first.into_iter().take(49));
    app.handle_event(Event::Convos {
        cursor: None,
        result: Ok(Page {
            items: fresh,
            cursor: Some("p2b".into()),
        }),
    });
    assert_eq!(app.chat.convos.items.len(), 61);
    assert_eq!(app.chat.convos.items[0].id, "new");
    assert_eq!(app.chat.convos.current().unwrap().id, "c55");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        jobs.iter()
            .all(|j| !matches!(j, Job::ReadConvo { convo_id } if convo_id != "c55")),
        "{jobs:?}"
    );
    assert_eq!(app.chat.open.as_ref().unwrap().convo.id, "c55");
}

// The conversation m asked for opens only if its profile is still what is
// looked at: an answer that comes after a move to another tab, or to a
// thread, neither takes the screen nor marks the conversation read.
#[test]
fn a_late_answer_to_m_does_not_take_the_screen() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap(),
        Vec::new().into(),
    ))));
    assert_eq!(app.handle_key(key('m')).len(), 1);
    app.handle_key(key('1'));
    let jobs = app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("new", 2)),
    });
    assert!(jobs.is_empty(), "{jobs:?}");
    assert_eq!(app.tab, Tab::Timeline);
    assert!(app.chat.open.is_none());
    assert!(
        app.status.as_ref().unwrap().text.contains("Chat tab"),
        "{:?}",
        app.status
    );
}

// A conversation that comes from m on a profile goes at the top of the
// list. The one selected stays selected: Enter opens it, not the one above.
#[test]
fn a_conversation_added_at_the_top_keeps_the_selection() {
    let mut app = chat_tab();
    app.handle_key(key('j'));
    assert_eq!(app.chat.convos.current().unwrap().id, "b");
    app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("new", 0)),
    });
    assert_eq!(app.chat.convos.items[0].id, "new");
    assert_eq!(app.chat.convos.current().unwrap().id, "b");
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(
        !jobs
            .iter()
            .any(|j| matches!(j, Job::ReadConvo { convo_id } if convo_id == "a")),
        "{jobs:?}"
    );
}

// A conversation cannot be closed while its message is on its way: opened
// again, the answer to that message would let the next Enter send a second
// one before the first is known to have gone.
#[test]
fn a_conversation_is_not_closed_while_its_message_is_sent() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('i'));
    type_str(&mut app, "one 👍🏽");
    assert_eq!(app.handle_key(code(KeyCode::Enter)).len(), 1);
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Esc));
    let o = app.chat.open.as_ref().expect("closed while sending");
    assert!(o.sending);
    assert!(app.status.as_ref().unwrap().text.contains("on its way"));
}

// m on the profile of someone whose conversation is open already brings
// that conversation back as it was, the draft with it.
#[test]
fn m_for_the_conversation_already_open_keeps_its_draft() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('i'));
    type_str(&mut app, "draft 🇯🇵");
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap(),
        Vec::new().into(),
    ))));
    assert_eq!(app.handle_key(key('m')).len(), 1);
    app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("a", 0)),
    });
    assert_eq!(app.tab, Tab::Chat);
    assert_eq!(app.chat.open.as_ref().unwrap().input.text(), "draft 🇯🇵");
}

// B asked on a profile, and the answer to m took the screen to the Chat tab
// before y: the question is answered by the next key there too, and Esc or
// any other key calls it off, not a y pressed later.
#[test]
fn a_question_takes_the_next_key_on_the_chat_tab_too() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap(),
        Vec::new().into(),
    ))));
    assert_eq!(app.handle_key(key('m')).len(), 1);
    app.handle_key(key('B'));
    app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("new", 0)),
    });
    app.handle_key(code(KeyCode::Esc));
    assert!(!matches!(app.confirm, Some(Confirm::Block(_))));
    assert!(app.handle_key(key('y')).is_empty());
}

// A message that comes into the conversation being read is read: it is
// marked so, and does not come back as unread in the list or elsewhere.
#[test]
fn a_message_that_comes_while_the_conversation_is_open_is_marked_read() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::ConvoRead {
        convo_id: "a".into(),
        result: Ok(()),
    });
    let first = app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(vec![a_message("m1", "first")].into()),
    });
    assert!(
        first.is_empty(),
        "opening marked it read already: {first:?}"
    );
    let jobs = app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(vec![a_message("m2", "new 🇯🇵"), a_message("m1", "first")].into()),
    });
    assert!(
        matches!(&jobs[..], [Job::ReadConvo { convo_id }] if convo_id == "a"),
        "{jobs:?}"
    );
    // Nothing new: nothing to mark.
    let again = app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(vec![a_message("m2", "new 🇯🇵"), a_message("m1", "first")].into()),
    });
    assert!(again.is_empty(), "{again:?}");
}

// A conversation opened with m before the Chat tab was ever shown: the list
// of conversations is asked for too, so Esc shows them all, not that one.
#[test]
fn m_before_the_chat_tab_was_shown_loads_the_list_too() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({"did": "did:plc:alice", "handle": "alice.test"})).unwrap(),
        Vec::new().into(),
    ))));
    app.handle_key(key('m'));
    let jobs = app.handle_event(Event::ConvoFor {
        did: "did:plc:alice".into(),
        result: Ok(a_convo("new", 0)),
    });
    assert!(
        jobs.iter()
            .any(|j| matches!(j, Job::Convos { cursor: None })),
        "{jobs:?}"
    );
    assert!(app.chat.convos.loading);
}

// A conversation read while the list was on its way stays read when the
// list, asked for before, comes.
#[test]
fn a_list_asked_for_before_a_read_does_not_bring_back_its_unread_count() {
    let mut app = chat_tab();
    let later = Instant::now() + chat::POLL_EVERY + Duration::from_secs(1);
    let polled = app.poll_chat(later);
    assert!(matches!(&polled[..], [Job::Convos { cursor: None }]));
    let s_list = app.stamp(&polled[0]);
    let s: Vec<u64> = press(&mut app, code(KeyCode::Enter));
    // ReadConvo answered first.
    app.handle_answer(
        s[1],
        Event::ConvoRead {
            convo_id: "a".into(),
            result: Ok(()),
        },
    );
    assert_eq!(app.chat.unread(), 0);
    // The list read before arrives late.
    app.handle_answer(
        s_list,
        Event::Convos {
            cursor: None,
            result: Ok(vec![a_convo("a", 2), a_convo("b", 0)].into()),
        },
    );
    assert_eq!(app.chat.unread(), 0, "conversation a was read");
}

// A first page that failed, asked for again, still says where the older
// messages are.
#[test]
fn a_retry_after_a_failed_first_page_keeps_the_older_messages_reachable() {
    let mut app = chat_tab();
    app.handle_key(code(KeyCode::Enter));
    app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Err(Error::api("chat.bsky.convo.getMessages failed: HTTP 502")),
    });
    let jobs = app.handle_key(key('R'));
    assert!(
        matches!(&jobs[..], [Job::Messages { cursor: None, .. }]),
        "{jobs:?}"
    );
    app.handle_event(Event::Messages {
        convo_id: "a".into(),
        cursor: None,
        result: Ok(Page {
            items: vec![a_message("m2", "b"), a_message("m1", "a")],
            cursor: Some("older".into()),
        }),
    });
    let o = app.chat.open.as_ref().unwrap();
    assert_eq!(o.messages.len(), 2);
    assert_eq!(
        o.older.as_deref(),
        Some("older"),
        "the earlier messages can still be asked for"
    );
}
