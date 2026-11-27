//! The Misskey side of the rest of bsky's commands: a note's thread, a
//! profile, who reacted to a note and who renoted it (bsky's `likes` and
//! `reposts`), reacting (bsky's `like`), renoting (`repost`), deleting your
//! own note, and muting and blocking accounts with their lists. Each takes
//! bsky's name and arguments; `--json` prints the shapes the other commands
//! print.

use super::*;
use crate::api::types::User;
use crate::cli::format;

/// The reaction `like` gives without `--reaction`: Misskey's own like.
pub const LIKE: &str = "❤";

/// `msky thread NOTE`: what the note answers (the first note first), the
/// note, and the replies to it.
pub fn thread(ctx: &Ctx, out: &mut dyn Write, note: &str) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref())?;
    let n = client.note(&id)?;
    let mut all = if n.reply_id.is_some() {
        let mut up = client.conversation(&id, api::MAX_PAGE)?;
        up.reverse();
        up
    } else {
        Vec::new()
    };
    let replies = if n.replies_count > 0 {
        client.children(&id, api::MAX_PAGE)?
    } else {
        Vec::new()
    };
    all.push(n);
    all.extend(replies);
    print_notes(ctx, out, client.origin(), &all)
}

/// The account `actor` names, read on its server or the one `--host` does.
fn account(ctx: &Ctx, actor: &str) -> Result<(Client, User)> {
    let who = parse_user(actor)?;
    let client = ctx.client(who.origin.as_deref())?;
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let u = client.user(&who.username, host)?;
    Ok((client, u))
}

/// `msky profile [ACTOR]`: an account as `whoami` prints yours; yours
/// without ACTOR.
pub fn profile(ctx: &Ctx, out: &mut dyn Write, actor: Option<&str>) -> Result<()> {
    let (client, u) = match actor {
        Some(a) => account(ctx, a)?,
        None => {
            let client = ctx.account_client()?;
            let me = client.me().map_err(|e| ctx.explained(e))?;
            (client, me)
        }
    };
    if ctx.json {
        return json_line(out, &format::account_json(&u, client.origin()));
    }
    text(out, &format::account(&u, client.origin()))
}

/// `msky reactions NOTE` (bsky's `likes`): who reacted, and with what.
pub fn reactions(ctx: &Ctx, out: &mut dyn Write, note: &str, limit: usize) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref())?;
    let all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.reactions(&id, n, until),
    )?;
    for r in &all {
        if ctx.json {
            json_line(
                out,
                &json!({
                    "reaction": r.reaction,
                    "createdAt": r.created_at,
                    "user": format::account_json(&r.user, client.origin()),
                }),
            )?;
        } else {
            text(
                out,
                &format!(
                    "{}  {}",
                    crate::text::one_line(&format::reaction_label(&r.reaction)),
                    format::account_line(&r.user, client.origin())
                ),
            )?;
        }
    }
    Ok(())
}

impl Paged for crate::api::types::Reaction {
    fn id(&self) -> &str {
        &self.id
    }
}

/// `msky renotes NOTE` (bsky's `reposts`): the renotes of a note.
pub fn renotes(ctx: &Ctx, out: &mut dyn Write, note: &str, limit: usize) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref())?;
    let all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.renotes(&id, n, until),
    )?;
    print_notes(ctx, out, client.origin(), &all)
}

/// The note `note` names on the server written to: an address of another
/// server is refused, as `follow` refuses one.
fn own_server_note(ctx: &Ctx, note: &str) -> Result<String> {
    let (origin, id) = parse_note(note)?;
    if let Some(o) = origin.as_deref().filter(|o| *o != ctx.origin) {
        return Err(Error::usage(format!(
            "the note is given by its address on {}, and msky writes on {}",
            api::authority(o),
            api::authority(&ctx.origin)
        ))
        .with_hint("give its ID on the server you write on"));
    }
    Ok(id)
}

/// Print what a write did: `said` as text, `v` with `--json`.
fn did(ctx: &Ctx, out: &mut dyn Write, v: Value, said: String) -> Result<()> {
    if ctx.json {
        json_line(out, &v)
    } else {
        text(out, &format!("{said}\n"))
    }
}

