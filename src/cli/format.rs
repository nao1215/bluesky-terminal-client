//! What the commands print: plain lines for reading, no color, nothing cut,
//! each note closed by its address so it can be given to the next command;
//! and with `--json` one object per note, the same shape for every command.

use serde_json::{Value, json};

use crate::api::types::{DriveFile, Note, User};
pub use crate::clock::local_time as time;
use crate::text::{drawable, one_line};

/// The address of note `id` on the server at `origin`.
pub fn note_url(origin: &str, id: &str) -> String {
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
fn reaction_label(key: &str) -> String {
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
/// shows the note it renotes, under a line that says who renoted it.
pub fn note(n: &Note, origin: &str) -> String {
    let mut out = String::new();
    let shown = n.shown();
    if n.is_pure_renote() {
        out.push_str(&format!(
            "⟳ {} renoted · {}\n",
            who(&n.user),
            time(&n.created_at)
        ));
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

/// A note as `--json` prints it. `host` of an account of the server asked
/// is that server's host, where Misskey writes null, so a line read on its
/// own still says where the account is.
pub fn note_json(n: &Note, origin: &str) -> Value {
    let server = crate::api::authority(origin);
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
        "user": {
            "id": n.user.id,
            "username": n.user.username,
            "host": n.user.host.as_deref().unwrap_or(server),
            "name": n.user.name,
        },
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
            "⟳ @alice renoted · t1\n\
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
}
