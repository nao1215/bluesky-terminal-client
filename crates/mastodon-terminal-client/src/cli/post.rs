//! `mstdn post`, with the options of msky's and bsky's. Everything that can
//! be told before anything is sent (the length as the server counts it, the
//! number of files, a file that cannot be read or is too large, a login
//! that may not write) is a usage error, and nothing is sent. Every write
//! is sent once and never tried again; the status goes with an
//! `Idempotency-Key`, so a server that got it twice makes one.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use unicode_segmentation::UnicodeSegmentation;

use super::{Ctx, format, json_line, parse_status, text};
use crate::api::types::{Account, Limits, Status};
use crate::api::{self, Client};
use crate::error::{Error, Result};
use crate::text::{drawable, one_line};

/// The most pictures a post has, as bsky's and msky's `post` take them.
pub(super) const MAX_IMAGES: usize = 4;

/// How long the post waits for the server to process its files.
const PROCESSING_TIMEOUT: Duration = Duration::from_secs(300);

/// Who may see a post, by the names Mastodon's API gives them; msky's
/// names are taken too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Visibility {
    /// Everyone, on every timeline.
    Public,
    /// Everyone, but not on the public timelines (quiet public).
    #[value(alias = "home")]
    Unlisted,
    /// Your followers only.
    #[value(alias = "followers")]
    Private,
    /// The accounts the post mentions only.
    Direct,
}

impl Visibility {
    /// Its name, as Mastodon's API takes it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Unlisted => "unlisted",
            Visibility::Private => "private",
            Visibility::Direct => "direct",
        }
    }
}

/// What `mstdn post` was given.
#[derive(Debug, Clone)]
pub(crate) struct PostArgs {
    pub text: String,
    pub images: Vec<PathBuf>,
    pub alts: Vec<String>,
    pub video: Option<PathBuf>,
    pub reply: Option<String>,
    pub quote: Option<String>,
    pub cw: Option<String>,
    pub sensitive: bool,
    pub visibility: Visibility,
    pub dry_run: bool,
}

/// Time as the wait for processed files sees it, so the tests wait
/// without sleeping.
pub(super) trait Clock {
    fn sleep(&mut self, d: Duration);
}

/// The clock on the wall.
pub(super) struct WallClock;

impl Clock for WallClock {
    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// A file read from disk, to be uploaded.
struct Local {
    path: PathBuf,
    name: String,
    data: Vec<u8>,
    alt: String,
    video: bool,
}

fn graphemes(s: &str) -> usize {
    s.graphemes(true).count()
}

/// How long `text` is as Mastodon counts it: in graphemes, an address as
/// `per_url` characters whatever its length, and a mention of an account of
/// another server (`@name@host`) as `@name`. As Mastodon does, the text is
/// counted with each address put as that many `x` and each mention as
/// `@name`.
pub(crate) fn counted(text: &str, per_url: usize) -> usize {
    let mut countable = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let free = prev.is_none_or(|p| !(p.is_alphanumeric() || p == '_' || p == '/'));
        if free {
            if let Some(len) = url_len(rest) {
                countable.extend(std::iter::repeat_n('x', per_url));
                i += len;
                prev = Some('x');
                continue;
            }
            if let Some((name, len)) = remote_mention(rest) {
                countable.push_str(&rest[..1 + name]);
                i += len;
                prev = Some('x');
                continue;
            }
        }
        let Some(c) = rest.chars().next() else { break };
        countable.push(c);
        prev = Some(c);
        i += c.len_utf8();
    }
    graphemes(&countable)
}

/// The length in bytes of the address `s` starts with, if it starts with
/// one: `http://` or `https://` and something, up to a space, less the
/// punctuation that ends a sentence after it.
fn url_len(s: &str) -> Option<usize> {
    let lower: String = s.chars().take(8).collect::<String>().to_ascii_lowercase();
    let scheme = if lower.starts_with("https://") {
        8
    } else if lower.starts_with("http://") {
        7
    } else {
        return None;
    };
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    let url = s[..end].trim_end_matches(['.', ',', ':', ';', '!', '?', '\'', '"', ')', ']', '}']);
    (url.len() > scheme).then_some(url.len())
}

