//! Random keys and random, late, reordered answers from a stand-in worker,
//! thousands of steps per seed: the client must not panic, must keep every
//! selection inside its list, must never send more than one write for one
//! key, and must draw at any terminal size.

use super::*;
use crate::api::types::{Profile, ThreadNode};
use crate::config::Session;
use crate::error::Error;
use crate::tui::worker::{Event, Feed, Job, MorePage, Page};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui_image::picker::Picker;
use serde_json::json;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*: a failing seed can be replayed.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

const TEXTS: &[&str] = &[
    "hello",
    "👨‍👩‍👧‍👦 family 🇯🇵",
    "日本語の投稿 #タグ https://example.com",
    "e\u{301}t\u{e9} 1️⃣ ❤️ 👍🏽",
    "",
    "a very long line that keeps going and going so that it has to wrap over several rows of the screen",
];
const AUTHORS: &[&str] = &["did:plc:me", "did:plc:alice", "did:plc:bob"];

fn post(rng: &mut Rng, n: u64) -> Post {
    let author = AUTHORS[rng.below(AUTHORS.len())];
    let mut v = json!({
        "uri": format!("at://{author}/app.bsky.feed.post/p{n}"),
        "cid": format!("c{n}"),
        "author": {"did": author, "handle": format!("{}.test", &author[8..]), "displayName": TEXTS[rng.below(TEXTS.len())]},
        "record": {"text": TEXTS[rng.below(TEXTS.len())], "createdAt": "2026-09-22T00:00:00Z"},
        "likeCount": rng.below(5),
        "indexedAt": "2026-09-22T00:00:00Z",
    });
    if rng.chance(50) {
        v["author"]["viewer"] =
            json!({"following": format!("at://did:plc:me/app.bsky.graph.follow/{n}")});
    }
    if rng.chance(30) {
        v["viewer"] = json!({"like": format!("at://did:plc:me/app.bsky.feed.like/{n}")});
    }
    serde_json::from_value(v).unwrap()
}

fn page(rng: &mut Rng, next_id: &mut u64) -> Page<Post> {
    let items = (0..rng.below(8))
        .map(|_| {
            *next_id += 1;
            post(rng, *next_id)
        })
        .collect();
    Page {
        items,
        cursor: rng.chance(60).then(|| format!("c{}", rng.below(1000))),
    }
}

fn profile(did: &str) -> Profile {
    serde_json::from_value(
        json!({"did": did, "handle": "someone.test", "displayName": "Some 👍🏽 one"}),
    )
    .unwrap()
}

fn fail() -> Error {
    Error::api("the server said no")
}

fn convo(id: &str, unread: u64) -> crate::api::types::Convo {
    serde_json::from_value(json!({
        "id": id, "rev": "r",
        "members": [
            {"did": "did:plc:me", "handle": "me.test"},
            {"did": "did:plc:alice", "handle": "alice.test", "displayName": TEXTS[1]}
        ],
        "lastMessage": {"$type": "chat.bsky.convo.defs#messageView", "id": "l", "rev": "r",
                        "text": TEXTS[2], "sender": {"did": "did:plc:alice"}, "sentAt": "2026-09-22T00:00:00Z"},
        "muted": false, "unreadCount": unread
    }))
    .unwrap()
}

fn message(id: &str) -> crate::api::types::ChatMessage {
    crate::api::types::ChatMessage {
        id: id.into(),
        text: TEXTS[3].into(),
        sender: "did:plc:alice".into(),
        sent_at: "2026-09-22T00:00:00Z".into(),
        ..Default::default()
    }
}

/// Now and then the session expires, which brings the login form back.
fn fail_or_expire(rng: &mut Rng) -> Error {
    if rng.chance(10) {
        Error::api("com.atproto.server.refreshSession failed: ExpiredToken: Token has expired")
    } else {
        fail()
    }
}