/// `msky like NOTE` (bsky's `like`): react to the note, ❤ unless
/// `--reaction` names another; a reaction given before is replaced.
pub fn like(ctx: &Ctx, out: &mut dyn Write, note: &str, reaction: Option<&str>) -> Result<()> {
    let id = own_server_note(ctx, note)?;
    let reaction = reaction
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .unwrap_or(LIKE);
    let client = ctx.writer(&["write:reactions"], "react to notes")?;
    let n = client.note(&id).map_err(|e| ctx.explained(e))?;
    if n.my_reaction.as_deref() == Some(reaction) {
        return did(
            ctx,
            out,
            json!({"id": id, "reaction": reaction, "already": true}),
            format!("reacted {reaction} already"),
        );
    }
    if n.my_reaction.is_some() {
        client.unreact(&id).map_err(|e| ctx.explained(e))?;
    }
    client.react(&id, reaction).map_err(|e| ctx.explained(e))?;
    did(
        ctx,
        out,
        json!({"id": id, "reaction": reaction}),
        format!(
            "reacted {reaction} to {}",
            format::note_url(client.origin(), &id)
        ),
    )
}

/// `msky unlike NOTE`: take back your reaction to the note.
pub fn unlike(ctx: &Ctx, out: &mut dyn Write, note: &str) -> Result<()> {
    let id = own_server_note(ctx, note)?;
    let client = ctx.writer(&["write:reactions"], "react to notes")?;
    client.unreact(&id).map_err(|e| ctx.explained(e))?;
    did(
        ctx,
        out,
        json!({"deleted": id}),
        format!(
            "took back the reaction to {}",
            format::note_url(client.origin(), &id)
        ),
    )
}

/// `msky repost NOTE` (bsky's `repost`): renote the note, public.
pub fn repost(ctx: &Ctx, out: &mut dyn Write, note: &str, on: bool) -> Result<()> {
    let id = own_server_note(ctx, note)?;
    let client = ctx.writer(&["write:notes"], "renote")?;
    if on {
        let made = client
            .create_note(&json!({"renoteId": id, "visibility": "public"}))
            .map_err(|e| ctx.explained(e))?;
        let url = format::note_url(client.origin(), &made.id);
        return did(ctx, out, json!({"uri": url, "id": made.id}), url);
    }
    client.unrenote(&id).map_err(|e| ctx.explained(e))?;
    did(
        ctx,
        out,
        json!({"deleted": id}),
        format!(
            "took back the renotes of {}",
            format::note_url(client.origin(), &id)
        ),
    )
}

/// `msky delete NOTE`: delete one of your own notes. The server refuses
/// another's.
pub fn delete(ctx: &Ctx, out: &mut dyn Write, note: &str) -> Result<()> {
    let id = own_server_note(ctx, note)?;
    let client = ctx.writer(&["write:notes"], "delete notes")?;
    client.delete_note(&id).map_err(|e| ctx.explained(e))?;
    did(
        ctx,
        out,
        json!({"deleted": id}),
        format!("deleted {}", format::note_url(client.origin(), &id)),
    )
}

/// What `mute` and `block` do, and how they say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    Mute,
    Block,
}

impl Guard {
    fn endpoint(self, on: bool) -> &'static str {
        match (self, on) {
            (Guard::Mute, true) => "mute/create",
            (Guard::Mute, false) => "mute/delete",
            (Guard::Block, true) => "blocking/create",
            (Guard::Block, false) => "blocking/delete",
        }
    }

    fn permission(self, write: bool) -> &'static str {
        match (self, write) {
            (Guard::Mute, true) => "write:mutes",
            (Guard::Mute, false) => "read:mutes",
            (Guard::Block, true) => "write:blocks",
            (Guard::Block, false) => "read:blocks",
        }
    }

    fn verb(self, on: bool) -> &'static str {
        match (self, on) {
            (Guard::Mute, true) => "muted",
            (Guard::Mute, false) => "unmuted",
            (Guard::Block, true) => "blocked",
            (Guard::Block, false) => "unblocked",
        }
    }
}

