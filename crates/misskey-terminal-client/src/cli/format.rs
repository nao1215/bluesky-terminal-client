//! What the commands print: plain lines for reading, no color, nothing cut,
//! each note closed by its address so it can be given to the next command;
//! and with `--json` one object per note, the same shape for every command.

use serde_json::{Value, json};

use crate::api::types::{DriveFile, Note, Notification, User};
pub(crate) use crate::clock::local_time as time;
use crate::text::{drawable, one_line};

/// The address of note `id` on the server at `origin`.
pub(crate) fn note_url(origin: &str, id: &str) -> String {
    format!("{origin}/notes/{id}")
}

/// `name @username`, `name @username@host` for an account of another
/// server, or the handle alone when there is no display name.
fn who(u: &User) -> String {
    let handle = match u.host.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => format!("@{}@{}", one_line(&u.username), one_line(h)),
        None => format!("@{}", one_line(&u.username)),
    };
    match u.name.as_deref().map(one_line) {
        Some(name) if !name.trim().is_empty() && name != u.username => format!("{name} {handle}"),
        _ => handle,
    }
}

/// A reaction as it reads: a custom emoji of the server asked, `:name@.:`,
/// as `:name:`; one of another server keeps its host.
pub(crate) fn reaction_label(key: &str) -> String {
    match key.strip_suffix("@.:") {
        Some(name) => one_line(&format!("{name}:")),
        None => one_line(key),
    }
}

/// How many kinds of reaction the text lists, the most given first. A
/// popular note gets reactions from dozens of servers' emoji, a line longer
/// than the note; `--json` lists them all, and the total counts them all.
const MAX_REACTION_KINDS: usize = 10;

/// `[2 pictures]`, `[1 video]`... one line per kind of file.
fn file_lines(files: &[DriveFile]) -> String {
    let kinds = [
        ("image/", "picture", "pictures"),
        ("video/", "video", "videos"),
        ("audio/", "sound", "sounds"),
    ];
    let mut out = String::new();
    let mut counted = 0;
    for (prefix, one, many) in kinds {
        let n = files.iter().filter(|f| f.mime.starts_with(prefix)).count();
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

/// A note: who and when, its warning and text, what it carries, its
/// reactions, its counts, its address. A renote with nothing of its own
/// shows the note it renotes, under a line that says who renoted it, as
/// bsky says who reposted a post.
pub(crate) fn note(n: &Note, origin: &str) -> String {
    let mut out = String::new();
    let shown = n.shown();
    if n.is_pure_renote() {
        out.push_str(&format!("⟳ renoted by @{}\n", one_line(&acct(&n.user))));
        if n.renote.is_none() {
            out.push_str("[renotes a note]\n");
        }
    }
    if !(n.is_pure_renote() && n.renote.is_none()) {
        out.push_str(&format!(
            "{} · {}\n",
            who(&shown.user),
            time(&shown.created_at)
        ));
    }
    if let Some(cw) = &shown.cw {
        out.push_str(&format!("CW: {}\n", one_line(cw)));
    }
    if let Some(text) = &shown.text {
        for line in drawable(text).lines() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&file_lines(&shown.files));
    if !shown.is_pure_renote() && shown.renote_id.is_some() {
        match &shown.renote {
            Some(q) => out.push_str(&format!("[quotes @{}]\n", one_line(&acct(&q.user)))),
            None => out.push_str("[quotes a note]\n"),
        }
    }
    let mut reactions: Vec<(&String, &u64)> = shown.reactions.iter().collect();
    reactions.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    if !reactions.is_empty() {
        let mut line: Vec<String> = reactions
            .iter()
            .take(MAX_REACTION_KINDS)
            .map(|(k, v)| format!("{} {v}", reaction_label(k)))
            .collect();
        if reactions.len() > MAX_REACTION_KINDS {
            line.push(format!("+{} more", reactions.len() - MAX_REACTION_KINDS));
        }
        out.push_str(&format!("{}\n", line.join("  ")));
    }
    out.push_str(&format!(
        "♡ {}  ⟳ {}  ↩ {}\n{}\n",
        shown.reaction_total(),
        shown.renote_count,
        shown.replies_count,
        one_line(&note_url(origin, &shown.id))
    ));
    out
}

/// `username` or `username@host`.
fn acct(u: &User) -> String {
    match u.host.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => format!("{}@{h}", u.username),
        None => u.username.clone(),
    }
}

/// An account as a note's `user` is printed with `--json`. `host` of an
/// account of the server asked is that server's host, where Misskey writes
/// null, so a line read on its own still says where the account is.
fn user_json(u: &User, origin: &str) -> Value {
    json!({
        "id": u.id,
        "username": u.username,
        "host": u.host.as_deref().unwrap_or(crate::api::authority(origin)),
        "name": u.name,
    })
}

/// The address of account `u` on the server at `origin`.
fn account_url(u: &User, origin: &str) -> String {
    match u.host.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => format!("{origin}/@{}@{h}", u.username),
        None => format!("{origin}/@{}", u.username),
    }
}

