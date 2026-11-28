//! What the commands print: plain lines for reading, no color, nothing cut,
//! each status closed by its address so it can be given to the next
//! command; and with `--json` one object per status, the same shape for
//! every command, as msky's.

use serde_json::{Value, json};

use crate::api::types::{Account, Media, Notification, Status};
use crate::html;
use crate::text::{drawable, one_line};
pub(crate) use terminal_kit::clock::local_time as time;

/// The address of a status on the server at `origin`: `/@acct/ID`, which
/// that server shows a status of another server at too, so the next
/// command reads it there.
pub(crate) fn status_url(origin: &str, s: &Status) -> String {
    format!("{origin}/@{}/{}", s.account.acct, s.id)
}

/// The address of account `a` on the server at `origin`.
fn account_url(a: &Account, origin: &str) -> String {
    format!("{origin}/@{}", a.acct)
}

/// `@acct`: `@alice` for an account of the server asked, `@bob@host` for
/// one of another.
fn handle(a: &Account) -> String {
    let acct = if a.acct.is_empty() {
        &a.username
    } else {
        &a.acct
    };
    format!("@{}", one_line(acct))
}

/// `display name @acct`, or the handle alone when there is no display
/// name.
fn who(a: &Account) -> String {
    let name = one_line(&a.display_name);
    if name.trim().is_empty() || name == a.username {
        handle(a)
    } else {
        format!("{name} {}", handle(a))
    }
}

/// `[2 pictures]`, `[1 video]`... one line per kind of file. A GIF that
/// Mastodon made a looping video counts as a video.
fn file_lines(files: &[Media]) -> String {
    let kinds: [(&[&str], &str, &str); 3] = [
        (&["image"], "picture", "pictures"),
        (&["video", "gifv"], "video", "videos"),
        (&["audio"], "sound", "sounds"),
    ];
    let mut out = String::new();
    let mut counted = 0;
    for (of, one, many) in kinds {
        let n = files
            .iter()
            .filter(|f| of.contains(&f.kind.as_str()))
            .count();
        counted += n;
        match n {
            0 => {}
            1 => out.push_str(&format!("[1 {one}]\n")),
            n => out.push_str(&format!("[{n} {many}]\n")),
        }
    }
    match files.len() - counted {
        0 => {}
        1 => out.push_str("[1 file]\n"),
        n => out.push_str(&format!("[{n} files]\n")),
    }
    out
}

/// The text of a status, its lines as they are to be printed.
fn body(s: &Status, out: &mut String) {
    if !s.spoiler_text.trim().is_empty() {
        out.push_str(&format!("CW: {}\n", one_line(&s.spoiler_text)));
    }
    let text = html::to_text(&s.content);
    for line in drawable(&text).lines() {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&file_lines(&s.media_attachments));
    if let Some(p) = &s.poll {
        let options: Vec<String> = p
            .options
            .iter()
            .map(|o| match o.votes_count {
                Some(n) => format!("{} {n}", one_line(&o.title)),
                None => one_line(&o.title),
            })
            .collect();
        let ended = if p.expired { ", ended" } else { "" };
        out.push_str(&format!("[poll{ended}: {}]\n", options.join(" / ")));
    }
}

/// A status: who and when, its warning and text, what it carries, its
/// counts, its address. A boost shows the status it boosts, under a line
/// that says who boosted it, as msky says who renoted a note.
pub(crate) fn status(s: &Status, origin: &str) -> String {
    let mut out = String::new();
    let shown = s.shown();
    if s.reblog.is_some() {
        out.push_str(&format!("⟳ boosted by {}\n", handle(&s.account)));
    }
    out.push_str(&format!(
        "{} · {}\n",
        who(&shown.account),
        time(&shown.created_at)
    ));
    body(shown, &mut out);
    out.push_str(&format!(
        "♡ {}  ⟳ {}  ↩ {}\n{}\n",
        shown.favourites_count,
        shown.reblogs_count,
        shown.replies_count,
        one_line(&status_url(origin, shown))
    ));
    out
}