/// The name's length and the bytes taken by a mention of an account of
/// another server, `@name@host`, that `s` starts with.
fn remote_mention(s: &str) -> Option<(usize, usize)> {
    let rest = s.strip_prefix('@')?;
    let name = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if name == 0 {
        return None;
    }
    let after = rest[name..].strip_prefix('@')?;
    let host = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        .unwrap_or(after.len());
    let host = after[..host].trim_end_matches(['.', '-']).len();
    if host == 0 || !after[..host].contains('.') {
        return None;
    }
    Some((name, 1 + name + 1 + host))
}

/// The post to make, as the checks that need no server leave it.
struct Draft {
    text: String,
    cw: Option<String>,
    files: Vec<Local>,
    reply: Option<(Option<String>, String)>,
}

/// Check what `post` was given and read its files, before anything is
/// asked of the server.
fn draft(a: &PostArgs, stdin: &mut dyn Read) -> Result<Draft> {
    let body = if a.text == "-" {
        let mut s = String::new();
        stdin
            .read_to_string(&mut s)
            .map_err(|e| Error::io(format!("cannot read the post from stdin: {e}")))?;
        s
    } else {
        a.text.clone()
    };
    if a.video.is_some() && !a.images.is_empty() {
        return Err(Error::usage(
            "a post can have up to 4 pictures or one video, not both",
        ));
    }
    if a.images.len() > MAX_IMAGES {
        return Err(Error::usage(format!(
            "a post can have at most {MAX_IMAGES} pictures"
        )));
    }
    let paths: Vec<(PathBuf, bool)> = a
        .images
        .iter()
        .map(|p| (p.clone(), false))
        .chain(a.video.clone().map(|v| (v, true)))
        .collect();
    if a.alts.len() > paths.len() {
        return Err(Error::usage("there is more --alt than pictures and videos"));
    }
    let cw = match a.cw.as_deref() {
        Some(c) if c.trim().is_empty() => {
            return Err(Error::usage("--cw is empty")
                .with_hint("give the warning shown in place of the text"));
        }
        c => c.map(|c| c.trim().to_string()),
    };
    // The server trims the text; it is counted as it will be kept.
    let text = body.trim().to_string();
    if text.is_empty() && paths.is_empty() {
        return Err(
            Error::usage("the post is empty").with_hint("give its text, or --image or --video")
        );
    }
    let reply = a.reply.as_deref().map(parse_status).transpose()?;
    if let Some(q) = &a.quote {
        parse_status(q)?;
    }
    let mut files = Vec::new();
    for (i, (path, video)) in paths.into_iter().enumerate() {
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
            video,
        });
    }
    Ok(Draft {
        text,
        cw,
        files,
        reply,
    })
}

/// What the server's limits say against `d`, if anything.
fn too_much(d: &Draft, l: &Limits, host: &str) -> Result<()> {
    let n = counted(&d.text, l.characters_reserved_per_url) + d.cw.as_deref().map_or(0, graphemes);
    if n > l.max_characters {
        let with_cw = if d.cw.is_some() {
            " with its content warning"
        } else {
            ""
        };
        return Err(Error::usage(format!(
            "the post is {n} characters{with_cw}; {host} takes up to {}",
            l.max_characters
        ))
        .with_hint(format!(
            "an address counts as {} characters, and @name@host as @name",
            l.characters_reserved_per_url
        )));
    }
    if d.files.len() > l.max_media_attachments {
        return Err(Error::usage(format!(
            "{host} takes up to {} files on a post",
            l.max_media_attachments
        )));
    }
    if let Some(f) = d
        .files
        .iter()
        .find(|f| graphemes(&f.alt) > l.description_limit)
    {
        // A file is named as it is uploaded: the full-screen client picks
        // files by absolute path, and a long one buries the limit.
        return Err(Error::usage(format!(
            "the alt text of {} is {} characters; {host} takes up to {}",
            f.name,
            graphemes(&f.alt),
            l.description_limit
        )));
    }
    for f in &d.files {
        let max = if f.video {
            l.video_size_limit
        } else {
            l.image_size_limit
        };
        if let Some(max) = max
            && f.data.len() as u64 > max
        {
            return Err(Error::usage(format!(
                "{} is {} bytes; {host} takes files up to {max} bytes",
                f.name,
                f.data.len()
            )));
        }
    }
    Ok(())
}