/// Your account, as `whoami` prints it: who, with the server's host, its
/// address, its counts and what it says about itself.
pub(crate) fn account(u: &User, origin: &str) -> String {
    let mut with_host = u.clone();
    if with_host.host.as_deref().is_none_or(str::is_empty) {
        with_host.host = Some(crate::api::authority(origin).to_string());
    }
    let mut out = format!(
        "{}\n{}\n{} followers  {} following  {} notes\n",
        who(&with_host),
        one_line(&account_url(u, origin)),
        u.followers_count.unwrap_or(0),
        u.following_count.unwrap_or(0),
        u.notes_count.unwrap_or(0),
    );
    if let Some(d) = u.description.as_deref().filter(|d| !d.trim().is_empty()) {
        out.push('\n');
        for line in drawable(d).lines() {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// An account on one line, as `follows` and `followers` print it, and as
/// bsky's do: who, whether you follow it, and its address.
pub(crate) fn account_line(u: &User, origin: &str) -> String {
    let mut with_host = u.clone();
    if with_host.host.as_deref().is_none_or(str::is_empty) {
        with_host.host = Some(crate::api::authority(origin).to_string());
    }
    let following = if u.is_following { "  (following)" } else { "" };
    format!(
        "{}{following}  {}\n",
        who(&with_host),
        one_line(&account_url(u, origin))
    )
}

/// Your account as `whoami --json` prints it: a note's `user` with its
/// address, what it says about itself and its counts.
pub(crate) fn account_json(u: &User, origin: &str) -> Value {
    let mut v = user_json(u, origin);
    v["url"] = json!(account_url(u, origin));
    v["description"] = json!(u.description);
    v["followersCount"] = json!(u.followers_count);
    v["followingCount"] = json!(u.following_count);
    v["notesCount"] = json!(u.notes_count);
    v
}

/// What a notification says happened.
fn happened(n: &Notification) -> String {
    match n.kind.as_str() {
        "follow" => "followed you".into(),
        "mention" => "mentioned you".into(),
        "reply" => "replied to you".into(),
        "renote" => "renoted your note".into(),
        "quote" => "quoted your note".into(),
        "reaction" => format!(
            "reacted {} to your note",
            reaction_label(n.reaction.as_deref().unwrap_or("?"))
        ),
        "pollEnded" => "a poll has ended".into(),
        "receiveFollowRequest" => "asked to follow you".into(),
        "followRequestAccepted" => "accepted your follow request".into(),
        "achievementEarned" => format!(
            "achievement earned: {}",
            one_line(n.achievement.as_deref().unwrap_or(""))
        ),
        "app" => one_line(
            &[n.header.as_deref(), n.body.as_deref()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(": "),
        ),
        other => one_line(other),
    }
}

/// A notification: who did what and when, the note it is about (its
/// warning and text), and the address to go on from: the note's, else the
/// account's.
pub(crate) fn notification(n: &Notification, origin: &str) -> String {
    let did = happened(n);
    let mut out = match &n.user {
        Some(u) => format!("{} {did} · {}\n", who(u), time(&n.created_at)),
        None => format!("{did} · {}\n", time(&n.created_at)),
    };
    if let Some(note) = &n.note {
        let shown = note.shown();
        if let Some(cw) = &shown.cw {
            out.push_str(&format!("CW: {}\n", one_line(cw)));
        }
        if let Some(text) = &shown.text {
            for line in drawable(text).lines() {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.push_str(&file_lines(&shown.files));
        out.push_str(&format!("{}\n", one_line(&note_url(origin, &shown.id))));
    } else if let Some(u) = &n.user {
        out.push_str(&format!("{}\n", one_line(&account_url(u, origin))));
    }
    out
}

/// A notification as `notifications --json` prints it: what happened, who
/// did it (a note's `user`, or null), the reaction, and the note it is
/// about in the shape every command prints (or null).
pub(crate) fn notification_json(n: &Notification, origin: &str) -> Value {
    json!({
        "id": n.id,
        "createdAt": n.created_at,
        "type": n.kind,
        "user": n.user.as_ref().map(|u| user_json(u, origin)),
        "reaction": n.reaction,
        "note": n.note.as_ref().map(|x| note_json(x, origin)),
    })
}

/// A note as `--json` prints it, its `user` as [`user_json`] puts it.
pub(crate) fn note_json(n: &Note, origin: &str) -> Value {
    let files: Vec<Value> = n
        .files
        .iter()
        .map(|f| {
            json!({
                "id": f.id,
                "name": f.name,
                "url": f.url,
                "type": f.mime,
                "size": f.size,
                "isSensitive": f.is_sensitive,
            })
        })
        .collect();
    json!({
        "id": n.id,
        "createdAt": n.created_at,
        "url": note_url(origin, &n.id),
        "uri": n.uri,
        "user": user_json(&n.user, origin),
        "text": n.text,
        "cw": n.cw,
        "reactionCount": n.reaction_total(),
        "reactions": n.reactions,
        "renoteCount": n.renote_count,
        "repliesCount": n.replies_count,
        "files": files,
        "replyId": n.reply_id,
        "renoteId": n.renote_id,
        "renote": n.renote.as_deref().map(|r| note_json(r, origin)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    const ORIGIN: &str = "https://misskey.test";

    fn a_note(v: Value) -> Note {
        serde_json::from_value(v).unwrap()
    }

    // Text is printed whole, emoji, CJK and right-to-left text alike, one
    // line per line, and the address closes the note so the next command
    // can take it.
    #[test]
    fn a_note_is_printed_whole_and_ends_with_its_address() {
        let n = a_note(json!({
            "id": "9abc", "createdAt": "not a time",
            "user": {"username": "alice", "host": null, "name": "家族👨\u{200d}👩\u{200d}👧 Alice"},
            "text": "今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}\nمرحبا second line",
            "reactions": {"👍": 3, ":blobcat@.:": 3, ":x@other.example:": 1},
            "renoteCount": 1, "repliesCount": 2,
            "files": [
                {"type": "image/png", "size": 1}, {"type": "image/jpeg", "size": 1},
                {"type": "video/mp4", "size": 1}, {"type": "application/pdf", "size": 1}
            ]
        }));
        assert_eq!(
            note(&n, ORIGIN),
            "家族👨\u{200d}👩\u{200d}👧 Alice @alice · not a time\n\
             今日は👍🏽 1️⃣ ❤️ 🇯🇵 e\u{301}\n\
             مرحبا second line\n\
             [2 pictures]\n\
             [1 video]\n\
             [1 file]\n\
             :blobcat: 3  👍 3  :x@other.example: 1\n\
             ♡ 7  ⟳ 1  ↩ 2\n\
             https://misskey.test/notes/9abc\n"
        );
    }

    #[test]
    fn a_pure_renote_shows_the_note_it_renotes_under_who_renoted_it() {
        let n = a_note(json!({
            "id": "rn1", "createdAt": "t1", "user": {"username": "alice"},
            "renoteId": "orig",
            "renote": {"id": "orig", "createdAt": "t0", "user": {"username": "bob", "host": "other.example"},
                       "cw": "ネタバレ", "text": "本文"}
        }));
        assert_eq!(
            note(&n, ORIGIN),
            "⟳ renoted by @alice\n\
             @bob@other.example · t0\n\
             CW: ネタバレ\n\
             本文\n\
             ♡ 0  ⟳ 0  ↩ 0\n\
             https://misskey.test/notes/orig\n"
        );
        let quote = a_note(json!({
            "id": "q", "createdAt": "t", "user": {"username": "alice"}, "text": "見て",
            "renoteId": "orig", "renote": {"id": "orig", "user": {"username": "bob", "host": "other.example"}}
        }));
        assert!(note(&quote, ORIGIN).contains("見て\n[quotes @bob@other.example]\n"));
    }

    // What others wrote reaches the terminal as text only: an escape
    // sequence in a note, a name or a reaction would otherwise set the
    // clipboard (OSC 52), the window title, or clear the screen.
    #[test]
    fn control_characters_others_wrote_do_not_reach_the_terminal() {
        let evil = "a\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J\tb\r\nc";
        let n = a_note(json!({
            "id": "x", "createdAt": evil,
            "user": {"username": "eve", "name": "Eve\u{1b}]0;title\u{7}\n👨\u{200d}👩\u{200d}👧"},
            "text": evil, "cw": evil, "reactions": {evil: 1}
        }));
        let out = note(&n, ORIGIN);
        assert!(!out.chars().any(|c| c.is_control() && c != '\n'), "{out:?}");
        assert!(
            out.starts_with("Eve]0;title 👨\u{200d}👩\u{200d}👧 @eve · "),
            "{out:?}"
        );
        assert!(out.contains("\na]52;c;cHduZWQ=[2J    b\nc\n"), "{out:?}");
    }

    #[test]
    fn the_text_lists_the_ten_reactions_most_given() {
        let reactions: serde_json::Map<String, Value> = (1..=12)
            .map(|i| (format!(":r{i:02}@.:"), json!(i)))
            .collect();
        let n = a_note(json!({"id": "x", "reactions": reactions}));
        let out = note(&n, ORIGIN);
        assert!(
            out.contains("\n:r12: 12  :r11: 11  :r10: 10  :r09: 9  :r08: 8  :r07: 7  :r06: 6  :r05: 5  :r04: 4  :r03: 3  +2 more\n♡ 78  "),
            "{out}"
        );
    }

    #[rstest]
    #[case("👍", "👍")]
    #[case(":blobcat@.:", ":blobcat:")]
    #[case(":blobcat@other.example:", ":blobcat@other.example:")]
    #[case("❤️", "❤️")]
    fn a_reaction_reads_as_it_is_given(#[case] key: &str, #[case] want: &str) {
        assert_eq!(reaction_label(key), want);
    }

    // Every command prints this one shape; an account of the server asked
    // gets its host filled in, and the counts are numbers.
    #[test]
    fn the_json_of_a_note_has_one_shape() {
        let n = a_note(json!({
            "id": "9abc", "createdAt": "2026-09-20T10:00:00.000Z",
            "user": {"id": "u1", "username": "alice", "host": null, "name": "Alice"},
            "text": "🐈", "cw": null, "reactions": {"👍": 2, ":a@.:": 1},
            "renoteCount": 4, "repliesCount": 5,
            "files": [{"id": "f1", "name": "猫.png", "type": "image/png", "size": 10, "url": "https://cdn.test/f1.png"}],
            "renoteId": "r", "renote": {"id": "r", "user": {"username": "bob", "host": "other.example"}}
        }));
        let v = note_json(&n, ORIGIN);
        assert_eq!(v["url"], "https://misskey.test/notes/9abc");
        assert_eq!(v["user"]["host"], "misskey.test");
        assert_eq!(v["reactionCount"], 3);
        assert_eq!(v["reactions"][":a@.:"], 1);
        assert_eq!(v["files"][0]["type"], "image/png");
        assert_eq!(v["files"][0]["size"], 10);
        assert_eq!(v["renote"]["user"]["host"], "other.example");
        assert_eq!(v["renote"]["url"], "https://misskey.test/notes/r");
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "createdAt",
                "cw",
                "files",
                "id",
                "reactionCount",
                "reactions",
                "renote",
                "renoteCount",
                "renoteId",
                "repliesCount",
                "replyId",
                "text",
                "uri",
                "url",
                "user"
            ]
        );
    }

    fn a_notification(v: Value) -> Notification {
        serde_json::from_value(v).unwrap()
    }

    // Who did what, and the note it is about with the address to go on
    // from; a renote shows your note, a follow the account's address.
    #[test]
    fn a_notification_says_who_did_what_to_which_note() {
        let reaction = a_notification(json!({
            "id": "n1", "createdAt": "t", "type": "reaction", "reaction": ":blobcat@.:",
            "user": {"username": "bob", "host": "other.example", "name": "Bob 🐈‍⬛"},
            "note": {"id": "9abc", "text": "朝ごはん 🍙\n二行目", "files": [{"type": "image/png"}]}
        }));
        assert_eq!(
            notification(&reaction, ORIGIN),
            "Bob 🐈‍⬛ @bob@other.example reacted :blobcat: to your note · t\n\
             朝ごはん 🍙\n\
             二行目\n\
             [1 picture]\n\
             https://misskey.test/notes/9abc\n"
        );
        let renote = a_notification(json!({
            "id": "n2", "createdAt": "t", "type": "renote", "user": {"username": "carol"},
            "note": {"id": "rn", "renoteId": "mine", "renote": {"id": "mine", "cw": "ネタバレ", "text": "私の"}}
        }));
        assert_eq!(
            notification(&renote, ORIGIN),
            "@carol renoted your note · t\nCW: ネタバレ\n私の\nhttps://misskey.test/notes/mine\n"
        );
        let follow = a_notification(json!({
            "id": "n3", "createdAt": "t", "type": "follow", "user": {"username": "dave", "host": "x.example"}
        }));
        assert_eq!(
            notification(&follow, ORIGIN),
            "@dave@x.example followed you · t\nhttps://misskey.test/@dave@x.example\n"
        );
        let earned = a_notification(json!({
            "id": "n4", "createdAt": "t", "type": "achievementEarned", "achievement": "notes1"
        }));
        assert_eq!(
            notification(&earned, ORIGIN),
            "achievement earned: notes1 · t\n"
        );
        let unknown = a_notification(json!({"id": "n5", "createdAt": "t", "type": "new\u{1b}[2J"}));
        assert_eq!(notification(&unknown, ORIGIN), "new[2J · t\n");
    }

    // The JSON of a notification has one shape; its note is a note as
    // every command prints it, and its account a note's `user`.
    #[test]
    fn the_json_of_a_notification_has_one_shape() {
        let n = a_notification(json!({
            "id": "n1", "createdAt": "2026-11-26T10:00:00.000Z", "type": "reply",
            "user": {"id": "u2", "username": "bob", "host": null},
            "note": {"id": "9abc", "text": "返信 🇯🇵", "replyId": "8x", "user": {"username": "bob"}}
        }));
        let v = notification_json(&n, ORIGIN);
        assert_eq!(
            v["user"],
            json!({"id": "u2", "username": "bob", "host": "misskey.test", "name": null})
        );
        assert_eq!(v["note"], note_json(n.note.as_ref().unwrap(), ORIGIN));
        assert_eq!(v["reaction"], Value::Null);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            ["createdAt", "id", "note", "reaction", "type", "user"]
        );
        let bare = notification_json(&a_notification(json!({"id": "n2", "type": "app"})), ORIGIN);
        assert_eq!(
            (bare["user"].clone(), bare["note"].clone()),
            (Value::Null, Value::Null)
        );
    }

    // whoami names the account with the server's host, its address, its
    // counts and what it says about itself, as text only.
    #[test]
    fn whoami_shows_the_account_with_its_host_and_counts() {
        let u: User = serde_json::from_value(json!({
            "id": "u1", "username": "alice", "host": null, "name": "アリス 👩‍💻",
            "description": "こんにちは\u{1b}]52;c;eA==\u{7}\n二行目",
            "followersCount": 12, "followingCount": 34, "notesCount": 56
        }))
        .unwrap();
        assert_eq!(
            account(&u, ORIGIN),
            "アリス 👩‍💻 @alice@misskey.test\n\
             https://misskey.test/@alice\n\
             12 followers  34 following  56 notes\n\
             \n\
             こんにちは]52;c;eA==\n\
             二行目\n"
        );
        let v = account_json(&u, ORIGIN);
        assert_eq!(v["host"], "misskey.test");
        assert_eq!(v["url"], "https://misskey.test/@alice");
        assert_eq!(v["notesCount"], 56);
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "description",
                "followersCount",
                "followingCount",
                "host",
                "id",
                "name",
                "notesCount",
                "url",
                "username"
            ]
        );
    }
}
