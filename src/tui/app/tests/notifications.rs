use super::*;

#[test]
fn notifications_load_once_and_are_marked_seen() {
    let mut app = notifications_tab();
    app.handle_event(Event::Seen(Ok(())));
    assert_eq!(app.unread, 0);
    assert!(app.notifications.items.iter().all(|i| i.n.is_read));
    // The markers stay for this visit.
    assert!(app.notifications.items[0].fresh);
    // Coming back does not fetch again; R does.
    app.handle_key(key('1'));
    assert!(app.handle_key(key('4')).is_empty());
    assert!(matches!(
        &app.handle_key(key('R'))[..],
        [Job::Notifications]
    ));
}

#[test]
fn nothing_unread_sends_no_update_seen() {
    let mut app = logged_in();
    app.handle_key(key('4'));
    let jobs = app.handle_event(Event::Notifications {
        seen_at: "t".into(),
        result: Ok(vec![notif("follow", "at://f", true, None, None)].into()),
    });
    assert!(jobs.is_empty());
}

#[test]
fn a_reply_notification_can_be_answered_and_liked() {
    let mut app = notifications_tab();
    let jobs = app.handle_key(key('r'));
    assert!(jobs.is_empty());
    let Some(Overlay::Compose(c)) = &app.overlay else {
        panic!("no composer")
    };
    assert_eq!(c.reply.as_ref().unwrap().0.parent.uri, "at://reply/1");
    app.handle_key(code(KeyCode::Esc));
    let jobs = app.handle_key(key('l'));
    assert!(matches!(&jobs[..], [Job::Like { subject }] if subject.uri == "at://reply/1"));
}

#[test]
fn a_like_notification_opens_the_liked_post_and_its_author() {
    let mut app = notifications_tab();
    app.handle_key(key('j'));
    // Nothing to like or answer on a like: it is not a post.
    assert!(app.handle_key(key('l')).is_empty());
    let jobs = app.handle_key(key('v'));
    assert!(matches!(&jobs[..], [Job::Thread(u)] if u == "at://me/post"));
    app.handle_key(code(KeyCode::Esc));
    let jobs = app.handle_key(code(KeyCode::Enter));
    assert!(matches!(&jobs[..], [Job::OpenProfile(a)] if a == "did:plc:like"));
    assert_eq!(app.profile.came_from, Some(Tab::Notifications));
    app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.tab, Tab::Notifications);
}

#[test]
fn notifications_page_like_every_other_list() {
    let mut app = notifications_tab();
    // Three items: the first move is already near the end.
    let jobs = app.handle_key(key('j'));
    assert!(
        matches!(&jobs[..], [Job::More { feed: Feed::Notifications, cursor }] if cursor == "n1")
    );
    app.handle_event(Event::More {
        feed: Feed::Notifications,
        cursor: "n1".into(),
        result: Ok(MorePage::Notifications(Page {
            items: vec![notif("mention", "at://m/1", true, None, None)],
            cursor: None,
        })),
    });
    assert_eq!(app.notifications.items.len(), 4);
    assert_eq!(app.notifications.cursor, None);
}

#[test]
fn a_background_failure_waits_on_its_tab() {
    let mut app = logged_in();
    app.handle_event(Event::Notifications {
        seen_at: "t".into(),
        result: Err(Error::api("listNotifications failed: boom")),
    });
    assert!(app.status.is_none(), "the timeline is not interrupted");
    assert!(app.notifications.error.is_some());
    // An expired session is still reported at once.
    app.handle_event(Event::Notifications {
        seen_at: "t".into(),
        result: Err(Error::api(
            "com.atproto.server.refreshSession failed: ExpiredToken",
        )),
    });
    assert!(app.login.is_some());
}

#[test]
fn a_sent_post_reloads_the_timeline_so_it_shows() {
    let mut app = logged_in();
    app.handle_key(key('n'));
    type_str(&mut app, "hello");
    app.handle_key(ctrl('s'));
    let jobs = app.handle_event(Event::Posted {
        reply_to: None,
        result: Ok(()),
    });
    assert!(matches!(&jobs[..], [Job::Timeline]), "{jobs:?}");
    assert!(app.overlay.is_none());
}

// New notifications that came while a profile opened from the tab was
// shown are marked seen on the way back with Esc, as on arriving with 3.
#[test]
fn notifications_that_came_meanwhile_are_marked_seen_on_the_way_back() {
    let mut app = notifications_tab();
    app.handle_event(Event::Seen(Ok(())));
    app.handle_key(key('R'));
    app.handle_key(code(KeyCode::Enter));
    assert_eq!(app.tab, Tab::Profile);
    let jobs = app.handle_event(Event::Notifications {
        seen_at: "2026-09-22T02:00:00.000Z".into(),
        result: Ok(Page {
            items: vec![notif(
                "reply",
                "at://reply/2",
                false,
                Some(post("at://reply/2", "did:plc:reply", false)),
                None,
            )],
            cursor: None,
        }),
    });
    assert!(jobs.is_empty(), "not seen while elsewhere: {jobs:?}");
    let jobs = app.handle_key(code(KeyCode::Esc));
    assert_eq!(app.tab, Tab::Notifications);
    assert!(
        matches!(&jobs[..], [Job::UpdateSeen(at)] if at == "2026-09-22T02:00:00.000Z"),
        "{jobs:?}"
    );
}

// On a like of your post, v, space, o and c act on the post liked; the
// actions list offers them, and not like or reply, which have nothing to
// act on there.
#[test]
fn the_actions_list_of_a_like_offers_what_acts_on_the_post_liked() {
    let mut app = notifications_tab();
    app.handle_key(key('j'));
    let keys: Vec<&str> = crate::tui::keys::actions(&app)
        .iter()
        .map(|(k, _)| *k)
        .collect();
    for k in ["v", "space", "o", "c"] {
        assert!(keys.contains(&k), "{k} missing: {keys:?}");
    }
    assert!(!keys.contains(&"l"), "{keys:?}");
    assert!(!keys.contains(&"r"), "{keys:?}");
}

// Notifications a failed mark could not mark seen are marked on the next
// visit to the tab.
#[test]
fn a_failed_mark_as_seen_is_tried_again() {
    let mut app = logged_in();
    app.handle_event(Event::Notifications {
        seen_at: "t1".into(),
        result: Ok(vec![notif("reply", "at://r/1", false, None, None)].into()),
    });
    assert_eq!(app.unread, 1);
    let jobs = app.handle_key(key('4'));
    assert!(matches!(&jobs[..], [Job::UpdateSeen(_)]), "{jobs:?}");
    app.handle_event(Event::Seen(Err(Error::api("boom"))));
    app.handle_key(key('1'));
    let jobs = app.handle_key(key('4'));
    assert!(
        app.unread == 0 || matches!(&jobs[..], [Job::UpdateSeen(_)]),
        "unread {} and back on the tab: {jobs:?}",
        app.unread
    );
}

// H4: two first pages of the conversations out (the tab, then R); the
// older one arriving last replaces the newer.
