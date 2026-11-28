//! What the commands print with `--json`: the shape `doc/json.md` gives,
//! which msky and mstdn print too. `--raw` prints the server's own objects
//! instead, as bsky 0.15.0 and earlier did.

use serde_json::{Value, json};

use crate::api::types::{ChatMessage, Convo, Embed, FeedItem, Media, Notification, Post, Profile};

/// The account's page on bsky.app.
fn profile_url(p: &Profile) -> String {
    let who = if p.handle.is_empty() || p.handle == "handle.invalid" {
        &p.did
    } else {
        &p.handle
    };
    format!("https://bsky.app/profile/{who}")
}

/// The `user` of a post, a notification or a message. A handle is a whole
/// domain, so there is no host beside it.
pub(super) fn user(p: &Profile) -> Value {
    json!({
        "id": p.did,
        "username": p.handle,
        "host": null,
        "name": p.display_name.as_deref().map(str::trim).filter(|n| !n.is_empty()),
    })
}

/// An account, as `whoami`, `profile` and the lists of accounts print it.
pub(super) fn account(p: &Profile) -> Value {
    let mut v = user(p);
    v["url"] = json!(profile_url(p));
    v["description"] = json!(p.description);
    v["followersCount"] = json!(p.followers_count);
    v["followingCount"] = json!(p.follows_count);
    v["postsCount"] = json!(p.posts_count);
    v["following"] = json!(p.viewer.as_ref().map(|v| v.following.is_some()));
    v
}

/// The pictures and the video of a post: its own, not those of the post
/// it quotes, which the client draws with it.
fn own_media(embed: Option<&Embed>) -> Vec<Media> {
    match embed {
        Some(e @ (Embed::Images { .. } | Embed::Video { .. })) => e.media(),
        Some(Embed::RecordWithMedia { media, .. }) => media.media(),
        _ => Vec::new(),
    }
}

fn file(m: &Media) -> Value {
    match m {
        Media::Image {
            url, thumb, alt, ..
        } => json!({
            "id": null,
            "type": "image",
            "mime": null,
            "url": url,
            "previewUrl": (!thumb.is_empty()).then_some(thumb),
            "description": (!alt.is_empty()).then_some(alt),
        }),
        Media::Video {
            playlist,
            thumbnail,
            alt,
            ..
        } => json!({
            "id": null,
            "type": "video",
            "mime": null,
            "url": playlist,
            "previewUrl": thumbnail,
            "description": (!alt.is_empty()).then_some(alt),
        }),
    }
}

/// The at:// URI of the post `embed` quotes, if it quotes one.
fn quoted(embed: Option<&Embed>) -> Option<String> {
    let q = embed?.quoted()?;
    let uri = q.get("uri").and_then(Value::as_str)?;
    uri.contains("/app.bsky.feed.post/")
        .then(|| uri.to_string())
}

/// A post: `id` and `uri` its at:// URI, `url` its page on bsky.app.
pub(super) fn post(p: &Post) -> Value {
    post_with(p, Some((p.like_count, p.repost_count, p.reply_count)))
}

/// A post, with its counts, or `null` for each when the server did not
/// send them (the post of a notification).
fn post_with(p: &Post, counts: Option<(u64, u64, u64)>) -> Value {
    let record = p.record();
    let created = record
        .created_at
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| p.indexed_at.clone());
    let files: Vec<Value> = own_media(p.embed.as_ref()).iter().map(file).collect();
    json!({
        "id": p.uri,
        "url": p.web_url(),
        "uri": p.uri,
        "createdAt": created,
        "user": user(&p.author),
        "text": record.text,
        "cw": null,
        "likeCount": counts.map(|c| c.0),
        "repostCount": counts.map(|c| c.1),
        "repliesCount": counts.map(|c| c.2),
        "files": files,
        "replyId": record.reply.map(|r| r.parent.uri),
        "quoteId": quoted(p.embed.as_ref()),
        "repostOf": null,
        "cid": p.cid,
        "quoteCount": counts.map(|_| p.quote_count),
        "langs": p.raw_record.get("langs").cloned().unwrap_or(Value::Null),
    })
}

