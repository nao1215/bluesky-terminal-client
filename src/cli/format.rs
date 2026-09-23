//! The text the commands print: plain lines for reading, no color, nothing
//! cut. A post ends with its at:// URI so it can be given to the next
//! command.

use crate::api::types::{ChatMessage, Convo, Media, Notification, Post, Profile, ThreadNode};

pub use crate::clock::local_time as time;
use crate::tui::text::drawable;

/// Text someone wrote, as text only: a control character in it would reach
/// the terminal as a command (an escape sequence can set the clipboard or
/// the window title, or clear the screen), so it is dropped as the client
/// drops it. Line breaks stay.
fn plain(s: &str) -> std::borrow::Cow<'_, str> {
    drawable(s)
}

/// As [`plain`], on one line: for what is printed as one line of a list.
fn one_line(s: &str) -> String {
    drawable(s).replace('\n', " ")
}

/// `name @handle`, or `@handle` when there is no display name.
fn who(p: &Profile) -> String {
    let name = one_line(p.name());
    let handle = one_line(&p.handle);
    if name == handle {
        format!("@{handle}")
    } else {
        format!("{name} @{handle}")
    }
}

/// A post: who and when, its text, what it carries, its counts, its URI.
pub fn post(p: &Post) -> String {
    post_indented(p, "")
}

fn post_indented(p: &Post, pad: &str) -> String {
    let record = p.record();
    let when = record
        .created_at
        .as_deref()
        .filter(|t| !t.is_empty())
        .unwrap_or(&p.indexed_at);
    let mut out = format!("{pad}{} · {}\n", who(&p.author), time(when));
    for line in plain(&record.text).lines() {
        out.push_str(&format!("{pad}{line}\n"));
    }
    if let Some(embed) = &p.embed {
        let media = embed.media();
        let pictures = media
            .iter()
            .filter(|m| matches!(m, Media::Image { .. }))
            .count();
        match pictures {
            0 => {}
            1 => out.push_str(&format!("{pad}[1 picture]\n")),
            n => out.push_str(&format!("{pad}[{n} pictures]\n")),
        }
        if media.iter().any(|m| matches!(m, Media::Video { .. })) {
            out.push_str(&format!("{pad}[video]\n"));
        }
        if let Some(q) = embed.quoted() {
            let quoted = q
                .pointer("/author/handle")
                .and_then(|h| h.as_str())
                .map(|h| format!("[quotes @{}]", one_line(h)))
                .unwrap_or_else(|| "[quotes a post]".to_string());
            out.push_str(&format!("{pad}{quoted}\n"));
        }
    }
    for link in p.links().into_iter().take(1) {
        out.push_str(&format!("{pad}{}\n", one_line(&link)));
    }
    out.push_str(&format!(
        "{pad}♡ {}  ⟳ {}  ↩ {}\n{pad}{}\n",
        p.like_count,
        p.repost_count,
        p.reply_count,
        one_line(&p.uri)
    ));
    out
}

/// A thread: the posts above the one asked for, it, then its replies,
/// each level of reply indented by two spaces.
pub fn thread(node: &ThreadNode) -> String {
    let mut above = Vec::new();
    let mut cur = match node {
        ThreadNode::Post { parent, .. } => parent.as_deref(),
        _ => None,
    };
    while let Some(ThreadNode::Post { post, parent, .. }) = cur {
        above.push(post_indented(post, ""));
        cur = parent.as_deref();
    }
    above.reverse();
    let mut out = String::new();
    for p in above {
        out.push_str(&p);
        out.push('\n');
    }
    replies(node, 0, &mut out);
    out
}

fn replies(node: &ThreadNode, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match node {
        ThreadNode::Post {
            post, replies: r, ..
        } => {
            if depth > 0 {
                out.push('\n');
            }
            out.push_str(&post_indented(post, &pad));
            for reply in r {
                replies(reply, depth + 1, out);
            }
        }
        ThreadNode::NotFound { uri } => {
            out.push_str(&format!("\n{pad}[post not found] {uri}\n"));
        }
        _ => out.push_str(&format!("\n{pad}[post you cannot see]\n")),
    }
}

/// A notification: who did what, when, and what it is about.
pub fn notification(n: &Notification, text: Option<&str>) -> String {
    let did = match n.reason.as_str() {
        "like" => "liked your post",
        "repost" => "reposted your post",
        "follow" => "followed you",
        "mention" => "mentioned you",
        "reply" => "replied to you",
        "quote" => "quoted your post",
        other => &one_line(other),
    };
    let unread = if n.is_read { "" } else { "● " };
    let mut out = format!(
        "{unread}{} {did} · {}\n",
        who(&n.author),
        time(&n.indexed_at)
    );
    if let Some(t) = text.filter(|t| !t.is_empty()) {
        for line in plain(t).lines() {
            out.push_str(&format!("{line}\n"));
        }
    }
    let about = n.reason_subject.as_deref().unwrap_or(&n.uri);
    out.push_str(&format!("{}\n", one_line(about)));
    out
}

