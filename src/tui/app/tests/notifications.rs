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
    assert!(app.handle_key(key('3')).is_empty());
    assert!(matches!(
        &app.handle_key(key('R'))[..],
        [Job::Notifications]
    ));
}

#[test]
fn nothing_unread_sends_no_update_seen() {
    let mut app = logged_in();
    app.handle_key(key('3'));
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
