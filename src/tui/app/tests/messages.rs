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
    app.handle_key(key('4'));
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
    assert!(
        matches!(&jobs[..], [Job::Messages { convo_id, .. }] if convo_id == "new"),
        "{jobs:?}"
    );
    assert_eq!(app.chat.open.as_ref().unwrap().convo.id, "new");
}

#[test]
fn an_app_password_without_access_to_messages_is_explained() {
    let mut app = logged_in();
    app.handle_key(key('6'));
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
    // Sent again, then the conversation closed and opened again, and a new
    // draft begun before the answer came.
    assert_eq!(app.handle_key(code(KeyCode::Enter)).len(), 1);
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Esc));
    app.handle_key(code(KeyCode::Enter));
    app.handle_key(key('i'));
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
    app.handle_key(key('6'));
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
