//! The text the commands print: plain lines for reading, no color, nothing
//! cut. A post ends with its at:// URI so it can be given to the next
//! command.

use chrono::{DateTime, Local};

use crate::api::types::{ChatMessage, Convo, Media, Notification, Post, Profile, ThreadNode};

/// A timestamp as local time, `2026-09-20 10:00`; one that does not parse
/// is shown as it is.
pub fn time(ts: &str) -> String {
    match DateTime::parse_from_rfc3339(ts) {
        Ok(t) => t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => ts.to_string(),
    }
}

/// `name @handle`, or `@handle` when there is no display name.
fn who(p: &Profile) -> String {
    let name = p.name();
    if name == p.handle {
        format!("@{}", p.handle)
    } else {
        format!("{name} @{}", p.handle)
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
    for line in record.text.lines() {
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
                .map(|h| format!("[quotes @{h}]"))
                .unwrap_or_else(|| "[quotes a post]".to_string());
            out.push_str(&format!("{pad}{quoted}\n"));
        }
    }
    for link in p.links().into_iter().take(1) {
        out.push_str(&format!("{pad}{link}\n"));
    }
    out.push_str(&format!(
        "{pad}♡ {}  ⟳ {}  ↩ {}\n{pad}{}\n",
        p.like_count, p.repost_count, p.reply_count, p.uri
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
        other => other,
    };
    let unread = if n.is_read { "" } else { "● " };
    let mut out = format!(
        "{unread}{} {did} · {}\n",
        who(&n.author),
        time(&n.indexed_at)
    );
    if let Some(t) = text.filter(|t| !t.is_empty()) {
        for line in t.lines() {
            out.push_str(&format!("{line}\n"));
        }
    }
    let about = n.reason_subject.as_deref().unwrap_or(&n.uri);
    out.push_str(&format!("{about}\n"));
    out
}

/// One line for an account in a list.
pub fn account_line(p: &Profile) -> String {
    let following = if p.following_uri().is_some() {
        "  (following)"
    } else {
        ""
    };
    format!("{}{following}  {}\n", who(p), p.did)
}

/// A profile.
pub fn profile(p: &Profile) -> String {
    let mut out = format!("{}\n{}\n", who(p), p.did);
    out.push_str(&format!(
        "{} followers  {} following  {} posts\n",
        p.followers_count.unwrap_or(0),
        p.follows_count.unwrap_or(0),
        p.posts_count.unwrap_or(0)
    ));
    if let Some(d) = p.description.as_deref().filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        for line in d.lines() {
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
        let text = if m.deleted {
            "(deleted)"
        } else {
            m.text.lines().next().unwrap_or("")
        };
        out.push_str(&format!("  {who}{text}\n"));
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
            .unwrap_or_else(|| m.sender.clone())
    };
    let mut out = format!("{who} · {}\n", time(&m.sent_at));
    if m.deleted {
        out.push_str("  (deleted)\n");
    } else {
        for line in m.text.lines() {
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