/// What the worker would answer to `job`, sometimes with an error.
fn answer(rng: &mut Rng, job: Job, next_id: &mut u64) -> Option<Event> {
    let ok = !rng.chance(15);
    Some(match job {
        // Logging in again, as the same account, mostly works.
        Job::Login { .. } => Event::LoggedIn(if rng.chance(70) {
            Ok(Session {
                service: "https://pds.test".into(),
                did: "did:plc:me".into(),
                handle: "me.test".into(),
                access_jwt: "a2".into(),
                refresh_jwt: "r2".into(),
            })
        } else {
            Err(fail())
        }),
        Job::PinnedFeeds => Event::PinnedFeeds(if ok {
            Ok(["discover", "science"]
                .iter()
                .take(rng.below(3))
                .map(|n| crate::api::types::FeedInfo {
                    uri: format!("at://did:plc:f/app.bsky.feed.generator/{n}"),
                    name: n.to_string(),
                })
                .collect())
        } else {
            Err(fail())
        }),
        Job::Timeline => Event::Timeline(if ok {
            Ok(page(rng, next_id))
        } else {
            Err(fail_or_expire(rng))
        }),
        Job::SearchPosts(query) => Event::SearchPosts {
            query,
            result: if ok {
                Ok(page(rng, next_id))
            } else {
                Err(fail())
            },
        },
        Job::SearchActors(query) => Event::SearchActors {
            query,
            result: if ok {
                Ok(vec![profile("did:plc:alice"), profile("did:plc:bob")].into())
            } else {
                Err(fail())
            },
        },
        Job::OpenProfile(a) => Event::Profile(if ok {
            Ok((profile(&a), page(rng, next_id)))
        } else {
            Err(fail())
        }),
        Job::Thread(uri) => {
            let node: ThreadNode = serde_json::from_value(json!({
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {
                    "uri": uri.clone(), "cid": "c",
                    "author": {"did": "did:plc:bob", "handle": "bob.test"},
                    "record": {"text": TEXTS[rng.below(TEXTS.len())], "createdAt": "2026-09-22T00:00:00Z"},
                },
                "replies": [],
            }))
            .unwrap();
            Event::Thread {
                uri,
                result: if ok { Ok(node) } else { Err(fail()) },
            }
        }
        Job::Notifications => Event::Notifications {
            seen_at: "2026-09-22T00:00:00Z".into(),
            result: if ok {
                Ok(Vec::new().into())
            } else {
                Err(fail())
            },
        },
        Job::UpdateSeen(_) => Event::Seen(Ok(())),
        Job::Column {
            id,
            generation,
            feed,
            cursor,
        } => Event::Column {
            result: if !ok {
                Err(fail())
            } else {
                Ok(match &feed {
                    Feed::Notifications => MorePage::Notifications(Vec::new().into()),
                    _ => MorePage::Posts(page(rng, next_id)),
                })
            },
            id,
            generation,
            cursor,
        },
        Job::Convos { cursor } => Event::Convos {
            cursor,
            result: if ok {
                Ok(Page {
                    items: (0..rng.below(4))
                        .map(|i| convo(&format!("c{i}"), rng.below(3) as u64))
                        .collect(),
                    cursor: rng.chance(50).then(|| format!("k{}", rng.below(9))),
                })
            } else {
                Err(fail())
            },
        },
        Job::Messages { convo_id, cursor } => Event::Messages {
            convo_id,
            cursor,
            result: if ok {
                Ok(Page {
                    items: (0..rng.below(6))
                        .map(|_| {
                            *next_id += 1;
                            message(&format!("m{next_id}"))
                        })
                        .collect(),
                    cursor: rng.chance(50).then(|| format!("k{}", rng.below(9))),
                })
            } else {
                Err(fail())
            },
        },
        Job::SendMessage { convo_id, text } => Event::MessageSent {
            convo_id,
            text,
            result: if ok {
                *next_id += 1;
                Ok(message(&format!("m{next_id}")))
            } else {
                Err(fail())
            },
        },
        Job::ConvoFor { did } => Event::ConvoFor {
            did,
            result: if ok { Ok(convo("cx", 1)) } else { Err(fail()) },
        },
        Job::ReadConvo { convo_id } => Event::ConvoRead {
            convo_id,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::More { feed, cursor } => {
            let result = if !ok {
                Err(fail())
            } else {
                Ok(match &feed {
                    Feed::SearchActors(_) => MorePage::Actors(vec![profile("did:plc:x")].into()),
                    Feed::Notifications => MorePage::Notifications(Vec::new().into()),
                    _ => MorePage::Posts(page(rng, next_id)),
                })
            };
            Event::More {
                feed,
                cursor,
                result,
            }
        }
        Job::Like { subject } => Event::Liked {
            post_uri: subject.uri,
            result: if ok {
                Ok("at://did:plc:me/app.bsky.feed.like/new".into())
            } else {
                Err(fail())
            },
        },
        Job::Unlike { post_uri, .. } => Event::Unliked {
            post_uri,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::Repost { subject } => Event::Reposted {
            post_uri: subject.uri,
            result: if ok {
                Ok("at://did:plc:me/app.bsky.feed.repost/new".into())
            } else {
                Err(fail())
            },
        },
        Job::Unrepost { post_uri, .. } => Event::Unreposted {
            post_uri,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::Follow { did } => Event::Followed {
            did,
            result: if ok {
                Ok("at://did:plc:me/app.bsky.graph.follow/new".into())
            } else {
                Err(fail())
            },
        },
        Job::Unfollow { did, .. } => Event::Unfollowed {
            did,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::Mute { did, on } => Event::Muted {
            did,
            on,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::Block { did } => Event::Blocked {
            did,
            result: if ok {
                Ok("at://did:plc:me/app.bsky.graph.block/new".into())
            } else {
                Err(fail())
            },
        },
        Job::Unblock { did, .. } => Event::Unblocked {
            did,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::Post { reply, .. } => Event::Posted {
            reply_to: reply.map(|r| r.parent.uri),
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::DeletePost { uri } => Event::PostDeleted {
            uri,
            result: if ok { Ok(()) } else { Err(fail()) },
        },
        Job::LoadProfileEditor => Event::ProfileEditor(Err(fail())),
        Job::SaveProfile { .. } => Event::ProfileSaved(if ok { Ok(()) } else { Err(fail()) }),
        Job::Download { .. } => Event::Downloaded(Err(fail())),
        Job::OpenLink { url, .. } => Event::Opened {
            url,
            result: Err(fail()),
        },
    })
}

fn is_write(job: &Job) -> bool {
    matches!(
        job,
        Job::Like { .. }
            | Job::Unlike { .. }
            | Job::Repost { .. }
            | Job::Unrepost { .. }
            | Job::Follow { .. }
            | Job::Unfollow { .. }
            | Job::Mute { .. }
            | Job::Block { .. }
            | Job::Unblock { .. }
            | Job::Post { .. }
            | Job::DeletePost { .. }
            | Job::SaveProfile { .. }
            | Job::SendMessage { .. }
            | Job::ReadConvo { .. }
    )
}

fn key(rng: &mut Rng) -> KeyEvent {
    const CHARS: &[char] = &[
        'j', 'k', 'g', 'G', 'l', 'b', 'f', 'r', 'n', 'v', 'o', '/', 't', 'T', '?', 'R', 'e', 'd',
        'D', '1', '2', '3', '4', ' ', 'a', 'y', 'x', '日', '👍', '[', ']', 's', '.', 'c', 'Q', 'i',
        'h', 'A', '5', '+', '<', '>', 'H', 'L', 'm', 'M', 'B',
    ];
    let codes = [
        KeyCode::Esc,
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Backspace,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::PageDown,
        KeyCode::PageUp,
        KeyCode::Home,
        KeyCode::End,
    ];
    match rng.below(10) {
        0..=5 => KeyEvent::from(KeyCode::Char(CHARS[rng.below(CHARS.len())])),
        6..=8 => KeyEvent::from(codes[rng.below(codes.len())]),
        _ => KeyEvent::new(
            KeyCode::Char(['s', 'o', 'x', 'u', 't'][rng.below(5)]),
            KeyModifiers::CONTROL,
        ),
    }
}

/// The selected item of each list, by its key.
fn selections(app: &App) -> Vec<(&'static str, Option<String>)> {
    fn key<T: crate::tui::app::Keyed>(l: &crate::tui::app::List<T>) -> Option<String> {
        l.current().map(|i| i.key().to_string())
    }
    vec![
        ("timeline", key(&app.timeline)),
        ("search posts", key(&app.search.posts)),
        ("notifications", key(&app.notifications)),
        ("conversations", key(&app.chat.convos)),
    ]
}

/// Whether the list `name` holds the item keyed `key`.
fn holds(app: &App, name: &str, key: &str) -> bool {
    use crate::tui::app::Keyed;
    match name {
        "timeline" => app.timeline.items.iter().any(|i| i.key() == key),
        "search posts" => app.search.posts.items.iter().any(|i| i.key() == key),
        "notifications" => app.notifications.items.iter().any(|i| i.key() == key),
        "conversations" => app.chat.convos.items.iter().any(|i| i.key() == key),
        _ => false,
    }
}

fn check_lists(app: &App, seed: u64, step: usize) {
    let bounded = |name: &str, len: usize, selected: usize| {
        assert!(
            len == 0 || selected < len,
            "seed {seed} step {step}: {name} selected {selected} of {len}"
        );
    };
    bounded("timeline", app.timeline.items.len(), app.timeline.selected);
    bounded(
        "search posts",
        app.search.posts.items.len(),
        app.search.posts.selected,
    );
    bounded(
        "search actors",
        app.search.actors.items.len(),
        app.search.actors.selected,
    );
    bounded(
        "profile posts",
        app.profile.posts.items.len(),
        app.profile.posts.selected,
    );
    bounded(
        "notifications",
        app.notifications.items.len(),
        app.notifications.selected,
    );
    for c in &app.columns.items {
        match &c.rows {
            crate::tui::columns::Rows::Posts(l) => bounded("column", l.items.len(), l.selected),
            crate::tui::columns::Rows::Notifications(l) => {
                bounded("column", l.items.len(), l.selected)
            }
        }
    }
    assert!(
        app.columns.items.is_empty() || app.columns.focus < app.columns.items.len(),
        "seed {seed} step {step}: column focus {} of {}",
        app.columns.focus,
        app.columns.items.len()
    );
}

#[test]
fn random_keys_and_late_answers_keep_the_client_sound() {
    let dir = tempfile::tempdir().unwrap();
    // A short run in every `cargo test`; BSKY_FUZZ_SEEDS=2000 (in a
    // release build) for a long one.
    let seeds: u64 = std::env::var("BSKY_FUZZ_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    for seed in 1..=seeds {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let session = Session {
            service: "https://pds.test".into(),
            did: "did:plc:me".into(),
            handle: "me.test".into(),
            access_jwt: "a".into(),
            refresh_jwt: "r".into(),
        };
        let (mut app, jobs) = App::new(Some(session), "https://pds.test");
        app.browse_from = Some(dir.path().to_path_buf());
        // The settings screen's folders stay fixed: choosing one tries
        // to write in it, and a fuzz run must not write outside its own
        // folder. Choosing them is tested on its own in app/tests/settings.rs.
        let fixed = Some(dir.path().display().to_string());
        app.env.download_dir = fixed.clone();
        app.env.cache_dir = fixed;
        // The post D asked about, which the y after it must delete.
        let mut delete_asked: Option<String> = None;
        let mut next_id = 0u64;
        // Answers waiting to arrive, each with the number its job was sent
        // with, as the event loop numbers them: the answer to a job the
        // client has since overtaken is told apart by it.
        let mut pending: Vec<(u64, Event)> = Vec::new();
        for job in jobs {
            let seq = app.stamp(&job);
            pending.extend(answer(&mut rng, job, &mut next_id).map(|ev| (seq, ev)));
        }
        let mut images = Images::new(Picker::halfblocks(), None);
        // Some runs get their answers at once, others keep them waiting
        // over many keys, which is where a key meets a list still loading.
        let answer_rate = [5, 15, 35, 60][rng.below(4)];
        for step in 0..800 {
            if rng.chance(answer_rate) && !pending.is_empty() {
                // Any waiting answer, not only the oldest: answers arrive
                // late and out of order.
                let (seq, ev) = pending.swap_remove(rng.below(pending.len()));
                let before = selections(&app);
                let jobs = app.handle_answer(seq, ev);
                // An answer never moves a selection off an item still in
                // its list: the next key would act on another one.
                for ((name, was), (_, now)) in before.into_iter().zip(selections(&app)) {
                    if let Some(was) = was.filter(|was| holds(&app, name, was)) {
                        assert_eq!(
                            now.as_deref(),
                            Some(was.as_str()),
                            "seed {seed} step {step}: an answer moved the {name} selection off {was}"
                        );
                    }
                }
                for job in jobs {
                    let seq = app.stamp(&job);
                    pending.extend(answer(&mut rng, job, &mut next_id).map(|ev| (seq, ev)));
                }
            } else {
                let k = key(&mut rng);
                // What a like or repost must act on: the post selected
                // when the key is pressed, whatever arrives later. The
                // actions list runs the key on that same post.
                let on_list = matches!(app.overlay, None | Some(Overlay::Actions { .. }));
                // The actions list acts on the post it was opened on.
                let target = match &app.overlay {
                    Some(Overlay::Actions { about, .. }) => about.clone(),
                    _ => on_list
                        .then(|| app.shown_post().map(|p| p.uri.clone()))
                        .flatten(),
                };
                let account = on_list
                    .then(|| app.shown_account().map(|a| a.did.clone()))
                    .flatten();
                let asked = delete_asked.take();
                let block_asked = app.confirm_block.clone();
                let composing = matches!(app.overlay, Some(Overlay::Compose(_)));
                let jobs = app.handle_key(k);
                let writes = jobs.iter().filter(|j| is_write(j)).count();
                assert!(
                    writes <= 1,
                    "seed {seed} step {step}: {k:?} sent {writes} writes: {jobs:?}"
                );
                // Only the keys that write can: l b f, y after D, ctrl+s,
                // and enter in the actions list.
                let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
                if writes > 0 {
                    let writer = match k.code {
                        KeyCode::Char('s') => ctrl,
                        KeyCode::Char('l' | 'b' | 'f' | 'y' | 'M' | 'B') | KeyCode::Enter => !ctrl,
                        _ => false,
                    };
                    assert!(
                        writer,
                        "seed {seed} step {step}: {k:?} is not a key that writes, and sent {jobs:?}"
                    );
                }
                if app.confirm_delete.is_some() {
                    assert_eq!(
                        app.confirm_delete, target,
                        "seed {seed} step {step}: D asked about another post than the selected one"
                    );
                    delete_asked = app.confirm_delete.clone();
                }
                if app.confirm_block.is_some() && app.confirm_block != block_asked {
                    assert_eq!(
                        app.confirm_block, account,
                        "seed {seed} step {step}: B asked about another account than the one shown"
                    );
                }
                // A reply or a quote opened now is of the selected post.
                if !composing && let Some(Overlay::Compose(c)) = &app.overlay {
                    let of = c
                        .reply
                        .as_ref()
                        .map(|r| r.0.parent.uri.clone())
                        .or_else(|| c.quote.as_ref().map(|q| q.0.uri.clone()));
                    if of.is_some() {
                        assert_eq!(
                            of, target,
                            "seed {seed} step {step}: {k:?} answered another post than the selected one"
                        );
                    }
                }
                for job in &jobs {
                    match job {
                        Job::DeletePost { uri } => {
                            assert_eq!(
                                Some(uri),
                                asked.as_ref(),
                                "seed {seed} step {step}: deleted a post D did not ask about"
                            );
                            assert!(
                                uri.starts_with("at://did:plc:me/"),
                                "seed {seed} step {step}: deleted someone else's post {uri}"
                            );
                        }
                        Job::Follow { did }
                        | Job::Unfollow { did, .. }
                        | Job::Mute { did, .. }
                        | Job::Unblock { did, .. } => assert_eq!(
                            Some(did),
                            account.as_ref(),
                            "seed {seed} step {step}: {k:?} acted on another account than the one shown"
                        ),
                        // A block only on the y after B, of the account B
                        // asked about.
                        Job::Block { did } => {
                            assert_eq!(
                                Some(did),
                                block_asked.as_ref(),
                                "seed {seed} step {step}: blocked an account B did not ask about"
                            );
                            assert_eq!(k.code, KeyCode::Char('y'), "seed {seed} step {step}");
                        }
                        _ => {}
                    }
                }
                for job in &jobs {
                    let acted_on = match job {
                        Job::Like { subject } | Job::Repost { subject } => Some(&subject.uri),
                        Job::Unlike { post_uri, .. } | Job::Unrepost { post_uri, .. } => {
                            Some(post_uri)
                        }
                        _ => None,
                    };
                    if let Some(uri) = acted_on {
                        assert_eq!(
                            Some(uri),
                            target.as_ref(),
                            "seed {seed} step {step}: {k:?} acted on another post than the selected one"
                        );
                    }
                }
                for job in jobs {
                    let seq = app.stamp(&job);
                    pending.extend(answer(&mut rng, job, &mut next_id).map(|ev| (seq, ev)));
                }
            }
            if app.quit {
                break;
            }
            check_lists(&app, seed, step);
            if step % 7 == 0 {
                let (w, h) = (1 + rng.below(120) as u16, 1 + rng.below(50) as u16);
                let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                term.draw(|f| draw(f, &mut app, &mut images)).unwrap();
            }
        }
        // Every answer delivered, the ones they lead to as well: nothing may
        // still wait for the server.
        let mut rounds = 0;
        while let Some((seq, ev)) = pending.pop() {
            for job in app.handle_answer(seq, ev) {
                let seq = app.stamp(&job);
                pending.extend(answer(&mut rng, job, &mut next_id).map(|ev| (seq, ev)));
            }
            rounds += 1;
            assert!(rounds < 10_000, "seed {seed}: answers keep asking for more");
        }
        let stuck = app.stuck();
        assert!(
            stuck.is_empty(),
            "seed {seed}: after every answer, still {stuck:?}"
        );
    }
}