/// An account as a status's `user` is printed with `--json`: `host` of an
/// account of the server asked is that server's host, so a line read on
/// its own still says where the account is.
fn user_json(a: &Account, origin: &str) -> Value {
    json!({
        "id": a.id,
        "username": a.username,
        "host": a.host(crate::api::authority(origin)),
        "name": (!a.display_name.is_empty()).then_some(&a.display_name),
    })
}

/// A file's `type` in the JSON every client prints: a GIF that Mastodon
/// made a looping video is a video, and what Mastodon cannot tell is a file.
pub(crate) fn file_type(kind: &str) -> &'static str {
    match kind {
        "image" => "image",
        "video" | "gifv" => "video",
        "audio" => "audio",
        _ => "file",
    }
}

/// A status as `--json` prints it, the post of `doc/json.md`: its `user`
/// as [`user_json`] puts it and its text as [`html::to_text`] reads it
/// (`content` keeps the HTML).
pub(crate) fn status_json(s: &Status, origin: &str) -> Value {
    let files: Vec<Value> = s
        .media_attachments
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "type": file_type(&m.kind),
                "mime": null,
                "url": m.source(),
                "previewUrl": m.preview_url,
                "description": m.description,
            })
        })
        .collect();
    let poll = s.poll.as_ref().map(|p| {
        json!({
            "expired": p.expired,
            "votesCount": p.votes_count,
            "options": p.options.iter().map(|o| json!({"title": o.title, "votesCount": o.votes_count})).collect::<Vec<_>>(),
        })
    });
    json!({
        "id": s.id,
        "createdAt": s.created_at,
        "url": status_url(origin, s),
        "uri": s.uri,
        "user": user_json(&s.account, origin),
        "text": html::to_text(&s.content),
        "content": s.content,
        "cw": (!s.spoiler_text.is_empty()).then_some(&s.spoiler_text),
        "sensitive": s.sensitive,
        "visibility": s.visibility,
        "language": s.language,
        "likeCount": s.favourites_count,
        "repostCount": s.reblogs_count,
        "repliesCount": s.replies_count,
        "files": files,
        "mentions": s.mentions.iter().map(|m| &m.acct).collect::<Vec<_>>(),
        "tags": s.tags.iter().map(|t| &t.name).collect::<Vec<_>>(),
        "poll": poll,
        "replyId": s.in_reply_to_id,
        "quoteId": null,
        "repostOf": s.reblog.as_deref().map(|r| status_json(r, origin)),
    })
}