/// `msky mute ACTOR`, `unmute`, `block`, `unblock`.
pub fn guard(ctx: &Ctx, out: &mut dyn Write, actor: &str, what: Guard, on: bool) -> Result<()> {
    let who = parse_user(actor)?;
    if who.origin.as_deref().is_some_and(|o| o != ctx.origin) {
        return Err(
            Error::usage("the account is given by its address on another server")
                .with_hint("give it as @name@host"),
        );
    }
    let verb = what.verb(on);
    let doing = match what {
        Guard::Mute => "mute accounts",
        Guard::Block => "block accounts",
    };
    let client = ctx.writer(&[what.permission(true)], doing)?;
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let u = client
        .user(&who.username, host)
        .map_err(|e| ctx.explained(e))?;
    client
        .account_write(what.endpoint(on), &u.id)
        .map_err(|e| ctx.explained(e))?;
    let shown = match u.host.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => format!("@{}@{h}", crate::text::one_line(&u.username)),
        None => format!("@{}@{}", crate::text::one_line(&u.username), client.host()),
    };
    let key = if on { "id" } else { "deleted" };
    did(ctx, out, json!({ key: u.id }), format!("{verb} {shown}"))
}

/// `msky mutes`, `msky blocks`: the accounts you muted or blocked.
pub fn guarded(ctx: &Ctx, out: &mut dyn Write, what: Guard, limit: usize) -> Result<()> {
    let client = ctx.writer(&[what.permission(false)], "read whom you mute and block")?;
    let endpoint = match what {
        Guard::Mute => "mute/list",
        Guard::Block => "blocking/list",
    };
    let all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.relations(endpoint, json!({}), n, until),
    )
    .map_err(|e| ctx.explained(e))?;
    for r in &all {
        let Some(u) = r.mutee.as_ref().or(r.blockee.as_ref()) else {
            continue;
        };
        if ctx.json {
            json_line(out, &format::account_json(u, client.origin()))?;
        } else {
            text(out, &format::account_line(u, client.origin()))?;
        }
    }
    Ok(())
}

/// Why something is reported, as bsky's `--reason`: Misskey's report has
/// no kinds, so the reason starts the comment the moderators read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportReason {
    Spam,
    Violation,
    Misleading,
    Sexual,
    Rude,
    Other,
}

impl ReportReason {
    pub const ALL: [ReportReason; 6] = [
        ReportReason::Spam,
        ReportReason::Violation,
        ReportReason::Misleading,
        ReportReason::Sexual,
        ReportReason::Rude,
        ReportReason::Other,
    ];

    /// How the comment starts.
    pub fn label(self) -> &'static str {
        match self {
            ReportReason::Spam => crate::i18n::n!("Spam"),
            ReportReason::Violation => crate::i18n::n!("Breaks the server's rules or the law"),
            ReportReason::Misleading => crate::i18n::n!("Misleading"),
            ReportReason::Sexual => crate::i18n::n!("Sexual content not marked as sensitive"),
            ReportReason::Rude => crate::i18n::n!("Rude or harassing"),
            ReportReason::Other => crate::i18n::n!("Other"),
        }
    }
}

/// The most a report's comment may hold (`users/report-abuse`).
pub const MAX_REPORT_CHARS: usize = 2048;

/// The comment sent with a report: the reason, the note's address when a
/// note is reported (as Misskey's own form writes it), and what the user
/// wrote. Too long is refused before anything is sent.
pub fn report_comment(
    reason: ReportReason,
    note_url: Option<&str>,
    comment: Option<&str>,
) -> Result<String> {
    let mut c = reason.label().to_string();
    if let Some(u) = note_url {
        c.push_str(&format!("\nNote: {u}"));
    }
    if let Some(t) = comment.map(str::trim).filter(|t| !t.is_empty()) {
        c.push_str("\n-----\n");
        c.push_str(t);
    }
    let n = c.chars().count();
    if n > MAX_REPORT_CHARS {
        return Err(Error::usage(format!(
            "the report is {n} characters long; it can have {MAX_REPORT_CHARS}"
        )));
    }
    Ok(c)
}