/// One line for an account in a list.
pub fn account_line(p: &Profile) -> String {
    let following = if p.following_uri().is_some() {
        "  (following)"
    } else {
        ""
    };
    format!("{}{following}  {}\n", who(p), one_line(&p.did))
}

/// A list: its name, what it is for and how many are in it, its
/// description, its at:// URI.
pub fn list(v: &serde_json::Value) -> String {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("");
    let purpose = match s("purpose").rsplit('#').next().unwrap_or("") {
        "curatelist" => "curation list",
        "modlist" => "moderation list",
        "referencelist" => "reference list",
        other => other,
    };
    let mut head = one_line(s("name"));
    if !purpose.is_empty() {
        head.push_str(&format!(" · {}", one_line(purpose)));
    }
    if let Some(n) = v.get("listItemCount").and_then(|n| n.as_u64()) {
        head.push_str(&format!(
            " · {n} {}",
            if n == 1 { "member" } else { "members" }
        ));
    }
    let mut out = format!("{head}\n");
    for line in plain(s("description"))
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        out.push_str(&format!("{line}\n"));
    }
    out.push_str(&format!("{}\n", one_line(s("uri"))));
    out
}

/// A profile.
pub fn profile(p: &Profile) -> String {
    let mut out = format!("{}\n{}\n", who(p), one_line(&p.did));
    out.push_str(&format!(
        "{} followers  {} following  {} posts\n",
        p.followers_count.unwrap_or(0),
        p.follows_count.unwrap_or(0),
        p.posts_count.unwrap_or(0)
    ));
    if let Some(d) = p.description.as_deref().filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        for line in plain(d).lines() {
            out.push_str(&format!("{line}\n"));
        }
    }
    out
}

/// Who a conversation is with: everyone in it but `me`.
pub fn convo_with(c: &Convo, me: &str) -> String {
    let others: Vec<String> = c.others(me).iter().map(|p| who(p)).collect();
    if others.is_empty() {
        "(just you)".to_string()
    } else {
        others.join(", ")
    }
}

/// A conversation in the list: who, how many unread, its last message.
pub fn convo(c: &Convo, me: &str) -> String {
    let unread = if c.unread_count > 0 {
        format!("  ({} new)", c.unread_count)
    } else {
        String::new()
    };
    let mut out = format!("{}{unread}\n", convo_with(c, me));
    if let Some(m) = &c.last_message {
        let who = if m.sender == me { "you: " } else { "" };
        let last = if m.deleted {
            "(deleted)".to_string()
        } else {
            plain(&m.text).lines().next().unwrap_or("").to_string()
        };
        out.push_str(&format!("  {who}{last}\n"));
    }
    out
}

