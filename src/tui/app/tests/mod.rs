use super::*;
use crate::error::Error;
use serde_json::json;

mod accounts;
mod compose;
mod deck;
mod feeds_and_actions;
mod lists;
mod login_and_writes;
mod messages;
mod moderation;
mod navigation;
mod notifications;
mod settings;
mod sync;
mod threads_and_viewer;

fn session() -> Session {
    Session {
        service: "https://pds.test".into(),
        did: "did:plc:me".into(),
        handle: "me.test".into(),
        access_jwt: "a".into(),
        refresh_jwt: "r".into(),
    }
}

fn post(uri: &str, author: &str, following: bool) -> Post {
    let mut v = json!({
        "uri": uri, "cid": format!("cid-{uri}"),
        "author": {"did": author, "handle": format!("{author}.test")},
        "record": {"text": format!("text of {uri}")},
        "likeCount": 2,
    });
    if following {
        v["author"]["viewer"] =
            json!({"following": format!("at://did:plc:me/app.bsky.graph.follow/{author}")});
    }
    serde_json::from_value(v).unwrap()
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn code(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn logged_in() -> App {
    let (mut app, jobs) = App::new(Some(session()), "https://bsky.social");
    assert!(matches!(jobs[..], [Job::Timeline, Job::Notifications]));
    app.handle_event(Event::Timeline(Ok(vec![
        post("at://a/p/1", "did:plc:alice", true),
        post("at://b/p/2", "did:plc:bob", true),
    ]
    .into())));
    app
}

fn type_str(app: &mut App, s: &str) {
    for c in s.chars() {
        app.handle_key(key(c));
    }
}

/// The post as a reload fetched after the server took the like or repost.
fn reloaded(liked: bool, reposted: bool) -> Post {
    let mut p = post("at://a/p/1", "did:plc:alice", true);
    p.viewer = Some(crate::api::types::PostViewer {
        like: liked.then(|| "at://did:plc:me/app.bsky.feed.like/x".into()),
        repost: reposted.then(|| "at://did:plc:me/app.bsky.feed.repost/y".into()),
    });
    p.like_count = if liked { 3 } else { 2 };
    p.repost_count = u64::from(reposted);
    p
}

/// Press a key and number its jobs the way the event loop does as it
/// sends them.
fn press(app: &mut App, k: KeyEvent) -> Vec<u64> {
    let jobs = app.handle_key(k);
    jobs.iter().map(|j| app.stamp(j)).collect()
}

fn feed_info(name: &str) -> crate::api::types::FeedInfo {
    crate::api::types::FeedInfo {
        uri: format!("at://did:plc:f/app.bsky.feed.generator/{name}"),
        name: name.to_string(),
    }
}

/// On the Timeline tab with the columns of `sources`, each answered with
/// `posts` (notifications columns with none).
fn columns_with(sources: &[columns::Source], posts: Vec<Post>) -> App {
    let mut app = logged_in();
    let mut settings = Settings::default();
    settings
        .columns
        .insert("did:plc:me".into(), sources.to_vec());
    app.apply_settings(settings, ColorDepth::TrueColor, None);
    let jobs = app.handle_key(key('1'));
    assert_eq!(jobs.len(), sources.len(), "{jobs:?}");
    for job in jobs {
        let Job::Column {
            id,
            generation,
            feed,
            cursor,
        } = job
        else {
            panic!("{job:?}")
        };
        assert!(cursor.is_none());
        let page = match feed {
            Feed::Notifications => MorePage::Notifications(Vec::new().into()),
            _ => MorePage::Posts(posts.clone().into()),
        };
        app.handle_event(Event::Column {
            id,
            generation,
            cursor: None,
            result: Ok(page),
        });
    }
    app
}

fn a_convo(id: &str, unread: u64) -> Convo {
    serde_json::from_value(json!({
        "id": id, "rev": "r",
        "members": [
            {"did": "did:plc:me", "handle": "me.test"},
            {"did": "did:plc:alice", "handle": "alice.test", "displayName": "家族👨\u{200d}👩\u{200d}👧 Alice"}
        ],
        "muted": false, "unreadCount": unread
    }))
    .unwrap()
}

fn a_message(id: &str, text: &str) -> ChatMessage {
    ChatMessage {
        id: id.into(),
        text: text.into(),
        sender: "did:plc:alice".into(),
        sent_at: "2026-09-22T00:00:00Z".into(),
        ..ChatMessage::default()
    }
}

/// On the Chat tab with conversations `a` (2 unread) and `b` (none).
fn chat_tab() -> App {
    let mut app = logged_in();
    let jobs = app.handle_key(key('2'));
    assert!(
        matches!(&jobs[..], [Job::Convos { cursor: None }]),
        "{jobs:?}"
    );
    app.handle_event(Event::Convos {
        cursor: None,
        result: Ok(vec![a_convo("a", 2), a_convo("b", 0)].into()),
    });
    app
}

fn work() -> Session {
    Session {
        did: "did:plc:work".into(),
        handle: "work.example".into(),
        ..session()
    }
}

/// Logged in as two accounts, using the first.
fn two_accounts() -> App {
    let mut app = logged_in();
    app.accounts = vec![Account::from(&session()), Account::from(&work())];
    app
}

fn expire(app: &mut App) {
    app.handle_event(Event::Timeline(Err(Error::api(
        "com.atproto.server.refreshSession failed: ExpiredToken: Token has expired",
    ))));
    assert!(app.login.is_some());
}

/// The settings screen, on the row named `name`.
fn settings_on(app: &mut App, name: &str) {
    app.handle_key(key('5'));
    app.handle_key(key('s'));
    let at = app
        .settings_rows()
        .iter()
        .position(|r| r.name == name)
        .expect("a row of that name");
    for _ in 0..at {
        app.handle_key(key('j'));
    }
    assert!(
        matches!(app.overlay, Some(Overlay::Settings { selected, .. }) if selected == at),
        "{:?}",
        app.overlay
    );
}

/// A picture post, open in the viewer.
fn viewing_a_picture(app: &mut App) {
    app.handle_key(key('1'));
    app.handle_event(Event::Timeline(Ok(vec![with_pictures(
        "at://did:plc:alice/app.bsky.feed.post/p1",
        json!({"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": ""}]}),
    )]
    .into())));
    app.handle_key(key(' '));
    assert!(app.viewer_open());
}