/// `msky report TARGET --reason R` (bsky's `report`): report an account,
/// or a note's author with the note's address, to the moderators of the
/// server written on. It is sent only once confirmed: `--yes`, or `y` to
/// the question on a terminal.
#[expect(
    clippy::too_many_arguments,
    reason = "each is one of the command's options, or where it reads and asks"
)]
pub fn report(
    ctx: &Ctx,
    out: &mut dyn Write,
    target: &str,
    reason: ReportReason,
    comment: Option<&str>,
    yes: bool,
    input: &mut dyn std::io::BufRead,
    asked: Option<&mut dyn Write>,
) -> Result<()> {
    let t = target.trim();
    let is_note = !t.starts_with('@') && (t.contains("/notes/") || !t.contains('@'));
    let client = ctx.writer(&["write:report-abuse"], "report")?;
    let (user_id, shown, note_url) = if is_note {
        let id = own_server_note(ctx, t)?;
        let n = client.note(&id).map_err(|e| ctx.explained(e))?;
        let url = format::note_url(client.origin(), &n.id);
        let who = acct_of(&n.user, client.host());
        (n.user.id.clone(), format!("{who} for {url}"), Some(url))
    } else {
        let who = parse_user(t)?;
        if who.origin.as_deref().is_some_and(|o| o != ctx.origin) {
            return Err(
                Error::usage("the account is given by its address on another server")
                    .with_hint("give it as @name@host"),
            );
        }
        let host = who
            .host
            .as_deref()
            .filter(|h| !h.eq_ignore_ascii_case(client.host()));
        let u = client
            .user(&who.username, host)
            .map_err(|e| ctx.explained(e))?;
        (u.id.clone(), acct_of(&u, client.host()), None)
    };
    let body = report_comment(reason, note_url.as_deref(), comment)?;
    if !yes {
        let Some(err) = asked else {
            return Err(Error::usage("a report is sent only once confirmed")
                .with_hint("pass --yes, or run msky report on a terminal to answer y"));
        };
        let question = format!(
            "report {} to the moderators of {} as \"{}\"? [y/N] ",
            crate::text::one_line(&shown),
            client.host(),
            reason.label()
        );
        err.write_all(question.as_bytes())
            .and_then(|()| err.flush())
            .map_err(|e| Error::io(format!("cannot ask: {e}")))?;
        let mut answer = String::new();
        input
            .read_line(&mut answer)
            .map_err(|e| Error::io(format!("cannot read the answer: {e}")))?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            return Err(Error::usage("not reported"));
        }
    }
    client
        .report(&user_id, &body)
        .map_err(|e| ctx.explained(e))?;
    did(
        ctx,
        out,
        json!({"reported": user_id, "comment": body}),
        format!("reported {}", crate::text::one_line(&shown)),
    )
}

/// A chat message as `--json` prints it: who sent it and to whom (or to
/// which room), in the account shape the other commands print.
fn chat_json(m: &crate::api::types::ChatMessage, origin: &str) -> Value {
    json!({
        "id": m.id,
        "createdAt": m.created_at,
        "from": m.from_user.as_ref().map(|u| format::account_json(u, origin)),
        "to": m.to_user.as_ref().map(|u| format::account_json(u, origin)),
        "room": m.to_room.as_ref().map(|r| json!({"id": r.id, "name": r.name})),
        "text": m.text,
    })
}

/// A chat message as text: who, when, and what it says.
fn chat_text(m: &crate::api::types::ChatMessage) -> String {
    let who = m.from_user.as_ref().map_or_else(
        || m.from_user_id.clone(),
        |u| match u.host.as_deref().filter(|h| !h.is_empty()) {
            Some(h) => format!("@{}@{h}", u.username),
            None => format!("@{}", u.username),
        },
    );
    let mut out = format!(
        "{} · {}\n",
        crate::text::one_line(&who),
        format::time(&m.created_at)
    );
    if let Some(t) = &m.text {
        for line in crate::text::drawable(t).lines() {
            out.push_str(line);
            out.push('\n');
        }
    }
    if let Some(f) = &m.file {
        out.push_str(&format!("[{}]\n", crate::text::one_line(&f.name)));
    }
    out
}

