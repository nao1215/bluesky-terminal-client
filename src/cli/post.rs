//! The commands that write: `post`, `follow` and `unfollow`, with the
//! options of bsky's. Everything that can be told before anything is sent
//! (the length, the number of files, a file that cannot be read, a login
//! that may not write) is a usage error, and nothing is sent. Every write
//! is sent once and never tried again.

use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

use super::{Ctx, format, json_line, parse_note, parse_user, text};
use crate::api::types::{Note, User};
use crate::api::{self, Client};
use crate::error::{Error, Kind, Result};
use crate::mfm;
use crate::text::{drawable, one_line};

/// The most characters a content warning may have (`notes/create`'s `cw`).
pub const MAX_CW_CHARS: usize = 100;
/// The most characters a file's alt text may have (`drive/files/create`'s
/// `comment`).
pub const MAX_ALT_CHARS: usize = 512;
/// The most pictures a post has, as bsky's `post` takes them.
pub const MAX_IMAGES: usize = 4;
/// The language the preview is asked in, as Misskey's web client asks for
/// it when the account sets none.
pub const PREVIEW_LANG: &str = "ja-JP";

/// Who may see a note, by the names `notes/create` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Visibility {
    /// Everyone, on every timeline.
    Public,
    /// Everyone, but only on your followers' home timelines.
    Home,
    /// Your followers only.
    Followers,
}

impl Visibility {
    fn name(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Home => "home",
            Visibility::Followers => "followers",
        }
    }
}

/// What `msky post` was given.
#[derive(Debug, Clone)]
pub struct PostArgs {
    pub text: String,
    pub images: Vec<PathBuf>,
    pub alts: Vec<String>,
    pub video: Option<PathBuf>,
    pub reply: Option<String>,
    pub quote: Option<String>,
    pub no_card: bool,
    pub cw: Option<String>,
    pub visibility: Visibility,
    pub dry_run: bool,
}