/// A message: who and when, then its text.
pub fn message(m: &ChatMessage, me: &str, c: &Convo) -> String {
    let who = if m.sender == me {
        "you".to_string()
    } else {
        c.members
            .iter()
            .find(|p| p.did == m.sender)
            .map(who)
            .unwrap_or_else(|| one_line(&m.sender))
    };
    let mut out = format!("{who} · {}\n", time(&m.sent_at));
    if m.deleted {
        out.push_str("  (deleted)\n");
    } else {
        for line in plain(&m.text).lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out
}

/// The actor and record key of a bsky.app address of `kind` (`post` or
/// `feed`): `https://bsky.app/profile/<actor>/<kind>/<rkey>`.
pub fn bsky_app_path(url: &str, kind: &str) -> Option<(String, String)> {
    let rest = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.trim().strip_prefix("http://"))?;
    let rest = rest.strip_prefix("bsky.app/profile/")?;
    let rest = rest.split(['?', '#']).next()?;
    let mut parts = rest.trim_end_matches('/').split('/');
    let actor = parts.next()?;
    if parts.next()? != kind {
        return None;
    }
    let rkey = parts.next()?;
    if actor.is_empty() || rkey.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((actor.to_string(), rkey.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    #[case("https://bsky.app/profile/alice.test/post/3abc", "post", Some(("alice.test", "3abc")))]
    #[case("http://bsky.app/profile/did:plc:x/post/3abc?ref=a#b", "post", Some(("did:plc:x", "3abc")))]
    #[case("https://bsky.app/profile/alice.test/feed/cats/", "feed", Some(("alice.test", "cats")))]
    #[case("https://bsky.app/profile/alice.test/post/3abc", "feed", None)]
    #[case("https://bsky.app/profile/alice.test", "post", None)]
    #[case("https://example.com/profile/alice.test/post/3abc", "post", None)]
    #[case("https://bsky.app/profile/alice.test/post/3abc/extra", "post", None)]
    fn a_bsky_app_address_names_its_actor_and_record(
        #[case] url: &str,
        #[case] kind: &str,
        #[case] want: Option<(&str, &str)>,
    ) {
        assert_eq!(
            bsky_app_path(url, kind),
            want.map(|(a, r)| (a.to_string(), r.to_string()))
        );
    }

    fn a_post(text: &str, embed: serde_json::Value) -> Post {
        let mut v = json!({
            "uri": "at://did:plc:a/app.bsky.feed.post/1", "cid": "c",
            "author": {"did": "did:plc:a", "handle": "alice.test", "displayName": "家族👨\u{200d}👩\u{200d}👧 Alice"},
            "record": {"text": text, "createdAt": "not a time"},
            "likeCount": 2, "repostCount": 1, "replyCount": 0,
        });
        if !embed.is_null() {
            v["embed"] = embed;
        }
        serde_json::from_value(v).unwrap()
    }

    // Text is printed whole, emoji and all, one line per line, and the URI
    // closes the post so the next command can take it.
    #[test]
    fn a_post_is_printed_whole_and_ends_with_its_uri() {
        let p = a_post(
            "今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}\nمرحبا second line",
            json!({"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "t", "fullsize": "f", "alt": ""},
                {"thumb": "t2", "fullsize": "f2", "alt": ""}
            ]}),
        );
        assert_eq!(
            post(&p),
            "家族👨\u{200d}👩\u{200d}👧 Alice @alice.test · not a time\n\
             今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}\n\
             مرحبا second line\n\
             [2 pictures]\n\
             ♡ 2  ⟳ 1  ↩ 0\n\
             at://did:plc:a/app.bsky.feed.post/1\n"
        );
    }

    // What others wrote reaches the terminal as text only: an escape
    // sequence in a post, a name or a message would otherwise set the
    // clipboard (OSC 52), the window title, or clear the screen, and a line
    // break in a name would split a one-line entry in two.
    #[test]
    fn control_characters_others_wrote_do_not_reach_the_terminal() {
        let evil = "a\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J\tb\r\nc";
        let mut p = a_post(evil, serde_json::Value::Null);
        p.author.display_name = Some("Bob\u{1b}]0;title\u{7}\nEve 👨\u{200d}👩\u{200d}👧".into());
        let c: Convo = serde_json::from_value(json!({
            "id": "c", "rev": "r",
            "members": [{"did": "did:plc:me", "handle": "me.test"},
                        {"did": "did:plc:a", "handle": "alice.test", "displayName": "Al\nice"}],
            "lastMessage": {"$type": "chat.bsky.convo.defs#messageView", "id": "m", "rev": "r",
                            "text": evil, "sender": {"did": "did:plc:a"}, "sentAt": "x"},
            "muted": false, "unreadCount": 0
        }))
        .unwrap();
        let m = ChatMessage {
            text: evil.into(),
            sender: "did:plc:a".into(),
            ..Default::default()
        };
        let mut prof = p.author.clone();
        prof.description = Some(evil.into());
        let n: Notification = serde_json::from_value(json!({
            "uri": "at://x", "cid": "c", "reason": "reply", "isRead": true,
            "indexedAt": "x", "record": {},
            "author": {"did": "did:plc:b", "handle": "bob.test", "displayName": "B\u{1b}[2Job"}
        }))
        .unwrap();
        for out in [
            post(&p),
            convo(&c, "did:plc:me"),
            message(&m, "did:plc:me", &c),
            profile(&prof),
            account_line(&prof),
            notification(&n, Some(evil)),
        ] {
            assert!(!out.chars().any(|c| c.is_control() && c != '\n'), "{out:?}");
        }
        let first = post(&p);
        assert!(
            first.starts_with("Bob]0;title Eve 👨\u{200d}👩\u{200d}👧 @alice.test · "),
            "{first:?}"
        );
        assert!(
            first.contains("\na]52;c;cHduZWQ=[2J    b\nc\n"),
            "{first:?}"
        );
        assert_eq!(account_line(&prof).lines().count(), 1);
        assert!(convo(&c, "did:plc:me").starts_with("Al ice @alice.test\n"));
    }

    #[test]
    fn a_list_says_what_it_is_for_and_how_many_are_in_it() {
        let v = json!({
            "uri": "at://did:plc:me/app.bsky.graph.list/l1", "name": "Rust 🦀 仲間",
            "purpose": "app.bsky.graph.defs#curatelist", "listItemCount": 12,
            "description": "People who write Rust\n\n👨\u{200d}👩\u{200d}👧 and friends"
        });
        assert_eq!(
            list(&v),
            "Rust 🦀 仲間 · curation list · 12 members\n\
             People who write Rust\n\
             👨\u{200d}👩\u{200d}👧 and friends\n\
             at://did:plc:me/app.bsky.graph.list/l1\n"
        );
        let one = json!({"uri": "at://x", "name": "Mods", "purpose": "app.bsky.graph.defs#modlist", "listItemCount": 1});
        assert_eq!(list(&one), "Mods · moderation list · 1 member\nat://x\n");
    }

    #[test]
    fn a_profile_and_an_account_line() {
        let p: Profile = serde_json::from_value(json!({
            "did": "did:plc:b", "handle": "bob.test",
            "description": "hello 🇯🇵",
            "followersCount": 3, "followsCount": 4, "postsCount": 5,
            "viewer": {"following": "at://did:plc:me/app.bsky.graph.follow/1"}
        }))
        .unwrap();
        assert_eq!(
            profile(&p),
            "@bob.test\ndid:plc:b\n3 followers  4 following  5 posts\n\nhello 🇯🇵\n"
        );
        assert_eq!(account_line(&p), "@bob.test  (following)  did:plc:b\n");
    }
}