/// `msky chat` (bsky's): the conversations; with ACTOR the messages with
/// them, oldest first; with TEXT too, send it (`-` reads it from stdin).
pub fn chat(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    message: Option<String>,
    limit: usize,
    stdin: &mut dyn std::io::Read,
) -> Result<()> {
    let client = ctx.account_client()?;
    let explained = |e: Error| {
        if api::chat_unavailable(&e) {
            let host = client.host().to_string();
            e.with_hint(format!("{host} has no chat for this account (a version before chat, or a role that may not chat)"))
        } else {
            ctx.explained(e)
        }
    };
    let Some(actor) = actor else {
        let mut all = client.chat_history(false, limit).map_err(explained)?;
        if let Ok(rooms) = client.chat_history(true, limit) {
            all.extend(rooms);
        }
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        all.truncate(limit);
        for m in &all {
            if ctx.json {
                json_line(out, &chat_json(m, client.origin()))?;
            } else {
                text(out, &chat_text(m))?;
            }
        }
        return Ok(());
    };
    let who = parse_user(actor)?;
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let u = client
        .user(&who.username, host)
        .map_err(|e| ctx.explained(e))?;
    if let Some(m) = message {
        let body = if m == "-" {
            let mut s = String::new();
            stdin
                .read_to_string(&mut s)
                .map_err(|e| Error::io(format!("cannot read the message from stdin: {e}")))?;
            s
        } else {
            m
        };
        let body = body.trim();
        if body.is_empty() {
            return Err(Error::usage("the message is empty"));
        }
        let client = ctx.writer(&["write:chat"], "send messages")?;
        let sent = client
            .chat_send(Some(&u.id), None, body)
            .map_err(explained)?;
        if ctx.json {
            return json_line(out, &chat_json(&sent, client.origin()));
        }
        return text(
            out,
            &format!(
                "sent to {}\n",
                crate::text::one_line(&acct_of(&u, client.host()))
            ),
        );
    }
    let mut all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.chat_timeline(Some(&u.id), None, n, until),
    )
    .map_err(explained)?;
    all.reverse();
    for (i, m) in all.iter().enumerate() {
        if ctx.json {
            json_line(out, &chat_json(m, client.origin()))?;
        } else {
            if i > 0 {
                text(out, "\n")?;
            }
            text(out, &chat_text(m))?;
        }
    }
    Ok(())
}

impl Paged for crate::api::types::ChatMessage {
    fn id(&self) -> &str {
        &self.id
    }
}