/// An account, as `whoami` and `profile` print it: who, with the server's
/// host, its address, its counts and what it says about itself.
pub(crate) fn account(a: &Account, origin: &str) -> String {
    let host = a.host(crate::api::authority(origin));
    let name = one_line(&a.display_name);
    let handle = format!("@{}@{host}", one_line(&a.username));
    let head = if name.trim().is_empty() || name == a.username {
        handle
    } else {
        format!("{name} {handle}")
    };
    let mut out = format!(
        "{head}\n{}\n{} followers  {} following  {} posts\n",
        one_line(&account_url(a, origin)),
        a.followers_count.unwrap_or(0),
        a.following_count.unwrap_or(0),
        a.statuses_count.unwrap_or(0),
    );
    let note = html::to_text(&a.note);
    if !note.trim().is_empty() {
        out.push('\n');
        for line in drawable(&note).lines() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// An account as `whoami --json` and `profile --json` print it: a status's
/// `user` with its address, what it says about itself and its counts.
/// Mastodon says whether you follow it elsewhere, so `following` is null.
pub(crate) fn account_json(a: &Account, origin: &str) -> Value {
    let mut v = user_json(a, origin);
    v["url"] = json!(account_url(a, origin));
    v["description"] = json!(html::to_text(&a.note));
    v["locked"] = json!(a.locked);
    v["bot"] = json!(a.bot);
    v["followersCount"] = json!(a.followers_count);
    v["followingCount"] = json!(a.following_count);
    v["postsCount"] = json!(a.statuses_count);
    v["following"] = Value::Null;
    v
}

/// What a notification says happened.
fn happened(n: &Notification) -> String {
    match n.kind.as_str() {
        "mention" => "mentioned you".into(),
        "status" => "posted".into(),
        "reblog" => "boosted your post".into(),
        "favourite" => "favourited your post".into(),
        "follow" => "followed you".into(),
        "follow_request" => "asked to follow you".into(),
        "poll" => "a poll has ended".into(),
        "update" => "edited a post".into(),
        "quote" => "quoted your post".into(),
        "admin.sign_up" => "signed up".into(),
        "admin.report" => "made a report".into(),
        "severed_relationships" => "some of your follows were cut off by a moderator".into(),
        "moderation_warning" => "the moderators warned you".into(),
        other => one_line(other),
    }
}

/// A notification: who did what and when, the status it is about (its
/// warning and text), and the address to go on from: the status's, else
/// the account's.
pub(crate) fn notification(n: &Notification, origin: &str) -> String {
    let did = happened(n);
    let mut out = match &n.account {
        Some(a) => format!("{} {did} · {}\n", who(a), time(&n.created_at)),
        None => format!("{did} · {}\n", time(&n.created_at)),
    };
    if let Some(s) = &n.status {
        let shown = s.shown();
        body(shown, &mut out);
        out.push_str(&format!("{}\n", one_line(&status_url(origin, shown))));
    } else if let Some(a) = &n.account {
        out.push_str(&format!("{}\n", one_line(&account_url(a, origin))));
    }
    out
}

/// A notification's `type` in the JSON every client prints: a favourite
/// is a like and a reblog a repost; the rest keep Mastodon's word.
fn notification_type(kind: &str) -> &str {
    match kind {
        "favourite" => "like",
        "reblog" => "repost",
        other => other,
    }
}

/// A notification as `notifications --json` prints it: what happened, who
/// did it (a status's `user`, or null), and the status it is about in the
/// shape every command prints (or null).
pub(crate) fn notification_json(n: &Notification, origin: &str) -> Value {
    json!({
        "id": n.id,
        "createdAt": n.created_at,
        "type": notification_type(&n.kind),
        "user": n.account.as_ref().map(|a| user_json(a, origin)),
        "postId": n.status.as_ref().map(|s| &s.id),
        "post": n.status.as_ref().map(|s| status_json(s, origin)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str = "https://mastodon.test";

    fn a_status(v: Value) -> Status {
        serde_json::from_value(v).unwrap()
    }

    // The text is read out of its HTML and printed whole, emoji, CJK and
    // right-to-left text alike, one line per line, and the address closes
    // the status so the next command can take it.
    #[test]
    fn a_status_is_printed_whole_and_ends_with_its_address() {
        let s = a_status(json!({
            "id": "109", "created_at": "not a time",
            "account": {"username": "alice", "acct": "alice", "display_name": "家族👨\u{200d}👩\u{200d}👧 Alice"},
            "content": "<p>今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}<br />مرحبا second line</p><p>:blobcat: <a href=\"https://mastodon.test/tags/rust\" class=\"mention hashtag\" rel=\"tag\">#<span>rust</span></a></p>",
            "favourites_count": 7, "reblogs_count": 1, "replies_count": 2,
            "media_attachments": [
                {"type": "image"}, {"type": "image"}, {"type": "gifv"}, {"type": "unknown"}
            ],
            "poll": {"expired": true, "votes_count": 3, "options": [{"title": "はい", "votes_count": 2}, {"title": "no", "votes_count": null}]}
        }));
        assert_eq!(
            status(&s, ORIGIN),
            "家族👨\u{200d}👩\u{200d}👧 Alice @alice · not a time\n\
             今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}\n\
             مرحبا second line\n\
             \n\
             :blobcat: #rust\n\
             [2 pictures]\n\
             [1 video]\n\
             [1 file]\n\
             [poll, ended: はい 2 / no]\n\
             ♡ 7  ⟳ 1  ↩ 2\n\
             https://mastodon.test/@alice/109\n"
        );
    }

    #[test]
    fn a_boost_shows_the_status_it_boosts_under_who_boosted_it() {
        let s = a_status(json!({
            "id": "b1", "created_at": "t1", "account": {"username": "alice", "acct": "alice"},
            "reblog": {"id": "o1", "created_at": "t0",
                       "account": {"username": "bob", "acct": "bob@other.example"},
                       "spoiler_text": "ネタバレ", "content": "<p>本文</p>", "favourites_count": 4}
        }));
        assert_eq!(
            status(&s, ORIGIN),
            "⟳ boosted by @alice\n\
             @bob@other.example · t0\n\
             CW: ネタバレ\n\
             本文\n\
             ♡ 4  ⟳ 0  ↩ 0\n\
             https://mastodon.test/@bob@other.example/o1\n"
        );
    }

    // What others wrote reaches the terminal as text only: an escape
    // sequence in a status, a name or a warning would otherwise set the
    // clipboard (OSC 52), the window title, or clear the screen; one
    // written as an entity is no different.
    #[test]
    fn control_characters_others_wrote_do_not_reach_the_terminal() {
        let evil = "a\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J\tb";
        let s = a_status(json!({
            "id": "x", "created_at": evil,
            "account": {"username": "eve", "acct": "eve", "display_name": "Eve\u{1b}]0;title\u{7}\n👨\u{200d}👩\u{200d}👧"},
            "content": format!("<p>{evil}&#27;[2J&#x9b;</p>"), "spoiler_text": evil
        }));
        let out = status(&s, ORIGIN);
        assert!(!out.chars().any(|c| c.is_control() && c != '\n'), "{out:?}");
        assert!(
            out.starts_with("Eve]0;title 👨\u{200d}👩\u{200d}👧 @eve · "),
            "{out:?}"
        );
        assert!(out.contains("\na]52;c;cHduZWQ=[2J    b[2J\n"), "{out:?}");
    }

    // Every command prints this one shape; an account of the server asked
    // gets its host filled in.
    #[test]
    fn the_json_of_a_status_has_one_shape() {
        let s = a_status(json!({
            "id": "109", "created_at": "2026-11-20T10:00:00.000Z", "uri": "https://mastodon.test/users/alice/statuses/109",
            "url": "https://mastodon.test/@alice/109",
            "account": {"id": "1", "username": "alice", "acct": "alice", "display_name": "Alice"},
            "content": "<p>🐈 <a href=\"https://e.test/a\">記事</a></p>", "spoiler_text": "", "sensitive": false,
            "visibility": "public", "language": "ja",
            "favourites_count": 2, "reblogs_count": 4, "replies_count": 5,
            "media_attachments": [{"id": "m1", "type": "image", "url": "https://cdn.test/m1.png", "preview_url": "https://cdn.test/s/m1.png", "description": "猫"}],
            "mentions": [{"acct": "bob@other.example"}], "tags": [{"name": "cat"}],
            "reblog": {"id": "r", "account": {"username": "bob", "acct": "bob@other.example"}}
        }));
        let v = status_json(&s, ORIGIN);
        assert_eq!(v["url"], "https://mastodon.test/@alice/109");
        assert_eq!(
            v["user"],
            json!({"id": "1", "username": "alice", "host": "mastodon.test", "name": "Alice"})
        );
        assert_eq!(v["text"], "🐈 記事 (https://e.test/a)");
        assert_eq!(v["cw"], Value::Null);
        assert_eq!(v["files"][0]["description"], "猫");
        assert_eq!(v["files"][0]["type"], "image");
        assert_eq!(v["files"][0]["mime"], Value::Null);
        assert_eq!(v["likeCount"], 2);
        assert_eq!(v["repostCount"], 4);
        assert_eq!(v["mentions"][0], "bob@other.example");
        assert_eq!(v["repostOf"]["user"]["host"], "other.example");
        assert_eq!(
            v["repostOf"]["url"],
            "https://mastodon.test/@bob@other.example/r"
        );
        let keys = keys_of(&v);
        assert_eq!(
            keys,
            [
                "content",
                "createdAt",
                "cw",
                "files",
                "id",
                "language",
                "likeCount",
                "mentions",
                "poll",
                "quoteId",
                "repliesCount",
                "replyId",
                "repostCount",
                "repostOf",
                "sensitive",
                "tags",
                "text",
                "uri",
                "url",
                "user",
                "visibility"
            ]
        );
        assert!(crate::json_keys::missing(crate::json_keys::POST, &keys).is_empty());
        assert!(crate::json_keys::missing(crate::json_keys::USER, &keys_of(&v["user"])).is_empty());
        assert!(
            crate::json_keys::missing(crate::json_keys::FILE, &keys_of(&v["files"][0])).is_empty()
        );
    }

    fn keys_of(v: &Value) -> Vec<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }

    // A GIF made a looping video is a video, and a kind Mastodon could not
    // tell is a file: the four types every client prints.
    #[test]
    fn a_file_is_one_of_the_four_types() {
        for (kind, want) in [
            ("image", "image"),
            ("gifv", "video"),
            ("video", "video"),
            ("audio", "audio"),
            ("unknown", "file"),
        ] {
            assert_eq!(file_type(kind), want);
            assert!(crate::json_keys::FILE_TYPES.contains(&want));
        }
    }

    fn a_notification(v: Value) -> Notification {
        serde_json::from_value(v).unwrap()
    }

    // Who did what, and the status it is about with the address to go on
    // from; a follow gives the account's address.
    #[test]
    fn a_notification_says_who_did_what_to_which_status() {
        let fav = a_notification(json!({
            "id": "n1", "created_at": "t", "type": "favourite",
            "account": {"username": "bob", "acct": "bob@other.example", "display_name": "Bob 🐈‍⬛"},
            "status": {"id": "109", "account": {"acct": "me"}, "content": "<p>朝ごはん 🍙<br>二行目</p>", "media_attachments": [{"type": "image"}]}
        }));
        assert_eq!(
            notification(&fav, ORIGIN),
            "Bob 🐈‍⬛ @bob@other.example favourited your post · t\n\
             朝ごはん 🍙\n\
             二行目\n\
             [1 picture]\n\
             https://mastodon.test/@me/109\n"
        );
        let follow = a_notification(json!({
            "id": "n3", "created_at": "t", "type": "follow", "account": {"username": "dave", "acct": "dave@x.example"}
        }));
        assert_eq!(
            notification(&follow, ORIGIN),
            "@dave@x.example followed you · t\nhttps://mastodon.test/@dave@x.example\n"
        );
        let unknown =
            a_notification(json!({"id": "n5", "created_at": "t", "type": "new\u{1b}[2J"}));
        assert_eq!(notification(&unknown, ORIGIN), "new[2J · t\n");
        let v = notification_json(&fav, ORIGIN);
        let keys = keys_of(&v);
        assert_eq!(keys, ["createdAt", "id", "post", "postId", "type", "user"]);
        assert!(crate::json_keys::missing(crate::json_keys::NOTIFICATION, &keys).is_empty());
        assert_eq!(v["type"], "like");
        assert_eq!(v["postId"], "109");
        assert_eq!(v["post"], status_json(fav.status.as_ref().unwrap(), ORIGIN));
    }

    #[test]
    fn whoami_shows_the_account_with_its_host_and_counts() {
        let a: Account = serde_json::from_value(json!({
            "id": "1", "username": "alice", "acct": "alice", "display_name": "アリス 👩‍💻",
            "note": "<p>こんにちは\u{1b}]52;c;eA==\u{7}<br>二行目</p>",
            "followers_count": 12, "following_count": 34, "statuses_count": 56
        }))
        .unwrap();
        assert_eq!(
            account(&a, ORIGIN),
            "アリス 👩‍💻 @alice@mastodon.test\n\
             https://mastodon.test/@alice\n\
             12 followers  34 following  56 posts\n\
             \n\
             こんにちは]52;c;eA==\n\
             二行目\n"
        );
        let v = account_json(&a, ORIGIN);
        assert_eq!(v["host"], "mastodon.test");
        assert_eq!(v["postsCount"], 56);
        assert!(crate::json_keys::missing(crate::json_keys::ACCOUNT, &keys_of(&v)).is_empty());
        assert_eq!(v["description"], "こんにちは\u{1b}]52;c;eA==\u{7}\n二行目");
    }
}