/// An item of a feed: its post, or for a repost the repost, by whom and
/// when, with the post it reposts in `repostOf`.
pub(super) fn feed_item(item: &FeedItem) -> Value {
    let Some(reason) = item.reason.as_ref().filter(|r| {
        r.get("$type").and_then(Value::as_str) == Some("app.bsky.feed.defs#reasonRepost")
    }) else {
        return post(&item.post);
    };
    let by: Profile = reason
        .get("by")
        .and_then(|b| serde_json::from_value(b.clone()).ok())
        .unwrap_or_default();
    let s = |k: &str| reason.get(k).and_then(Value::as_str);
    json!({
        "id": s("uri"),
        "url": null,
        "uri": s("uri"),
        "createdAt": s("indexedAt"),
        "user": user(&by),
        "text": null,
        "cw": null,
        "likeCount": 0,
        "repostCount": 0,
        "repliesCount": 0,
        "files": [],
        "replyId": null,
        "quoteId": null,
        "repostOf": post(&item.post),
        "cid": s("cid"),
        "quoteCount": 0,
        "langs": null,
    })
}

/// A notification. `id` is the at:// URI of what caused it; for a reply,
/// a mention or a quote that is the post, whose text and files the
/// notification carries (its counts it does not).
pub(super) fn notification(n: &Notification, raw: &Value) -> Value {
    let is_post = matches!(n.reason.as_str(), "reply" | "mention" | "quote");
    let post_id = if is_post {
        Some(n.uri.clone())
    } else {
        n.reason_subject.clone()
    };
    let post = is_post.then(|| {
        let p = Post {
            uri: n.uri.clone(),
            cid: n.cid.clone(),
            author: n.author.clone(),
            raw_record: raw.get("record").cloned().unwrap_or(Value::Null),
            indexed_at: n.indexed_at.clone(),
            ..Post::default()
        };
        post_with(&p, None)
    });
    json!({
        "id": n.uri,
        "createdAt": n.indexed_at,
        "type": n.reason,
        "user": user(&n.author),
        "postId": post_id,
        "post": post,
        "isRead": n.is_read,
    })
}

/// A direct message: who sent it, among `c`'s members.
pub(super) fn message(m: &ChatMessage, c: &Convo) -> Value {
    let from = c.members.iter().find(|p| p.did == m.sender);
    json!({
        "id": m.id,
        "createdAt": m.sent_at,
        "user": from.map(user),
        "text": (!m.deleted).then_some(&m.text),
        "convoId": c.id,
    })
}

/// A conversation: who is in it, how many unread, its last message.
pub(super) fn convo(c: &Convo) -> Value {
    json!({
        "id": c.id,
        "members": c.members.iter().map(user).collect::<Vec<_>>(),
        "unreadCount": c.unread_count,
        "muted": c.muted,
        "lastMessage": c.last_message.as_ref().map(|m| message(m, c)),
    })
}

/// A list: its name, what it is for, how many are on it, who made it.
pub(super) fn list(v: &Value) -> Value {
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    let creator: Profile = v
        .get("creator")
        .and_then(|c| serde_json::from_value(c.clone()).ok())
        .unwrap_or_default();
    let url = s("uri").and_then(|u| {
        let (did, rkey) = u
            .strip_prefix("at://")?
            .split_once("/app.bsky.graph.list/")?;
        Some(format!("https://bsky.app/profile/{did}/lists/{rkey}"))
    });
    json!({
        "id": s("uri"),
        "url": url,
        "name": s("name"),
        "purpose": s("purpose").and_then(|p| p.rsplit('#').next()),
        "description": s("description"),
        "memberCount": v.get("listItemCount").and_then(Value::as_u64),
        "user": user(&creator),
    })
}