/// `@name@host` of `u`, the host of an account of the server read filled in.
fn acct_of(u: &User, host: &str) -> String {
    let h = u.host.as_deref().filter(|h| !h.is_empty()).unwrap_or(host);
    format!("@{}@{h}", u.username)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::stub_tests::{Calls, reply, serve};
    use crate::config::{AccountStore, Session};
    use std::sync::Arc;

    const FAKE: &str = "fake-token-for-tests";

    fn ctx(url: &str, dir: &std::path::Path, json: bool) -> Ctx {
        AccountStore::open(dir)
            .save(&Session {
                service: url.into(),
                user_id: "u-me".into(),
                username: "me".into(),
                token: FAKE.into(),
                permissions: crate::miauth::PERMISSIONS
                    .iter()
                    .map(|p| p.to_string())
                    .collect(),
            })
            .unwrap();
        Ctx {
            origin: url.into(),
            json,
            host_given: true,
            accounts: Ok(AccountStore::open(dir)),
            env_token: None,
        }
    }

    fn paths(calls: &Calls) -> Vec<String> {
        calls
            .lock()
            .unwrap()
            .iter()
            .map(|(p, _)| p.clone())
            .collect()
    }

    fn body(calls: &Calls, path: &str) -> Value {
        let log = calls.lock().unwrap();
        let (_, b) = log.iter().rev().find(|(p, _)| p == path).unwrap();
        serde_json::from_str(b).unwrap()
    }

    fn run(f: impl FnOnce(&mut Vec<u8>) -> Result<()>) -> (Result<()>, String) {
        let mut out = Vec::new();
        let r = f(&mut out);
        (r, String::from_utf8(out).unwrap())
    }

    // like reacts ❤ unless --reaction says, replacing a reaction given
    // before, and says so when it is the one given already; unlike takes
    // it back.
    #[test]
    fn like_reacts_and_replaces_a_reaction_given_before() {
        let (url, calls) = serve(Arc::new(|path, _, _| match path {
            "/api/notes/show" => reply(200, r#"{"id":"n1","myReaction":"👍"}"#),
            _ => reply(204, ""),
        }));
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(&url, dir.path(), false);
        let (r, out) = run(|o| like(&c, o, "n1", None));
        r.unwrap();
        assert_eq!(out, format!("reacted ❤ to {url}/notes/n1\n"));
        assert_eq!(
            paths(&calls),
            [
                "/api/notes/show",
                "/api/notes/reactions/delete",
                "/api/notes/reactions/create"
            ]
        );
        assert_eq!(
            body(&calls, "/api/notes/reactions/create"),
            json!({"i": FAKE, "noteId": "n1", "reaction": "❤"})
        );
        let c = ctx(&url, dir.path(), true);
        let (r, out) = run(|o| like(&c, o, "n1", Some("👍")));
        r.unwrap();
        assert_eq!(
            out,
            "{\"already\":true,\"id\":\"n1\",\"reaction\":\"👍\"}\n"
        );
        let (r, out) = run(|o| unlike(&c, o, "n1"));
        r.unwrap();
        assert_eq!(out, "{\"deleted\":\"n1\"}\n");
        // A note given by its address on another server is not written to.
        let e = run(|o| like(&c, o, "https://other.test/notes/x", None))
            .0
            .unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage);
    }

    #[test]
    fn repost_unrepost_and_delete_say_what_they_did() {
        let (url, calls) = serve(Arc::new(|path, _, _| match path {
            "/api/notes/create" => reply(200, r#"{"createdNote":{"id":"rn1"}}"#),
            _ => reply(204, ""),
        }));
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(&url, dir.path(), false);
        assert_eq!(
            run(|o| repost(&c, o, "n1", true)).1,
            format!("{url}/notes/rn1\n")
        );
        assert_eq!(
            body(&calls, "/api/notes/create"),
            json!({"i": FAKE, "renoteId": "n1", "visibility": "public"})
        );
        assert_eq!(
            run(|o| repost(&c, o, "n1", false)).1,
            format!("took back the renotes of {url}/notes/n1\n")
        );
        assert_eq!(
            run(|o| delete(&c, o, "n2")).1,
            format!("deleted {url}/notes/n2\n")
        );
        assert_eq!(
            body(&calls, "/api/notes/delete"),
            json!({"i": FAKE, "noteId": "n2"})
        );
    }

    // mute and block find the account, then write about it; mutes and
    // blocks list the accounts, page by page.
    #[test]
    fn mute_block_and_their_lists() {
        let (url, calls) = serve(Arc::new(|path, body, _| match path {
            "/api/users/show" => reply(200, r#"{"id":"u-bob","username":"bob","host":"x.test"}"#),
            "/api/mute/list" if !body.contains("untilId") => reply(
                200,
                r#"[{"id":"m1","mutee":{"id":"u-bob","username":"bob","host":"x.test","name":"ボブ 🐈"}}]"#,
            ),
            "/api/blocking/list" if !body.contains("untilId") => reply(
                200,
                r#"[{"id":"b1","blockee":{"id":"u-eve","username":"eve"}}]"#,
            ),
            p if p.ends_with("/list") => reply(200, "[]"),
            _ => reply(204, ""),
        }));
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(&url, dir.path(), false);
        assert_eq!(
            run(|o| guard(&c, o, "@bob@x.test", Guard::Mute, true)).1,
            "muted @bob@x.test\n"
        );
        assert_eq!(
            body(&calls, "/api/mute/create"),
            json!({"i": FAKE, "userId": "u-bob"})
        );
        assert_eq!(
            run(|o| guard(&c, o, "bob@x.test", Guard::Block, false)).1,
            "unblocked @bob@x.test\n"
        );
        assert!(paths(&calls).contains(&"/api/blocking/delete".to_string()));
        assert_eq!(
            run(|o| guarded(&c, o, Guard::Mute, 10)).1,
            format!("ボブ 🐈 @bob@x.test  {url}/@bob@x.test\n")
        );
        let host = api::authority(&url);
        assert_eq!(
            run(|o| guarded(&c, o, Guard::Block, 10)).1,
            format!("@eve@{host}  {url}/@eve\n")
        );
    }

    // reactions lists who reacted with what; renotes the renotes; thread
    // what the note answers, the note, and its replies, in that order.
    #[test]
    fn reactions_renotes_and_thread() {
        let (url, _) = serve(Arc::new(|path, body, _| match path {
            "/api/notes/reactions" if !body.contains("untilId") => reply(
                200,
                r#"[{"id":"r1","createdAt":"t","type":":blobcat@.:","user":{"id":"u1","username":"ann"}},{"id":"r2","type":"👍🏽","user":{"id":"u2","username":"bob","host":"x.test"}}]"#,
            ),
            "/api/notes/renotes" if !body.contains("untilId") => reply(
                200,
                r#"[{"id":"rn","user":{"username":"cat"},"renoteId":"n"}]"#,
            ),
            "/api/notes/show" => reply(
                200,
                r#"{"id":"n","text":"本文","replyId":"p","repliesCount":1,"user":{"username":"me"}}"#,
            ),
            "/api/notes/conversation" => reply(
                200,
                r#"[{"id":"p","text":"親","user":{"username":"ann"}},{"id":"root","text":"最初","user":{"username":"bob"}}]"#,
            ),
            "/api/notes/children" => reply(
                200,
                r#"[{"id":"c","text":"返信","user":{"username":"cat"}}]"#,
            ),
            _ => reply(200, "[]"),
        }));
        let dir = tempfile::tempdir().unwrap();
        let c = ctx(&url, dir.path(), false);
        let host = api::authority(&url);
        assert_eq!(
            run(|o| reactions(&c, o, "n", 10)).1,
            format!(":blobcat:  @ann@{host}  {url}/@ann\n👍🏽  @bob@x.test  {url}/@bob@x.test\n")
        );
        let cj = ctx(&url, dir.path(), true);
        let ids: Vec<String> = run(|o| thread(&cj, o, "n"))
            .1
            .lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(ids, ["root", "p", "n", "c"]);
        let out = run(|o| renotes(&cj, o, "n", 10)).1;
        assert_eq!(
            serde_json::from_str::<Value>(out.trim()).unwrap()["renoteId"],
            "n"
        );
    }
}

#[cfg(test)]
mod chat_tests {
    use super::*;
    use crate::api::stub_tests::{reply, serve};
    use crate::config::{AccountStore, Session};
    use std::sync::Arc;

    // chat lists the conversations; with an account, its messages, oldest
    // first; with text, sends it once to the account.
    #[test]
    fn chat_lists_reads_and_sends() {
        let (url, calls) = serve(Arc::new(|path, body, _| match path {
            "/api/chat/history" if body.contains("\"room\":false") => reply(
                200,
                r#"[{"id":"m2","createdAt":"2026-11-26T10:00:00.000Z","fromUserId":"u-bob","fromUser":{"username":"bob"},"text":"やあ 👋"}]"#,
            ),
            "/api/chat/history" => reply(200, "[]"),
            "/api/users/show" => reply(200, r#"{"id":"u-bob","username":"bob"}"#),
            "/api/chat/messages/user-timeline" if !body.contains("untilId") => reply(
                200,
                r#"[{"id":"m2","createdAt":"2026-11-26T10:00:00.000Z","fromUserId":"u-bob","fromUser":{"username":"bob"},"text":"後"},{"id":"m1","createdAt":"2026-11-26T09:00:00.000Z","fromUserId":"u-me","fromUser":{"username":"me"},"text":"先"}]"#,
            ),
            "/api/chat/messages/user-timeline" => reply(200, "[]"),
            "/api/chat/messages/create-to-user" => reply(
                200,
                r#"{"id":"m3","fromUserId":"u-me","text":"こんにちは"}"#,
            ),
            _ => reply(404, r#"{"error":{"code":"NO_SUCH_ENDPOINT"}}"#),
        }));
        let dir = tempfile::tempdir().unwrap();
        AccountStore::open(dir.path())
            .save(&Session {
                service: url.clone(),
                user_id: "u-me".into(),
                username: "me".into(),
                token: "fake-token-for-tests".into(),
                permissions: crate::miauth::PERMISSIONS
                    .iter()
                    .map(|p| p.to_string())
                    .collect(),
            })
            .unwrap();
        let c = Ctx {
            origin: url.clone(),
            json: false,
            host_given: true,
            accounts: Ok(AccountStore::open(dir.path())),
            env_token: None,
        };
        let run = |a: Option<&str>, m: Option<&str>| {
            let mut out = Vec::new();
            chat(
                &c,
                &mut out,
                a,
                m.map(str::to_string),
                10,
                &mut std::io::empty(),
            )
            .unwrap();
            String::from_utf8(out).unwrap()
        };
        assert_eq!(
            run(None, None),
            "@bob · 2026-11-26 10:00\nやあ 👋\n".replace(
                "2026-11-26 10:00",
                &format::time("2026-11-26T10:00:00.000Z")
            )
        );
        let msgs = run(Some("bob"), None);
        assert!(
            msgs.find("先").unwrap() < msgs.find("後").unwrap(),
            "{msgs}"
        );
        assert_eq!(
            run(Some("bob"), Some("こんにちは")),
            format!("sent to @bob@{}\n", api::authority(&url))
        );
        let log = calls.lock().unwrap();
        let (_, body) = log
            .iter()
            .find(|(p, _)| p == "/api/chat/messages/create-to-user")
            .unwrap();
        let v: Value = serde_json::from_str(body).unwrap();
        assert_eq!(v["toUserId"], "u-bob");
        assert_eq!(v["text"], "こんにちは");
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;
    use crate::api::stub_tests::{reply, serve};
    use crate::config::{AccountStore, Session};
    use std::sync::Arc;

    fn ctx_for(url: &str, dir: &std::path::Path) -> Ctx {
        AccountStore::open(dir)
            .save(&Session {
                service: url.to_string(),
                user_id: "u-me".into(),
                username: "me".into(),
                token: "fake-token-for-tests".into(),
                permissions: crate::miauth::PERMISSIONS
                    .iter()
                    .map(|p| p.to_string())
                    .collect(),
            })
            .unwrap();
        Ctx {
            origin: url.to_string(),
            json: false,
            host_given: true,
            accounts: Ok(AccountStore::open(dir)),
            env_token: None,
        }
    }

    // A report is sent only once confirmed: --yes, or y on a terminal; a
    // note reports its author with the note's address; nothing is sent
    // otherwise.
    #[test]
    fn a_report_is_sent_only_once_confirmed() {
        let (url, calls) = serve(Arc::new(|path, _, _| match path {
            "/api/notes/show" => reply(
                200,
                r#"{"id":"n1","userId":"u-bob","user":{"id":"u-bob","username":"bob"},"text":"x"}"#,
            ),
            "/api/users/show" => reply(200, r#"{"id":"u-bob","username":"bob"}"#),
            "/api/users/report-abuse" => reply(204, ""),
            _ => reply(404, r#"{"error":{"code":"NO_SUCH_ENDPOINT"}}"#),
        }));
        let dir = tempfile::tempdir().unwrap();
        let c = ctx_for(&url, dir.path());
        let mut out = Vec::new();
        // No terminal, no --yes: refused before anything is sent.
        let e = report(
            &c,
            &mut out,
            "@bob",
            ReportReason::Spam,
            None,
            false,
            &mut std::io::empty(),
            None,
        )
        .unwrap_err();
        assert!(e.message().contains("only once confirmed"), "{e:?}");
        // Asked, and n: not sent.
        let mut asked = Vec::new();
        let e = report(
            &c,
            &mut out,
            "@bob",
            ReportReason::Spam,
            None,
            false,
            &mut "n\n".as_bytes(),
            Some(&mut asked),
        )
        .unwrap_err();
        assert!(e.message().contains("not reported"));
        assert!(String::from_utf8(asked).unwrap().contains("report @bob@"));
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|(p, _)| p == "/api/users/report-abuse")
        );
        // y: sent, the note's address and the comment in it.
        let mut asked = Vec::new();
        report(
            &c,
            &mut out,
            "n1",
            ReportReason::Rude,
            Some("ひどい 😡"),
            false,
            &mut "y\n".as_bytes(),
            Some(&mut asked),
        )
        .unwrap();
        report(
            &c,
            &mut out,
            "@bob",
            ReportReason::Other,
            None,
            true,
            &mut std::io::empty(),
            None,
        )
        .unwrap();
        let log = calls.lock().unwrap();
        let sent: Vec<Value> = log
            .iter()
            .filter(|(p, _)| p == "/api/users/report-abuse")
            .map(|(_, b)| serde_json::from_str(b).unwrap())
            .collect();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["userId"], "u-bob");
        assert_eq!(
            sent[0]["comment"],
            format!(
                "Rude or harassing\nNote: {}\n-----\nひどい 😡",
                format::note_url(&url, "n1")
            )
        );
        assert_eq!(sent[1]["comment"], "Other");
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("reported @bob@"), "{printed}");
    }

    // Over Misskey's 2048 characters, the report is refused before it is
    // sent; at the limit it goes.
    #[test]
    fn a_report_over_the_limit_is_refused() {
        let base = ReportReason::Other.label().chars().count() + "\n-----\n".chars().count();
        let fits = "🇯".repeat(MAX_REPORT_CHARS - base);
        assert!(report_comment(ReportReason::Other, None, Some(&fits)).is_ok());
        let over = format!("{fits}x");
        assert!(
            report_comment(ReportReason::Other, None, Some(&over))
                .unwrap_err()
                .message()
                .contains("2048")
        );
    }
}
