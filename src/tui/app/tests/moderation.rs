//! Muting and blocking accounts.

use super::*;

#[test]
fn m_mutes_the_selected_posts_author_and_their_posts_leave_the_lists() {
    let mut app = logged_in();
    let jobs = app.handle_key(key('M'));
    assert!(
        matches!(&jobs[..], [Job::Mute { did, on: true }] if did == "did:plc:alice"),
        "{jobs:?}"
    );
    // Once: a second M before the answer sends nothing.
    assert!(app.handle_key(key('M')).is_empty());
    app.handle_event(Event::Muted {
        did: "did:plc:alice".into(),
        on: true,
        result: Ok(()),
    });
    assert_eq!(
        app.timeline
            .items
            .iter()
            .map(|p| p.uri.as_str())
            .collect::<Vec<_>>(),
        ["at://b/p/2"]
    );
    // The next M acts on the post now selected, someone else's: the status
    // says where the mute is undone instead.
    let status = &app.status.as_ref().unwrap().text;
    assert!(status.contains("muted"), "{status}");
    assert!(status.contains("profile"), "{status}");
    assert!(!status.contains("M again"), "{status}");
}

// An unmute or an unblock brings the account's posts back: the lists they
// were taken out of are loaded again.
#[test]
fn an_unmute_or_an_unblock_loads_the_timeline_again() {
    let mut app = logged_in();
    let jobs = app.handle_event(Event::Muted {
        did: "did:plc:alice".into(),
        on: false,
        result: Ok(()),
    });
    assert!(matches!(&jobs[..], [Job::Timeline]), "{jobs:?}");
    let jobs = app.handle_event(Event::Unblocked {
        did: "did:plc:alice".into(),
        result: Ok(()),
    });
    assert!(matches!(&jobs[..], [Job::Timeline]), "{jobs:?}");
    let status = &app.status.as_ref().unwrap().text;
    assert!(status.contains("unblocked"), "{status}");
}

#[test]
fn m_on_a_muted_accounts_profile_unmutes_and_says_so_there() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({
            "did": "did:plc:alice", "handle": "alice.test", "displayName": "家族👨\u{200d}👩\u{200d}👧 Alice",
            "viewer": {"muted": true}
        }))
        .unwrap(),
        Vec::new().into(),
    ))));
    let jobs = app.handle_key(key('M'));
    assert!(
        matches!(&jobs[..], [Job::Mute { did, on: false }] if did == "did:plc:alice"),
        "{jobs:?}"
    );
    app.handle_event(Event::Muted {
        did: "did:plc:alice".into(),
        on: false,
        result: Ok(()),
    });
    assert!(!app.profile.profile.as_ref().unwrap().muted());
}

#[test]
fn b_blocks_only_after_a_y_and_b_again_unblocks_at_once() {
    let mut app = logged_in();
    assert!(app.handle_key(key('B')).is_empty());
    assert!(
        app.status
            .as_ref()
            .unwrap()
            .text
            .contains("press y to block @did:plc:alice.test")
    );
    // Any other key calls it off, and does nothing else.
    assert!(app.handle_key(key('j')).is_empty());
    assert_eq!(app.timeline.selected, 0, "j was the answer, not a move");
    assert!(app.status.as_ref().unwrap().text.contains("not blocked"));
    app.handle_key(key('B'));
    let jobs = app.handle_key(key('y'));
    assert!(
        matches!(&jobs[..], [Job::Block { did }] if did == "did:plc:alice"),
        "{jobs:?}"
    );
    // Asked again while the block is on its way: nothing is sent twice.
    app.handle_key(key('B'));
    assert!(app.handle_key(key('y')).is_empty());
    app.handle_event(Event::Blocked {
        did: "did:plc:alice".into(),
        result: Ok("at://did:plc:me/app.bsky.graph.block/b1".into()),
    });
    assert_eq!(app.timeline.items.len(), 1);
    assert_eq!(app.timeline.items[0].author.did, "did:plc:bob");
    // On their profile, B unblocks with the record it made, without a y.
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({
            "did": "did:plc:alice", "handle": "alice.test",
            "viewer": {"blocking": "at://did:plc:me/app.bsky.graph.block/b1"}
        }))
        .unwrap(),
        Vec::new().into(),
    ))));
    let jobs = app.handle_key(key('B'));
    assert!(
        matches!(&jobs[..], [Job::Unblock { did, block_uri }]
            if did == "did:plc:alice" && block_uri == "at://did:plc:me/app.bsky.graph.block/b1"),
        "{jobs:?}"
    );
}