/// The account, for its name. A token that may not read it (an
/// `MSTDN_TOKEN` made with `write` alone) is no reason to stop.
fn me(ctx: &Ctx, client: &Client) -> Result<Option<Account>> {
    match client.me() {
        Ok(a) => Ok(Some(a)),
        Err(e) if e.message().contains("scope") => Ok(None),
        Err(e) => Err(ctx.explained(e)),
    }
}

/// The status a reply answers, by its ID on the server posted to: one
/// given by its address on another server is found there with a search
/// that has the server fetch it, as Mastodon's web client does.
fn replied(ctx: &Ctx, client: &Client, typed: &str, at: Option<&str>, id: &str) -> Result<Status> {
    match at {
        Some(o) if o != client.origin() => {
            let found = client
                .search(typed.trim(), "statuses", true, 1, 0)
                .map_err(|e| ctx.explained(e))?;
            found.statuses.into_iter().next().ok_or_else(|| {
                Error::usage(format!(
                    "{} cannot find the post given to --reply",
                    client.host()
                ))
                .with_hint(
                    "check the address, or give the post's address on the server you post on",
                )
            })
        }
        _ => client.status(id).map_err(|e| ctx.explained(e)),
    }
}

/// `mstdn post`.
pub(super) fn post(
    ctx: &Ctx,
    out: &mut dyn Write,
    a: &PostArgs,
    stdin: &mut dyn Read,
    clock: &mut dyn Clock,
) -> Result<()> {
    let d = draft(a, stdin)?;
    let mut scopes = vec!["write:statuses"];
    if !d.files.is_empty() {
        scopes.push("write:media");
    }
    let client = ctx.writer(&scopes, "post")?;
    // The limits are the server's own, from its instance's configuration.
    let limits = client.posting_limits(a.quote.is_some())?;
    too_much(&d, &limits, client.host())?;
    // The account is asked for only to say who would post.
    let me = if a.dry_run { me(ctx, &client)? } else { None };
    let reply = match (&d.reply, a.reply.as_deref()) {
        (Some((at, id)), Some(typed)) => Some(replied(ctx, &client, typed, at.as_deref(), id)?),
        _ => None,
    };
    let quote = a
        .quote
        .as_deref()
        .map(|typed| {
            let (at, id) = parse_status(typed)?;
            replied(ctx, &client, typed, at.as_deref(), &id)
        })
        .transpose()?;
    if a.dry_run {
        return dry_run(
            ctx,
            out,
            &client,
            a,
            &d,
            me.as_ref(),
            reply.as_ref(),
            quote.as_ref(),
            &limits,
        );
    }
    let made = submit(
        &client,
        a,
        &d,
        reply.as_ref().map(|r| r.id.as_str()),
        clock,
        &quote
            .as_ref()
            .map_or_else(|| json!({}), |q| json!({"quoted_status_id": q.id})),
        |e| ctx.explained(e),
    )?;
    let url = format::status_url(client.origin(), &made);
    if ctx.json {
        return json_line(
            out,
            &json!({"action": "post", "target": null, "id": made.id, "url": url, "already": false, "uri": made.uri}),
        );
    }
    text(out, &format!("{url}\n"))
}

/// The same validation, uploads, processing wait and post used by the composer.
pub(crate) fn composed(client: &Client, a: &PostArgs, extra: &serde_json::Value) -> Result<Status> {
    let d = draft(a, &mut a.text.as_bytes())?;
    let quote =
        extra.get("quoted_status_id").is_some() || extra.get("quote_approval_policy").is_some();
    too_much(&d, &client.posting_limits(quote)?, client.host())?;
    submit(
        client,
        a,
        &d,
        a.reply.as_deref(),
        &mut WallClock,
        extra,
        |e| e,
    )
}