fn page(posts: Vec<Post>, cursor: Option<&str>) -> Page<Post> {
    Page {
        items: posts,
        cursor: cursor.map(str::to_string),
    }
}

fn timeline_with(n: usize, cursor: Option<&str>) -> App {
    let (mut app, _) = App::new(Some(session()), "x");
    let posts = (0..n)
        .map(|i| post(&format!("at://p/{i}"), "did:plc:a", true))
        .collect();
    app.handle_event(Event::Timeline(Ok(page(posts, cursor))));
    app
}

fn thread_json(focus: &str, replies: &[&str]) -> crate::api::types::ThreadNode {
    let node = |uri: &str| {
        json!({"$type": "app.bsky.feed.defs#threadViewPost",
               "post": {"uri": uri, "cid": format!("cid-{uri}"), "author": {"did": "did:plc:z", "handle": "z.test"}, "record": {"text": uri}},
               "replies": []})
    };
    let mut v = node(focus);
    v["parent"] = node("at://parent");
    v["replies"] = json!(replies.iter().map(|r| node(r)).collect::<Vec<_>>());
    serde_json::from_value(v).unwrap()
}

fn with_pictures(uri: &str, embed: serde_json::Value) -> Post {
    let mut p = post(uri, "did:plc:alice", true);
    p.embed = serde_json::from_value(embed).ok();
    p
}

fn notif(
    reason: &str,
    uri: &str,
    read: bool,
    post: Option<Post>,
    subject: Option<Post>,
) -> NotifItem {
    NotifItem {
        n: serde_json::from_value(json!({
            "uri": uri, "cid": "c", "reason": reason, "isRead": read,
            "author": {"did": format!("did:plc:{reason}"), "handle": format!("{reason}.test")},
            "indexedAt": "2026-09-22T00:00:00.000Z"
        }))
        .unwrap(),
        fresh: !read,
        post,
        subject,
    }
}

fn notifications_tab() -> App {
    let mut app = logged_in();
    // Loading since the start: arriving asks for nothing more.
    assert!(app.handle_key(key('4')).is_empty());
    let reply = post("at://reply/1", "did:plc:reply", false);
    let mine = post("at://me/post", "did:plc:me", false);
    let jobs = app.handle_event(Event::Notifications {
        seen_at: "2026-09-22T01:00:00.000Z".into(),
        result: Ok(Page {
            items: vec![
                notif("reply", "at://reply/1", false, Some(reply), None),
                notif("like", "at://like/1", false, None, Some(mine)),
                notif("follow", "at://follow/1", true, None, None),
            ],
            cursor: Some("n1".into()),
        }),
    });
    // Two were unread: they are marked seen, as of the time of the fetch.
    assert!(matches!(&jobs[..], [Job::UpdateSeen(at)] if at == "2026-09-22T01:00:00.000Z"));
    assert_eq!(app.unread, 2);
    app
}

/// A folder with two pictures and a subfolder, for the picture browser.
fn pictures() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    for name in ["a.png", "b.png"] {
        image::RgbImage::from_pixel(4, 3, image::Rgb([1, 2, 3]))
            .save(dir.path().join(name))
            .unwrap();
    }
    dir
}

fn composer(app: &App) -> &Compose {
    match &app.overlay {
        Some(Overlay::Compose(c)) => c,
        other => panic!("no composer: {other:?}"),
    }
}

/// A composer with both pictures of `dir` attached.
fn composer_with_pictures(dir: &tempfile::TempDir) -> App {
    let mut app = logged_in();
    app.browse_from = Some(dir.path().to_path_buf());
    app.handle_key(key('n'));
    app.handle_key(ctrl('o'));
    let b = composer(&app).browser.as_ref().expect("browser");
    assert_eq!(b.room, MAX_POST_IMAGES);
    assert_eq!(b.current().unwrap().name, "sub");
    app.handle_key(key('j'));
    app.handle_key(key(' '));
    app.handle_key(key(' '));
    app.handle_key(code(KeyCode::Enter));
    assert!(composer(&app).browser.is_none());
    assert_eq!(composer(&app).media.len(), 2);
    app
}

fn testdata(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("e2e/atago/testdata")
        .join(name)
}