#[test]
fn you_cannot_mute_or_block_yourself() {
    let mut app = logged_in();
    app.handle_event(Event::Timeline(Ok(vec![post(
        "at://did:plc:me/app.bsky.feed.post/1",
        "did:plc:me",
        false,
    )]
    .into())));
    assert!(app.handle_key(key('M')).is_empty());
    assert!(app.status.as_ref().unwrap().text.contains("mute yourself"));
    assert!(app.handle_key(key('B')).is_empty());
    assert!(!matches!(app.confirm, Some(Confirm::Block(_))));
}

// A reload asked for before the mute was confirmed still carries the
// muted account's posts; they are taken out of it too.
#[test]
fn a_page_asked_for_before_a_mute_comes_without_the_muted_posts() {
    let mut app = logged_in();
    let reload = press(&mut app, key('R'))[0];
    let mute = press(&mut app, key('M'))[0];
    app.handle_answer(
        mute,
        Event::Muted {
            did: "did:plc:alice".into(),
            on: true,
            result: Ok(()),
        },
    );
    app.handle_answer(
        reload,
        Event::Timeline(Ok(vec![
            post("at://a/p/1", "did:plc:alice", true),
            post("at://b/p/2", "did:plc:bob", true),
        ]
        .into())),
    );
    assert_eq!(app.timeline.items.len(), 1);
    assert_eq!(app.timeline.items[0].author.did, "did:plc:bob");
}

// A question asked before the session expired is not answered by the
// first key after logging in again: not B's, not the column's x, not the
// account list's x.
#[test]
fn questions_asked_before_the_session_expired_are_called_off() {
    let mut app = logged_in();
    app.handle_key(key('B'));
    assert!(matches!(app.confirm, Some(Confirm::Block(_))));
    for asked in [
        None,
        Some(Confirm::RemoveColumn(1)),
        Some(Confirm::Logout("did:plc:work".into())),
    ] {
        if asked.is_some() {
            app.confirm = asked;
        }
        expire(&mut app);
        app.handle_event(Event::LoggedIn(Ok(session())));
        assert!(app.handle_key(key('y')).is_empty());
        assert_eq!(app.confirm, None);
    }
}

// An account muted by one of your mute lists is muted, but not by you:
// M adds your own mute rather than sending an unmute that changes nothing.
#[test]
fn m_on_an_account_muted_by_a_list_mutes_it_yourself() {
    let mut app = logged_in();
    app.handle_key(key('5'));
    app.open_profile(Some("did:plc:alice".into()));
    app.handle_event(Event::Profile(Ok((
        serde_json::from_value(json!({
            "did": "did:plc:alice", "handle": "alice.test",
            "viewer": {"muted": true, "mutedByList": {"uri": "at://did:plc:me/app.bsky.graph.list/l1", "name": "Spam 🚫"}}
        }))
        .unwrap(),
        Vec::new().into(),
    ))));
    let jobs = app.handle_key(key('M'));
    assert!(
        matches!(&jobs[..], [Job::Mute { did, on: true }] if did == "did:plc:alice"),
        "{jobs:?}"
    );
}

// The count on the Notifications tab goes down with the notifications a
// mute takes out of the list.
#[test]
fn a_mute_takes_its_notifications_out_of_the_unread_count() {
    let mut app = notifications_tab();
    let before = app.unread;
    let did = app.notifications.current().unwrap().n.author.did.clone();
    let theirs = app
        .notifications
        .items
        .iter()
        .filter(|i| i.n.author.did == did && !i.n.is_read)
        .count();
    assert!(
        theirs > 0,
        "the test needs an unread notification of theirs"
    );
    app.handle_event(Event::Muted {
        did: did.clone(),
        on: true,
        result: Ok(()),
    });
    assert_eq!(app.unread, before - theirs);
}

// A question lasts as long as its prompt: once the prompt has gone, a y
// pressed later answers nothing, and q quits rather than calling it off.
#[test]
fn a_question_ends_with_its_prompt() {
    let mut app = logged_in();
    app.handle_key(key('B'));
    assert!(matches!(app.confirm, Some(Confirm::Block(_))));
    app.expire_status(Instant::now() + STATUS_TTL + Duration::from_secs(1));
    assert!(!matches!(app.confirm, Some(Confirm::Block(_))));
    assert!(app.handle_key(key('y')).is_empty());
    // Another message in the prompt's place ends it too.
    app.handle_key(key('D'));
    app.handle_event(Event::Liked {
        post_uri: "at://a/p/1".into(),
        result: Ok("at://did:plc:me/app.bsky.feed.like/l".into()),
    });
    app.expire_status(Instant::now());
    app.handle_key(key('q'));
    assert!(app.quit);
}