fn submit(
    client: &Client,
    a: &PostArgs,
    d: &Draft,
    reply: Option<&str>,
    clock: &mut dyn Clock,
    extra: &serde_json::Value,
    explain: impl Fn(Error) -> Error,
) -> Result<Status> {
    let mut ids = Vec::new();
    for f in &d.files {
        let alt = (!f.alt.is_empty()).then_some(f.alt.as_str());
        let (ready, m) = client.upload(&f.name, &f.data, alt).map_err(&explain)?;
        if !ready {
            wait_processed(client, &m.id, &f.path, clock)?;
        }
        ids.push(m.id);
    }
    let mut body = json!({"status": d.text, "visibility": a.visibility.name()});
    if let Some(cw) = &d.cw {
        body["spoiler_text"] = json!(cw);
    }
    if a.sensitive {
        body["sensitive"] = json!(true);
    }
    if let Some(id) = reply {
        body["in_reply_to_id"] = json!(id);
    }
    if !ids.is_empty() {
        body["media_ids"] = json!(ids);
    }
    if let (Some(extra), Some(body)) = (extra.as_object(), body.as_object_mut()) {
        body.extend(extra.clone());
    }
    let key = api::random_hex("mstdn-")?;
    client.create_status(&body, &key).map_err(|e| {
        let e = explain(e);
        if ids.is_empty() {
            e
        } else {
            let hint = e.hint().map(|h| format!("{h}; ")).unwrap_or_default();
            e.with_hint(format!(
                "{hint}the files uploaded for it are not attached to anything, and the server removes them in a day"
            ))
        }
    })
}

/// Ask every second (then less often) whether the server has processed
/// the file `id` uploaded from `path`, for up to five minutes.
fn wait_processed(
    client: &Client,
    id: &str,
    path: &std::path::Path,
    clock: &mut dyn Clock,
) -> Result<()> {
    let mut waited = Duration::ZERO;
    let mut step = Duration::from_secs(1);
    loop {
        clock.sleep(step);
        waited += step;
        if client.media(id)? {
            return Ok(());
        }
        if waited >= PROCESSING_TIMEOUT {
            return Err(Error::api(format!(
                "{} has not finished processing {} after {} s",
                client.host(),
                path.display(),
                waited.as_secs()
            ))
            .with_hint("nothing was posted; try again later, or with a smaller file"));
        }
        step = (step * 2).min(Duration::from_secs(5));
    }
}

/// `post --dry-run`: what would be sent. Nothing is uploaded or posted.
#[expect(
    clippy::too_many_arguments,
    reason = "what the dry run shows, each a part of the post or the server"
)]
fn dry_run(
    ctx: &Ctx,
    out: &mut dyn Write,
    client: &Client,
    a: &PostArgs,
    d: &Draft,
    me: Option<&Account>,
    reply: Option<&Status>,
    quote: Option<&Status>,
    limits: &Limits,
) -> Result<()> {
    let n =
        counted(&d.text, limits.characters_reserved_per_url) + d.cw.as_deref().map_or(0, graphemes);
    if ctx.json {
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
        return json_line(
            out,
            &json!({
                "dryRun": true,
                "host": client.host(),
                "username": me.map(|u| &u.username),
                "post": {
                    "text": d.text,
                    "cw": d.cw,
                    "sensitive": a.sensitive || d.cw.is_some(),
                    "visibility": a.visibility.name(),
                    "replyId": reply.map(|r| &r.id),
                    "quoteId": quote.map(|q| &q.id),
                    "files": files,
                },
                "characters": n,
                "maxCharacters": limits.max_characters,
            }),
        );
    }
    let mut s = format!("dry run: nothing was sent to {}\n", client.host());
    let who = me.map_or_else(
        || "the account of MSTDN_TOKEN".to_string(),
        |u| format!("@{}@{}", one_line(&u.username), client.host()),
    );
    s.push_str(&format!("as {who}, {}\n", a.visibility.name()));
    if let Some(cw) = &d.cw {
        s.push_str(&format!("cw: {}\n", one_line(cw)));
    }
    if a.sensitive {
        s.push_str("sensitive: the files are hidden until opened\n");
    }
    if let Some(r) = reply {
        s.push_str(&format!(
            "reply to: {}\n",
            format::status_url(client.origin(), r)
        ));
    }
    if let Some(q) = quote {
        s.push_str(&format!(
            "quote: {}\n",
            format::status_url(client.origin(), q)
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
    s.push_str(&format!(
        "text ({n} of {} characters):\n",
        limits.max_characters
    ));
    s.push_str(&drawable(&d.text));
    s.push('\n');
    text(out, &s)
}

#[cfg(test)]
mod tests;