/// A file read from disk, to be uploaded.
struct Local {
    path: PathBuf,
    name: String,
    data: Vec<u8>,
    alt: String,
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// The note to post, as the checks that need no server leave it.
struct Draft {
    text: String,
    /// Whether --no-card made links of the text silent, which made it longer.
    silenced: bool,
    cw: Option<String>,
    files: Vec<Local>,
    reply: Option<String>,
    quote: Option<String>,
}

/// Check what `post` was given and read its files, before anything is
/// asked of the server.
fn draft(a: &PostArgs, stdin: &mut dyn Read, origin: &str) -> Result<Draft> {
    let body = if a.text == "-" {
        let mut s = String::new();
        stdin
            .read_to_string(&mut s)
            .map_err(|e| Error::io(format!("cannot read the note from stdin: {e}")))?;
        s
    } else {
        a.text.clone()
    };
    if a.video.is_some() && !a.images.is_empty() {
        return Err(Error::usage(
            "a note can have up to 4 pictures or one video, not both",
        ));
    }
    if a.images.len() > MAX_IMAGES {
        return Err(Error::usage(format!(
            "a note can have at most {MAX_IMAGES} pictures"
        )));
    }
    let paths: Vec<PathBuf> = a.images.iter().cloned().chain(a.video.clone()).collect();
    if a.alts.len() > paths.len() {
        return Err(Error::usage("there is more --alt than pictures and videos"));
    }
    if let Some(alt) = a.alts.iter().find(|t| chars(t) > MAX_ALT_CHARS) {
        return Err(Error::usage(format!(
            "an --alt is {} characters; the limit is {MAX_ALT_CHARS}",
            chars(alt)
        )));
    }
    let cw = match a.cw.as_deref() {
        Some(c) if c.trim().is_empty() => {
            return Err(Error::usage("--cw is empty")
                .with_hint("give the warning shown in place of the text"));
        }
        Some(c) if chars(c) > MAX_CW_CHARS => {
            return Err(Error::usage(format!(
                "the content warning is {} characters; the limit is {MAX_CW_CHARS}",
                chars(c)
            )));
        }
        c => c.map(str::to_string),
    };
    // The server trims the text; it is counted as it will be kept.
    let trimmed = body.trim();
    let text = if a.no_card {
        mfm::silenced(trimmed)
    } else {
        trimmed.to_string()
    };
    if text.is_empty() && paths.is_empty() && a.quote.is_none() {
        return Err(Error::usage("the note is empty")
            .with_hint("give its text, or --image, --video or --quote"));
    }
    let same_server = |what: &str, typed: &str| -> Result<String> {
        let (at, id) = parse_note(typed)?;
        match at {
            Some(o) if o != origin => Err(Error::usage(format!(
                "the note given to --{what} is on {}, and msky posts on {}",
                api::authority(&o),
                api::authority(origin)
            ))
            .with_hint(format!(
                "open the note on {} and give its address there",
                api::authority(origin)
            ))),
            _ => Ok(id),
        }
    };
    let reply = a
        .reply
        .as_deref()
        .map(|r| same_server("reply", r))
        .transpose()?;
    let quote = a
        .quote
        .as_deref()
        .map(|q| same_server("quote", q))
        .transpose()?;
    let mut files = Vec::new();
    for (i, path) in paths.into_iter().enumerate() {
        let data = std::fs::read(&path)
            .map_err(|e| Error::io(format!("cannot read {}: {e}", path.display())))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        files.push(Local {
            path,
            name,
            data,
            alt: a.alts.get(i).cloned().unwrap_or_default(),
        });
    }
    Ok(Draft {
        silenced: text != trimmed,
        text,
        cw,
        files,
        reply,
        quote,
    })
}

/// The largest file the account may upload, from `i`'s `policies`.
fn max_file_bytes(me: Option<&User>) -> Option<u64> {
    let mb = me?.policies.as_ref()?.get("maxFileSizeMb")?.as_f64()?;
    (mb > 0.0).then_some((mb * 1024.0 * 1024.0) as u64)
}

/// `i`, for the account's name and what it may upload. A token that may
/// not read the account (an `MSKY_TOKEN` made without `read:account`) is
/// no reason to stop: the post goes on without them.
fn me(ctx: &Ctx, client: &Client) -> Result<Option<User>> {
    match client.me() {
        Ok(u) => Ok(Some(u)),
        Err(e) if e.message().contains("PERMISSION_DENIED") => Ok(None),
        Err(e) => Err(ctx.explained(e)),
    }
}

/// Why the text of `d` is longer than `max` characters, if it is.
fn too_long(d: &Draft, max: usize) -> Result<()> {
    let n = chars(&d.text);
    if n <= max {
        return Ok(());
    }
    let why = if d.silenced {
        format!(
            "the note is {n} characters with its links made silent for --no-card; the limit is {max}"
        )
    } else {
        format!("the note is {n} characters; the limit is {max}")
    };
    Err(Error::usage(why))
}

/// `msky post`.
pub fn post(ctx: &Ctx, out: &mut dyn Write, a: &PostArgs, stdin: &mut dyn Read) -> Result<()> {
    let d = draft(a, stdin, &ctx.origin)?;
    let mut needs = vec!["write:notes"];
    if !d.files.is_empty() {
        needs.push("write:drive");
    }
    let client = ctx.writer(&needs, "post notes")?;
    // The limit is the server's own: it says it in `meta`, counted in code
    // points as its API counts them.
    too_long(&d, client.max_note_chars())?;
    let me = me(ctx, &client)?;
    if let Some(max) = max_file_bytes(me.as_ref())
        && let Some(f) = d.files.iter().find(|f| f.data.len() as u64 > max)
    {
        return Err(Error::usage(format!(
            "{} is {} bytes; {} takes files up to {max} bytes",
            f.path.display(),
            f.data.len(),
            client.host()
        )));
    }
    let reply = d
        .reply
        .as_deref()
        .map(|id| client.note(id))
        .transpose()
        .map_err(|e| ctx.explained(e))?;
    let quote = d
        .quote
        .as_deref()
        .map(|id| client.note(id))
        .transpose()
        .map_err(|e| ctx.explained(e))?;
    if a.dry_run {
        return dry_run(ctx, out, &client, a, &d, me.as_ref(), quote.as_ref());
    }
    let mut ids = Vec::new();
    for f in &d.files {
        let alt = (!f.alt.is_empty()).then_some(f.alt.as_str());
        let up = client
            .upload(&f.name, &f.data, alt)
            .map_err(|e| ctx.explained(e))?;
        ids.push(up.id);
    }
    let mut body = json!({"visibility": a.visibility.name()});
    if !d.text.is_empty() {
        body["text"] = json!(d.text);
    }
    if let Some(cw) = &d.cw {
        body["cw"] = json!(cw);
    }
    if let Some(r) = &reply {
        body["replyId"] = json!(r.id);
    }
    if let Some(q) = &quote {
        body["renoteId"] = json!(q.id);
    }
    if !ids.is_empty() {
        body["fileIds"] = json!(ids);
    }
    let made = client.create_note(&body).map_err(|e| {
        let e = ctx.explained(e);
        if ids.is_empty() {
            e
        } else {
            let hint = e.hint().map(|h| format!("{h}; ")).unwrap_or_default();
            e.with_hint(format!(
                "{hint}the files uploaded for it are left in your drive"
            ))
        }
    })?;
    let url = format::note_url(client.origin(), &made.id);
    if ctx.json {
        return json_line(out, &json!({"uri": url, "id": made.id}));
    }
    text(out, &format!("{url}\n"))
}

/// What `/url` answered, put as `post --dry-run` shows it.
fn preview_json(v: &Value) -> Option<Value> {
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    s("url")?;
    Some(json!({
        "url": s("url"),
        "title": s("title"),
        "description": s("description"),
        "thumbnail": s("thumbnail"),
        "icon": s("icon"),
        "sitename": s("sitename"),
        "sensitive": v.get("sensitive").and_then(Value::as_bool).unwrap_or(false),
    }))
}

/// `post --dry-run`: what would be sent, and the preview the server makes
/// of the first address Misskey's web client would show one under. Nothing
/// is uploaded or posted. A preview the server cannot make is exit 4, after
/// the rest is printed.
fn dry_run(
    ctx: &Ctx,
    out: &mut dyn Write,
    client: &Client,
    a: &PostArgs,
    d: &Draft,
    me: Option<&User>,
    quote: Option<&Note>,
) -> Result<()> {
    let skip: Vec<&str> = quote
        .into_iter()
        .flat_map(|q| [q.url.as_deref(), q.uri.as_deref()])
        .flatten()
        .collect();
    let previewed = mfm::previewed(&d.text, &skip);
    let mut failed: Option<Error> = None;
    let preview = match previewed.first() {
        None => None,
        Some(first) => {
            // The client asks without the part after `#`.
            let asked = mfm::without_hash(first);
            match client.url_preview(asked, PREVIEW_LANG)? {
                Ok(v) => match preview_json(&v) {
                    Some(p) => Some(p),
                    None => {
                        failed = Some(Error::api(format!(
                            "{} made no preview of {asked}",
                            client.host()
                        )));
                        None
                    }
                },
                Err(r) => {
                    let said = one_line(&r.said());
                    let e = Error::api(format!(
                        "{} could not make a preview of {asked}: {said}",
                        client.host()
                    ));
                    failed = Some(if said.starts_with("URL_PREVIEW_DISABLED") {
                        e.with_hint(format!(
                            "{} shows no previews of links; the note would be posted without one",
                            client.host()
                        ))
                    } else {
                        e.with_hint(format!(
                            "check that the page is up and that {} can read it; it keeps an answer for up to a day",
                            client.host()
                        ))
                    });
                    None
                }
            }
        }
    };
    let files: Vec<Value> = d
        .files
        .iter()
        .map(|f| {
            json!({
                "path": f.path.to_string_lossy(),
                "name": f.name,
                "size": f.data.len(),
                "alt": (!f.alt.is_empty()).then_some(&f.alt),
            })
        })
        .collect();
    if ctx.json {
        let mut v = json!({
            "dryRun": true,
            "host": client.host(),
            "username": me.map(|u| &u.username),
            "note": {
                "text": (!d.text.is_empty()).then_some(&d.text),
                "cw": d.cw,
                "visibility": a.visibility.name(),
                "replyId": d.reply,
                "renoteId": d.quote,
                "files": files,
            },
            "previewed": previewed,
            "preview": preview,
        });
        if let Some(e) = &failed {
            v["previewError"] = json!(e.message());
        }
        json_line(out, &v)?;
    } else {
        let mut s = format!("dry run: nothing was sent to {}\n", client.host());
        let who = me.map_or_else(
            || "the account of MSKY_TOKEN".to_string(),
            |u| format!("@{}@{}", one_line(&u.username), client.host()),
        );
        s.push_str(&format!("as {who}, {}\n", a.visibility.name()));
        if let Some(cw) = &d.cw {
            s.push_str(&format!("cw: {}\n", one_line(cw)));
        }
        if let Some(id) = &d.reply {
            s.push_str(&format!(
                "reply to: {}\n",
                format::note_url(client.origin(), id)
            ));
        }
        if let Some(id) = &d.quote {
            s.push_str(&format!(
                "quote: {}\n",
                format::note_url(client.origin(), id)
            ));
        }
        for f in &d.files {
            s.push_str(&format!(
                "file: {} ({} bytes)",
                f.path.display(),
                f.data.len()
            ));
            if !f.alt.is_empty() {
                s.push_str(&format!(", alt: {}", one_line(&f.alt)));
            }
            s.push('\n');
        }
        s.push_str("text:\n");
        s.push_str(&drawable(&d.text));
        s.push('\n');
        match (&preview, previewed.first()) {
            (Some(p), Some(first)) => {
                if previewed.len() > 1 {
                    s.push_str(&format!(
                        "{} links get a preview; the first:\n",
                        previewed.len()
                    ));
                }
                s.push_str(&format!("preview of {first}:\n"));
                for (label, key) in [
                    ("title", "title"),
                    ("description", "description"),
                    ("image", "thumbnail"),
                    ("site", "sitename"),
                ] {
                    let v = p[key].as_str().unwrap_or("(none)");
                    s.push_str(&format!("  {label}: {}\n", one_line(v)));
                }
                if p["sensitive"] == json!(true) {
                    s.push_str("  sensitive: the server hides it until it is clicked\n");
                }
            }
            (None, Some(first)) => s.push_str(&format!("no preview of {first}\n")),
            (_, None) if a.no_card => {
                s.push_str("no preview: --no-card made the links silent\n");
            }
            (_, None) => s.push_str("no preview: the text has no link Misskey previews\n"),
        }
        text(out, &s)?;
    }
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `@name@host` of `u`, read on the server at `host`.
fn handle(u: &User, host: &str) -> String {
    let at = u.host.as_deref().filter(|h| !h.is_empty()).unwrap_or(host);
    format!("@{}@{}", one_line(&u.username), one_line(at))
}

/// Whether `u` is an account of another server than `host`.
fn remote(u: &User, host: &str) -> bool {
    u.host
        .as_deref()
        .is_some_and(|h| !h.is_empty() && !h.eq_ignore_ascii_case(host))
}

/// `msky follow` (`on`) and `msky unfollow`.
pub fn follow(ctx: &Ctx, out: &mut dyn Write, actor: &str, on: bool) -> Result<()> {
    let who = parse_user(actor)?;
    if let Some(o) = who.origin.as_deref().filter(|o| *o != ctx.origin) {
        return Err(Error::usage(format!(
            "the account is given by its address on {}, and msky follows from {}",
            api::authority(o),
            api::authority(&ctx.origin)
        ))
        .with_hint("give it as @name@host"));
    }
    let what = if on { "follow" } else { "unfollow" };
    let client = ctx.writer(&["write:following"], "follow accounts")?;
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let u = client
        .user(&who.username, host)
        .map_err(|e| ctx.explained(e))?;
    let shown = handle(&u, client.host());
    let my_id = match ctx.session(&ctx.origin)?.map(|s| s.user_id) {
        Some(id) if !id.is_empty() => Some(id),
        _ => me(ctx, &client)?.map(|m| m.id),
    };
    if my_id.as_deref() == Some(u.id.as_str()) {
        return Err(Error::new(
            Kind::Usage,
            format!("you cannot {what} yourself"),
        ));
    }
    let mut said = |v: Value, s: String| -> Result<()> {
        if ctx.json {
            json_line(out, &v)
        } else {
            text(out, &format!("{s}\n"))
        }
    };
    if on {
        if u.is_following {
            return said(
                json!({"id": u.id, "already": true}),
                format!("already following {shown}"),
            );
        }
        if u.has_pending_follow_request_from_you {
            return said(
                json!({"id": u.id, "already": true, "requested": true}),
                format!("already asked to follow {shown}"),
            );
        }
        client.follow(&u.id).map_err(|e| ctx.explained(e))?;
        if u.is_locked {
            return said(
                json!({"id": u.id, "requested": true}),
                format!("asked to follow {shown}; they approve their followers"),
            );
        }
        // An account of another server is followed once its server accepts
        // the request, which it does in its own time: until then the
        // follow is pending, and `msky follows --pending` lists it.
        if remote(&u, client.host()) {
            let now = client
                .user(&who.username, host)
                .map_err(|e| ctx.explained(e))?;
            if !now.is_following {
                return said(
                    json!({"id": u.id, "requested": true}),
                    format!(
                        "asked to follow {shown}; pending until their server accepts (msky follows --pending lists it)"
                    ),
                );
            }
        }
        return said(json!({"id": u.id}), format!("following {shown}"));
    }
    if u.is_following {
        client.unfollow(&u.id).map_err(|e| ctx.explained(e))?;
        return said(json!({"deleted": u.id}), format!("unfollowed {shown}"));
    }
    if u.has_pending_follow_request_from_you {
        client
            .cancel_follow_request(&u.id)
            .map_err(|e| ctx.explained(e))?;
        return said(
            json!({"deleted": u.id, "requested": true}),
            format!("took back the request to follow {shown}"),
        );
    }
    Err(Error::usage(format!("you do not follow {shown}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::stub_tests::{Calls, reply, serve};
    use crate::config::{AccountStore, Session};
    use rstest::rstest;
    use std::sync::Arc;

    /// A throwaway value in the place of a token; no server gave it.
    const FAKE: &str = "fake-token-for-tests";

    fn args(text: &str) -> PostArgs {
        PostArgs {
            text: text.into(),
            images: Vec::new(),
            alts: Vec::new(),
            video: None,
            reply: None,
            quote: None,
            no_card: false,
            cw: None,
            visibility: Visibility::Public,
            dry_run: false,
        }
    }

    /// A login saved for `origin` in `dir`, with `permissions`.
    fn login(dir: &std::path::Path, origin: &str, permissions: &[&str]) {
        AccountStore::open(dir)
            .save(&Session {
                service: origin.into(),
                user_id: "u-me".into(),
                username: "me".into(),
                token: FAKE.into(),
                permissions: permissions.iter().map(|p| p.to_string()).collect(),
            })
            .unwrap();
    }

    fn ctx(origin: &str, dir: &std::path::Path, json: bool) -> Ctx {
        Ctx {
            origin: origin.into(),
            json,
            host_given: true,
            accounts: Ok(AccountStore::open(dir)),
            env_token: None,
        }
    }

    fn run(c: &Ctx, a: &PostArgs) -> (Result<()>, String) {
        let mut out = Vec::new();
        let r = post(c, &mut out, a, &mut std::io::empty());
        (r, String::from_utf8(out).unwrap())
    }

    fn body(calls: &Calls, path: &str) -> Value {
        let log = calls.lock().unwrap();
        let (_, b) = log.iter().find(|(p, _)| p == path).unwrap();
        serde_json::from_str(b).unwrap()
    }

    fn paths(calls: &Calls) -> Vec<String> {
        calls
            .lock()
            .unwrap()
            .iter()
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// A server that takes a note: `i` with the policies given, notes to
    /// reply to and quote, uploads and `notes/create`, and `/url`.
    fn server(preview: (u16, &'static str)) -> (String, Calls) {
        serve(Arc::new(move |path, _, n| match path {
            "/api/i" => reply(
                200,
                r#"{"id":"u-me","username":"me","policies":{"maxFileSizeMb":1}}"#,
            ),
            "/api/notes/show" => reply(200, r#"{"id":"n0","text":"元","url":null,"uri":null}"#),
            "/api/drive/files/create" => reply(200, &format!(r#"{{"id":"f{n}","name":"x"}}"#)),
            "/api/notes/create" => reply(200, r#"{"createdNote":{"id":"9new"}}"#),
            p if p.starts_with("/url?") => reply(preview.0, preview.1),
            _ => reply(404, "{}"),
        }))
    }

    const PREVIEW: &str = r#"{"url":"https://debimate.jp/post/a/","title":"記事の題 🐈‍⬛","description":"説明\u001b[2J","thumbnail":"https://media.test/preview.webp?url=x","icon":null,"sitename":"debimate","sensitive":false,"player":{"url":null}}"#;

    // What can be told before anything is sent is exit 1, and nothing is
    // asked of the server: not even who the account is.
    #[rstest]
    #[case::too_many_pictures(PostArgs { images: vec!["a".into(); 5], ..args("x") }, "at most 4 pictures")]
    #[case::pictures_and_a_video(PostArgs { images: vec!["a".into()], video: Some("v".into()), ..args("x") }, "not both")]
    #[case::more_alt_than_files(PostArgs { alts: vec!["a".into()], ..args("x") }, "more --alt")]
    #[case::long_alt(PostArgs { images: vec!["a".into()], alts: vec!["あ".repeat(513)], ..args("x") }, "513 characters; the limit is 512")]
    #[case::empty_cw(PostArgs { cw: Some(" ".into()), ..args("x") }, "--cw is empty")]
    #[case::long_cw(PostArgs { cw: Some("🐈".repeat(101)), ..args("x") }, "101 characters; the limit is 100")]
    #[case::empty(args("  \n "), "the note is empty")]
    #[case::reply_elsewhere(PostArgs { reply: Some("https://other.test/notes/a1".into()), ..args("x") }, "is on other.test")]
    #[case::not_a_note(PostArgs { quote: Some("../x".into()), ..args("x") }, "is not a note")]
    fn what_is_wrong_before_sending_is_a_usage_error(#[case] a: PostArgs, #[case] said: &str) {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let (r, out) = run(&ctx(&url, dir.path(), false), &a);
        let e = r.unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{e:?}");
        assert!(e.message().contains(said), "{e:?}");
        assert!(out.is_empty());
        assert!(paths(&calls).is_empty(), "{:?}", paths(&calls));
    }

    // The limit is the one the server's `meta` names, counted in code
    // points as its API counts them; a server that does not say has
    // Misskey's 3000. Over it is exit 1, and nothing is sent.
    #[rstest]
    #[case::server_says_5000(r#"{"maxNoteTextLength":5000}"#, 5000, true)]
    #[case::over_5000(r#"{"maxNoteTextLength":5000}"#, 5001, false)]
    #[case::server_says_500(r#"{"maxNoteTextLength":500}"#, 501, false)]
    #[case::meta_says_nothing(r#"{}"#, 3000, true)]
    #[case::meta_fails_3001("", 3001, false)]
    fn the_length_limit_is_the_servers_own(
        #[case] meta: &'static str,
        #[case] n: usize,
        #[case] taken: bool,
    ) {
        let (url, calls) = serve(Arc::new(move |path, _, _| match path {
            "/api/meta" if !meta.is_empty() => reply(200, meta),
            "/api/i" => reply(200, r#"{"id":"u-me","username":"me"}"#),
            "/api/notes/create" => reply(200, r#"{"createdNote":{"id":"9new"}}"#),
            _ => reply(500, "{}"),
        }));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let (r, _) = run(&ctx(&url, dir.path(), false), &args(&"あ".repeat(n)));
        if taken {
            r.unwrap();
            assert!(paths(&calls).contains(&"/api/notes/create".to_string()));
        } else {
            let e = r.unwrap_err();
            assert_eq!(e.kind(), Kind::Usage);
            assert!(
                e.message()
                    .contains(&format!("{n} characters; the limit is")),
                "{e:?}"
            );
            assert_eq!(paths(&calls), ["/api/meta"]);
        }
    }

    // The text is counted as the server keeps it, trimmed.
    #[test]
    fn the_limit_itself_is_taken() {
        let text = format!(" {}\n", "👍🏽".repeat(1500));
        let d = draft(&args(&text), &mut std::io::empty(), "http://x.test").unwrap();
        assert_eq!(chars(&d.text), 3000);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_exit_3_and_nothing_is_sent() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let a = PostArgs {
            images: vec![dir.path().join("gone.png")],
            ..args("写真")
        };
        let e = run(&ctx(&url, dir.path(), false), &a).0.unwrap_err();
        assert_eq!(e.kind().exit_code(), 3);
        assert!(e.message().contains("gone.png"), "{e:?}");
        assert!(paths(&calls).is_empty());
    }

    // A login saved when msky could only read says to log in again,
    // before anything is sent.
    #[test]
    fn a_read_only_login_is_told_to_log_in_again() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, &[]);
        let e = run(&ctx(&url, dir.path(), false), &args("hello"))
            .0
            .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert!(
            e.message()
                .contains("does not allow msky to post notes: it lacks write:notes"),
            "{e:?}"
        );
        assert!(
            e.hint().unwrap().contains("run `msky login --host"),
            "{e:?}"
        );
        assert!(paths(&calls).is_empty());
    }

    #[test]
    fn without_a_login_post_is_a_usage_error() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        let e = run(&ctx(&url, dir.path(), false), &args("hello"))
            .0
            .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert!(e.message().starts_with("not logged in on"), "{e:?}");
        assert!(paths(&calls).is_empty());
    }

    // The pictures are uploaded with their alt text, then the note is
    // made with them, its warning, its visibility, the note it replies to
    // and the one it quotes; --json prints its address and ID.
    #[test]
    fn a_note_is_posted_with_its_files() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let pic = dir.path().join("猫 \"1\".png");
        std::fs::write(&pic, b"\x89PNG\r\n\x1a\nbinary\x00\xff").unwrap();
        let a = PostArgs {
            images: vec![pic.clone(), pic],
            alts: vec!["黒猫 🐈‍⬛".into()],
            reply: Some("n0".into()),
            quote: Some(format!("{url}/notes/n0")),
            cw: Some("ネタバレ".into()),
            visibility: Visibility::Followers,
            ..args("新しい記事 https://debimate.jp/post/a/\n")
        };
        let (r, out) = run(&ctx(&url, dir.path(), true), &a);
        r.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v, json!({"uri": format!("{url}/notes/9new"), "id": "9new"}));
        assert_eq!(
            paths(&calls),
            [
                "/api/meta",
                "/api/i",
                "/api/notes/show",
                "/api/notes/show",
                "/api/drive/files/create",
                "/api/drive/files/create",
                "/api/notes/create"
            ]
        );
        let log = calls.lock().unwrap();
        let upload = &log[4].1;
        assert!(
            upload.contains(&format!("name=\"i\"\r\n\r\n{FAKE}\r\n")),
            "{upload}"
        );
        assert!(
            upload.contains("name=\"name\"\r\n\r\n猫 \"1\".png\r\n"),
            "{upload}"
        );
        assert!(
            upload.contains("name=\"comment\"\r\n\r\n黒猫 🐈‍⬛\r\n"),
            "{upload}"
        );
        assert!(upload.contains("filename=\"猫 %221%22.png\""), "{upload}");
        assert!(
            !log[5].1.contains("name=\"comment\""),
            "the second has no alt"
        );
        drop(log);
        assert_eq!(
            body(&calls, "/api/notes/create"),
            json!({
                "i": FAKE,
                "text": "新しい記事 https://debimate.jp/post/a/",
                "cw": "ネタバレ",
                "visibility": "followers",
                "replyId": "n0",
                "renoteId": "n0",
                "fileIds": ["f4", "f5"],
            })
        );
    }

    #[test]
    fn a_posted_note_prints_its_address() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let (r, out) = run(&ctx(&url, dir.path(), false), &args("hello 👋"));
        r.unwrap();
        assert_eq!(out, format!("{url}/notes/9new\n"));
        assert_eq!(
            body(&calls, "/api/notes/create"),
            json!({"i": FAKE, "text": "hello 👋", "visibility": "public"})
        );
    }

    // A file larger than the account may upload is refused before any
    // upload.
    #[test]
    fn a_file_over_the_accounts_limit_is_refused_before_upload() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let clip = dir.path().join("clip.mp4");
        std::fs::write(&clip, vec![0u8; 1024 * 1024 + 1]).unwrap();
        let a = PostArgs {
            video: Some(clip),
            ..args("")
        };
        let e = run(&ctx(&url, dir.path(), false), &a).0.unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert!(
            e.message().contains("takes files up to 1048576 bytes"),
            "{e:?}"
        );
        assert_eq!(paths(&calls), ["/api/meta", "/api/i"]);
    }

    // --dry-run reads the files and asks the server for the preview of
    // the first link, as the web client asks (without the part after #,
    // in ja-JP), and uploads and posts nothing.
    #[test]
    fn a_dry_run_shows_the_note_and_the_preview_and_sends_nothing() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let pic = dir.path().join("a.png");
        std::fs::write(&pic, b"png").unwrap();
        let a = PostArgs {
            images: vec![pic],
            alts: vec!["絵".into()],
            dry_run: true,
            ..args(
                "記事 https://debimate.jp/post/a/#top と ?[静か](https://s.test) <https://b.test>",
            )
        };
        let (r, out) = run(&ctx(&url, dir.path(), true), &a);
        r.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["dryRun"], true);
        assert_eq!(v["username"], "me");
        assert_eq!(v["note"]["visibility"], "public");
        assert_eq!(v["note"]["files"][0]["size"], 3);
        assert_eq!(v["note"]["files"][0]["alt"], "絵");
        assert_eq!(
            v["previewed"],
            json!(["https://debimate.jp/post/a/#top", "https://b.test"])
        );
        assert_eq!(v["preview"]["title"], "記事の題 🐈‍⬛");
        assert_eq!(
            v["preview"]["thumbnail"],
            "https://media.test/preview.webp?url=x"
        );
        assert!(v.get("previewError").is_none());
        assert_eq!(
            paths(&calls),
            [
                "/api/meta",
                "/api/i",
                "/url?url=https%3A%2F%2Fdebimate.jp%2Fpost%2Fa%2F&lang=ja-JP"
            ]
        );
    }

    // In text, what others wrote (the page's title and description) is
    // printed without its control characters.
    #[test]
    fn a_dry_run_in_text_prints_the_preview_as_text() {
        let (url, _) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let a = PostArgs {
            dry_run: true,
            cw: Some("注意".into()),
            ..args("記事 https://debimate.jp/post/a/")
        };
        let (r, out) = run(&ctx(&url, dir.path(), false), &a);
        r.unwrap();
        let host = api::authority(&url);
        assert_eq!(
            out,
            format!(
                "dry run: nothing was sent to {host}\n\
                 as @me@{host}, public\n\
                 cw: 注意\n\
                 text:\n\
                 記事 https://debimate.jp/post/a/\n\
                 preview of https://debimate.jp/post/a/:\n\
                 \x20 title: 記事の題 🐈‍⬛\n\
                 \x20 description: 説明[2J\n\
                 \x20 image: https://media.test/preview.webp?url=x\n\
                 \x20 site: debimate\n"
            )
        );
    }

    // A preview the server cannot make is exit 4, after what would be
    // sent is printed with the reason.
    #[rstest]
    #[case::failed(
        422,
        r#"{"error":{"code":"URL_PREVIEW_FAILED","message":"Failed to get preview"}}"#,
        "is up"
    )]
    #[case::disabled(
        403,
        r#"{"error":{"code":"URL_PREVIEW_DISABLED","message":"URL preview is disabled"}}"#,
        "shows no previews"
    )]
    fn a_preview_the_server_cannot_make_is_exit_4(
        #[case] status: u16,
        #[case] answer: &'static str,
        #[case] hint: &str,
    ) {
        let (url, calls) = server((status, answer));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let a = PostArgs {
            dry_run: true,
            ..args("https://down.test/")
        };
        let (r, out) = run(&ctx(&url, dir.path(), true), &a);
        let e = r.unwrap_err();
        assert_eq!(e.kind().exit_code(), 4);
        assert!(
            e.message()
                .contains("could not make a preview of https://down.test/"),
            "{e:?}"
        );
        assert!(e.hint().unwrap().contains(hint), "{e:?}");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["preview"], Value::Null);
        assert!(v["previewError"].as_str().unwrap().contains("URL_PREVIEW_"));
        assert!(!paths(&calls).contains(&"/api/notes/create".to_string()));
    }

    // --no-card writes the links as silent links: the note shows no
    // preview, and --dry-run asks for none.
    #[test]
    fn no_card_posts_the_links_silent() {
        let (url, calls) = server((200, PREVIEW));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let mut a = PostArgs {
            no_card: true,
            dry_run: true,
            ..args("記事 https://debimate.jp/post/a/")
        };
        let (r, out) = run(&ctx(&url, dir.path(), false), &a);
        r.unwrap();
        assert!(
            out.ends_with("no preview: --no-card made the links silent\n"),
            "{out}"
        );
        a.dry_run = false;
        run(&ctx(&url, dir.path(), false), &a).0.unwrap();
        assert!(!paths(&calls).iter().any(|p| p.starts_with("/url")));
        assert_eq!(
            body(&calls, "/api/notes/create")["text"],
            "記事 ?[<plain>https://debimate.jp/post/a/</plain>](https://debimate.jp/post/a/)"
        );
    }

    // MSKY_TOKEN is used in place of the saved login, with no check of
    // its permissions here; the server's refusal says what to do about it.
    #[test]
    fn the_token_from_the_environment_is_used_and_its_refusal_explained() {
        let (url, calls) = serve(Arc::new(|path, _, _| match path {
            "/api/i" => reply(
                403,
                r#"{"error":{"code":"PERMISSION_DENIED","message":"no"}}"#,
            ),
            _ => reply(
                403,
                r#"{"error":{"code":"PERMISSION_DENIED","message":"Your app does not have the necessary permissions to use this endpoint."}}"#,
            ),
        }));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, &[]);
        let c = Ctx {
            env_token: Some("env-fake-token".into()),
            ..ctx(&url, dir.path(), false)
        };
        let e = run(&c, &args("hello")).0.unwrap_err();
        assert_eq!(e.kind().exit_code(), 4);
        assert!(e.hint().unwrap().contains("MSKY_TOKEN"), "{e:?}");
        assert!(!e.to_string().contains("env-fake-token"));
        assert_eq!(paths(&calls), ["/api/meta", "/api/i", "/api/notes/create"]);
        assert_eq!(body(&calls, "/api/notes/create")["i"], "env-fake-token");
        // Another server gets neither token.
        assert!(!c.client(Some("http://other.test")).unwrap().logged_in());
    }

    /// A server with the account `@bob` in the state given.
    fn follow_server(user: &'static str) -> (String, Calls) {
        serve(Arc::new(move |path, _, _| match path {
            "/api/users/show" => reply(200, user),
            "/api/following/create"
            | "/api/following/delete"
            | "/api/following/requests/cancel" => reply(200, r#"{"id":"u-bob"}"#),
            _ => reply(404, "{}"),
        }))
    }

    fn run_follow(c: &Ctx, actor: &str, on: bool) -> (Result<()>, String) {
        let mut out = Vec::new();
        let r = follow(c, &mut out, actor, on);
        (r, String::from_utf8(out).unwrap())
    }

    #[rstest]
    #[case::follows(
        r#"{"id":"u-bob","username":"bob","host":null}"#,
        true,
        "following @bob@H\n",
        r#"{"id":"u-bob"}"#,
        Some("/api/following/create")
    )]
    #[case::already(
        r#"{"id":"u-bob","username":"bob","isFollowing":true}"#,
        true,
        "already following @bob@H\n",
        r#"{"already":true,"id":"u-bob"}"#,
        None
    )]
    #[case::locked(
        r#"{"id":"u-bob","username":"bob","isLocked":true}"#,
        true,
        "asked to follow @bob@H; they approve their followers\n",
        r#"{"id":"u-bob","requested":true}"#,
        Some("/api/following/create")
    )]
    #[case::pending(
        r#"{"id":"u-bob","username":"bob","hasPendingFollowRequestFromYou":true}"#,
        true,
        "already asked to follow @bob@H\n",
        r#"{"already":true,"id":"u-bob","requested":true}"#,
        None
    )]
    #[case::unfollows(
        r#"{"id":"u-bob","username":"bob","host":"mstdn.test","isFollowing":true}"#,
        false,
        "unfollowed @bob@mstdn.test\n",
        r#"{"deleted":"u-bob"}"#,
        Some("/api/following/delete")
    )]
    #[case::takes_back(
        r#"{"id":"u-bob","username":"bob","hasPendingFollowRequestFromYou":true}"#,
        false,
        "took back the request to follow @bob@H\n",
        r#"{"deleted":"u-bob","requested":true}"#,
        Some("/api/following/requests/cancel")
    )]
    fn follow_and_unfollow_say_what_they_did(
        #[case] user: &'static str,
        #[case] on: bool,
        #[case] said: &str,
        #[case] json: &str,
        #[case] wrote: Option<&str>,
    ) {
        let (url, calls) = follow_server(user);
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let host = api::authority(&url);
        let (r, out) = run_follow(&ctx(&url, dir.path(), false), "@bob", on);
        r.unwrap();
        assert_eq!(out, said.replace("@H", &format!("@{host}")));
        let (r, out) = run_follow(&ctx(&url, dir.path(), true), "bob", on);
        r.unwrap();
        assert_eq!(out.trim_end(), json);
        let writes: Vec<String> = paths(&calls)
            .into_iter()
            .filter(|p| p != "/api/users/show")
            .collect();
        match wrote {
            Some(p) => {
                assert_eq!(writes, [p, p]);
                assert_eq!(body(&calls, p), json!({"i": FAKE, "userId": "u-bob"}));
            }
            None => assert!(writes.is_empty(), "{writes:?}"),
        }
    }

    #[test]
    fn unfollowing_an_account_not_followed_or_yourself_is_a_usage_error() {
        let (url, calls) = follow_server(r#"{"id":"u-bob","username":"bob"}"#);
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let e = run_follow(&ctx(&url, dir.path(), false), "bob", false)
            .0
            .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert!(e.message().starts_with("you do not follow @bob@"), "{e:?}");
        let (url2, _) = follow_server(r#"{"id":"u-me","username":"me"}"#);
        login(dir.path(), &url2, crate::miauth::PERMISSIONS);
        let e = run_follow(&ctx(&url2, dir.path(), false), "me", true)
            .0
            .unwrap_err();
        assert_eq!(e.message(), "you cannot follow yourself");
        assert_eq!(paths(&calls), ["/api/users/show"]);
    }

    #[test]
    fn follow_with_a_read_only_login_or_another_servers_address_sends_nothing() {
        let (url, calls) = follow_server(r#"{"id":"u-bob","username":"bob"}"#);
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, &["read:account"]);
        let e = run_follow(&ctx(&url, dir.path(), false), "bob", true)
            .0
            .unwrap_err();
        assert!(e.message().contains("it lacks write:following"), "{e:?}");
        let e = run_follow(
            &ctx(&url, dir.path(), false),
            "https://other.test/@bob",
            true,
        )
        .0
        .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert!(e.hint().unwrap().contains("@name@host"), "{e:?}");
        assert!(paths(&calls).is_empty());
    }

    // An account of another server is followed once its server accepts:
    // until then the account is asked again after the follow, and the
    // output says the follow is pending; one accepted at once is followed.
    #[rstest]
    #[case::pending(
        false,
        "asked to follow @bob@mstdn.test; pending until their server accepts (msky follows --pending lists it)\n",
        r#"{"id":"u-bob","requested":true}"#
    )]
    #[case::accepted_at_once(true, "following @bob@mstdn.test\n", r#"{"id":"u-bob"}"#)]
    fn following_an_account_of_another_server_says_it_is_pending_until_accepted(
        #[case] accepted: bool,
        #[case] said: &str,
        #[case] json: &str,
    ) {
        let (url, calls) = serve(Arc::new(move |path, _, n| match path {
            "/api/users/show" if n % 3 == 2 && accepted => reply(
                200,
                r#"{"id":"u-bob","username":"bob","host":"mstdn.test","isFollowing":true}"#,
            ),
            "/api/users/show" => reply(
                200,
                r#"{"id":"u-bob","username":"bob","host":"mstdn.test"}"#,
            ),
            "/api/following/create" => reply(200, r#"{"id":"u-bob"}"#),
            _ => reply(404, "{}"),
        }));
        let dir = tempfile::tempdir().unwrap();
        login(dir.path(), &url, crate::miauth::PERMISSIONS);
        let (r, out) = run_follow(&ctx(&url, dir.path(), false), "@bob@mstdn.test", true);
        r.unwrap();
        assert_eq!(out, said);
        assert_eq!(
            paths(&calls),
            [
                "/api/users/show",
                "/api/following/create",
                "/api/users/show"
            ]
        );
        let (r, out) = run_follow(&ctx(&url, dir.path(), true), "bob@mstdn.test", true);
        r.unwrap();
        assert_eq!(out.trim_end(), json);
    }
}