/// What a write did: the command, the post or account it was done to,
/// the record it made and its page, and whether there was nothing to do.
pub(super) fn wrote(
    action: &str,
    target: Option<&str>,
    id: Option<&str>,
    url: Option<&str>,
    already: bool,
) -> Value {
    json!({"action": action, "target": target, "id": id, "url": url, "already": already})
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_kit::json::{self as shape, missing};

    fn keys_of(v: &Value) -> Vec<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }

    fn a_post(v: Value) -> Post {
        serde_json::from_value(v).unwrap()
    }

    // The post of every command has the keys msky and mstdn print; a
    // quote's pictures are its own, and the post it quotes is named.
    #[test]
    fn a_post_has_the_shape_every_client_prints() {
        let p = a_post(json!({
            "uri": "at://did:plc:a/app.bsky.feed.post/3k", "cid": "c1",
            "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "家族👨\u{200d}👩\u{200d}👧"},
            "record": {"text": "見て 🇯🇵", "createdAt": "2026-11-20T10:00:00Z", "langs": ["ja"],
                       "reply": {"root": {"uri": "at://r", "cid": "c"}, "parent": {"uri": "at://p", "cid": "c"}}},
            "embed": {"$type": "app.bsky.embed.recordWithMedia#view",
                "record": {"record": {"uri": "at://did:plc:b/app.bsky.feed.post/q", "author": {"handle": "bob.test"}, "value": {"text": "元"},
                    "embeds": [{"$type": "app.bsky.embed.images#view", "images": [{"thumb": "https://t/theirs", "fullsize": "https://f/theirs", "alt": ""}]}]}},
                "media": {"$type": "app.bsky.embed.images#view", "images": [{"thumb": "https://t/1", "fullsize": "https://f/1", "alt": "山"}]}},
            "likeCount": 3, "repostCount": 2, "replyCount": 1, "quoteCount": 4,
            "indexedAt": "2026-11-20T10:00:01Z"
        }));
        let v = post(&p);
        let keys = keys_of(&v);
        assert!(missing(shape::POST, &keys).is_empty(), "{keys:?}");
        assert!(missing(shape::USER, &keys_of(&v["user"])).is_empty());
        assert_eq!(v["id"], "at://did:plc:a/app.bsky.feed.post/3k");
        assert_eq!(v["url"], "https://bsky.app/profile/did:plc:a/post/3k");
        assert_eq!(
            v["user"],
            json!({"id": "did:plc:a", "username": "alice.test", "host": null, "name": "家族👨\u{200d}👩\u{200d}👧"})
        );
        assert_eq!(v["createdAt"], "2026-11-20T10:00:00Z");
        assert_eq!(
            (
                v["likeCount"].clone(),
                v["repostCount"].clone(),
                v["repliesCount"].clone()
            ),
            (json!(3), json!(2), json!(1))
        );
        assert_eq!(v["replyId"], "at://p");
        assert_eq!(v["quoteId"], "at://did:plc:b/app.bsky.feed.post/q");
        assert_eq!(v["files"].as_array().unwrap().len(), 1);
        assert!(missing(shape::FILE, &keys_of(&v["files"][0])).is_empty());
        assert_eq!(v["files"][0]["url"], "https://f/1");
        assert_eq!(v["files"][0]["description"], "山");
        assert_eq!(v["quoteCount"], 4);
        assert_eq!(v["langs"], json!(["ja"]));
    }

    // A repost is the repost, by whom and when, with the post whole.
    #[test]
    fn a_repost_holds_the_post_it_reposts() {
        let item: FeedItem = serde_json::from_value(json!({
            "post": {"uri": "at://did:plc:b/app.bsky.feed.post/1", "cid": "c", "author": {"did": "did:plc:b", "handle": "bob.test"}, "record": {"text": "元"}},
            "reason": {"$type": "app.bsky.feed.defs#reasonRepost", "by": {"did": "did:plc:a", "handle": "alice.test"},
                       "uri": "at://did:plc:a/app.bsky.feed.repost/r", "indexedAt": "2026-11-20T10:00:00Z"}
        }))
        .unwrap();
        let v = feed_item(&item);
        assert!(missing(shape::POST, &keys_of(&v)).is_empty());
        assert_eq!(v["user"]["username"], "alice.test");
        assert_eq!(v["text"], Value::Null);
        assert_eq!(v["repostOf"]["text"], "元");
        assert_eq!(v["repostOf"]["user"]["username"], "bob.test");
        // A pinned post is the post.
        let pinned: FeedItem = serde_json::from_value(json!({
            "post": {"uri": "at://x/app.bsky.feed.post/2", "cid": "c", "author": {}, "record": {"text": "📌"}},
            "reason": {"$type": "app.bsky.feed.defs#reasonPin"}
        }))
        .unwrap();
        assert_eq!(feed_item(&pinned)["text"], "📌");
    }

    // A reply is about the reply itself, whose text the notification has;
    // a like about the post liked, which it does not carry.
    #[test]
    fn a_notification_names_the_post_it_is_about() {
        let raw = json!({"uri": "at://did:plc:b/app.bsky.feed.post/r", "cid": "c", "reason": "reply", "isRead": false,
            "indexedAt": "2026-11-20T10:00:00Z", "author": {"did": "did:plc:b", "handle": "bob.test"},
            "record": {"text": "返信 👍🏽", "createdAt": "2026-11-20T09:59:00Z"}});
        let n: Notification = serde_json::from_value(raw.clone()).unwrap();
        let v = notification(&n, &raw);
        assert!(missing(shape::NOTIFICATION, &keys_of(&v)).is_empty());
        assert_eq!(v["type"], "reply");
        assert_eq!(v["postId"], "at://did:plc:b/app.bsky.feed.post/r");
        assert_eq!(v["post"]["text"], "返信 👍🏽");
        assert_eq!(v["post"]["likeCount"], Value::Null);
        assert_eq!(v["isRead"], false);
        let raw = json!({"uri": "at://did:plc:b/app.bsky.feed.like/l", "reason": "like",
            "reasonSubject": "at://did:plc:me/app.bsky.feed.post/1", "author": {"did": "did:plc:b", "handle": "bob.test"}, "record": {}});
        let n: Notification = serde_json::from_value(raw.clone()).unwrap();
        let v = notification(&n, &raw);
        assert_eq!(v["postId"], "at://did:plc:me/app.bsky.feed.post/1");
        assert_eq!(v["post"], Value::Null);
        assert!(shape::NOTIFICATION_TYPES.contains(&v["type"].as_str().unwrap()));
    }

    #[test]
    fn an_account_is_a_user_and_its_counts() {
        let p: Profile = serde_json::from_value(json!({
            "did": "did:plc:b", "handle": "bob.test", "displayName": " ", "description": "🐈",
            "followersCount": 3, "followsCount": 4, "postsCount": 5,
            "viewer": {"following": "at://did:plc:me/app.bsky.graph.follow/1"}
        }))
        .unwrap();
        let v = account(&p);
        assert!(missing(shape::ACCOUNT, &keys_of(&v)).is_empty());
        assert_eq!(v["name"], Value::Null);
        assert_eq!(v["url"], "https://bsky.app/profile/bob.test");
        assert_eq!(
            (v["followingCount"].clone(), v["postsCount"].clone()),
            (json!(4), json!(5))
        );
        assert_eq!(v["following"], true);
    }

    #[test]
    fn a_message_says_who_sent_it() {
        let c: Convo = serde_json::from_value(json!({"id": "cv", "members": [
            {"did": "did:plc:me", "handle": "me.test"}, {"did": "did:plc:a", "handle": "alice.test"}]}))
        .unwrap();
        let m = ChatMessage {
            id: "m1".into(),
            text: "やあ 👋".into(),
            sender: "did:plc:a".into(),
            sent_at: "t".into(),
            ..Default::default()
        };
        let v = message(&m, &c);
        assert!(missing(shape::MESSAGE, &keys_of(&v)).is_empty());
        assert_eq!(v["user"]["username"], "alice.test");
        assert_eq!(v["convoId"], "cv");
        let gone = ChatMessage { deleted: true, ..m };
        assert_eq!(message(&gone, &c)["text"], Value::Null);
        assert!(
            missing(
                shape::WROTE,
                &keys_of(&wrote("like", Some("at://p"), Some("at://l"), None, false))
            )
            .is_empty()
        );
    }
}
