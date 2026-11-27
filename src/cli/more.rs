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
