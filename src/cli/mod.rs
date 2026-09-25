//! The commands: Bluesky from scripts and pipes, as the account in use (or
//! the one `-a` names), with the API code, the settings and the post
//! assembly the client uses. `bsky` with no command opens the client.
//!
//! Every command prints text for reading, or with `--json` the server's own
//! objects (one JSON value per line for a list) and, for a write, what the
//! server answered. Writes are sent once and never tried again.

mod format;

use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::api::types::{FeedItem, Notification, Post, Profile};
use crate::api::{self, Client};
use crate::compose::{Attachment, send_post};
use crate::config::{AccountStore, Environment, Session, SettingsStore};
use crate::error::{Error, Kind, Result};

/// The commands besides `logout`, which main handles.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// The following timeline, newest first.
    #[command(visible_alias = "tl")]
    Timeline {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// A custom feed: its at:// URI, its bsky.app address, or the name of a
    /// pinned feed.
    Feed {
        feed: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// A post and its replies.
    Thread { post: String },
    /// Notifications, newest first.
    #[command(visible_alias = "notif")]
    Notifications {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Mark them seen, which nothing else here does.
        #[arg(long)]
        seen: bool,
    },
    /// Posts that match QUERY, or accounts with --accounts.
    Search {
        query: String,
        #[arg(long)]
        accounts: bool,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// A profile; your own without ACTOR.
    Profile { actor: Option<String> },
    /// The accounts that follow ACTOR (you by default).
    Followers {
        actor: Option<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// The accounts ACTOR follows (you by default).
    Follows {
        actor: Option<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// Send a post; TEXT `-` reads it from stdin.
    Post {
        text: String,
        /// A picture to attach, up to four times.
        #[arg(long = "image", value_name = "PATH")]
        images: Vec<PathBuf>,
        /// Alt text, one per --image or --video, in the same order.
        #[arg(long = "alt", value_name = "TEXT")]
        alts: Vec<String>,
        /// A video to attach, alone.
        #[arg(long, value_name = "PATH")]
        video: Option<PathBuf>,
        /// The post this one replies to.
        #[arg(long, value_name = "POST")]
        reply: Option<String>,
        /// The post this one quotes.
        #[arg(long, value_name = "POST")]
        quote: Option<String>,
    },
    /// The accounts that liked a post, newest first.
    Likes {
        post: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// The accounts that reposted a post, newest first.
    Reposts {
        post: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// The lists ACTOR made (you by default).
    Lists {
        actor: Option<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// The members of a list: its at:// URI or its bsky.app address.
    List {
        list: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// Like a post.
    Like { post: String },
    /// Remove your like of a post.
    Unlike { post: String },
    /// Repost a post.
    Repost { post: String },
    /// Remove your repost of a post.
    Unrepost { post: String },
    /// Follow an account.
    Follow { actor: String },
    /// Unfollow an account.
    Unfollow { actor: String },
    /// Mute an account.
    Mute { actor: String },
    /// Unmute an account.
    Unmute { actor: String },
    /// The accounts you muted.
    Mutes {
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// Block an account.
    Block { actor: String },
    /// Unblock an account.
    Unblock { actor: String },
    /// The accounts you blocked.
    Blocks {
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// Delete one of your own posts.
    Delete { post: String },
    /// Report a post or an account to Bluesky's moderators.
    Report {
        /// A post (at:// URI or bsky.app address) or an account.
        target: String,
        #[arg(long, value_enum)]
        reason: ReportReason,
        /// What the moderators should know.
        #[arg(long, value_name = "TEXT")]
        comment: Option<String>,
    },
    /// The account's app passwords; add or revoke one.
    AppPasswords {
        #[command(subcommand)]
        action: Option<AppPasswordAction>,
    },
    /// Log an account in and make it the one in use.
    Login {
        /// The handle or email; asked for when left out.
        identifier: Option<String>,
        /// Read the password from stdin rather than the terminal.
        #[arg(long)]
        password_stdin: bool,
    },
    /// The logged-in accounts, the one in use marked.
    Accounts,
    /// Direct messages: the conversations; with ACTOR the messages with
    /// them; with TEXT too, send it (`-` reads it from stdin).
    Chat {
        actor: Option<String>,
        text: Option<String>,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
}

/// Why a post or an account is reported: `com.atproto.moderation.defs`.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ReportReason {
    Spam,
    Violation,
    Misleading,
    Sexual,
    Rude,
    Other,
}

impl ReportReason {
    fn token(self) -> &'static str {
        match self {
            ReportReason::Spam => "com.atproto.moderation.defs#reasonSpam",
            ReportReason::Violation => "com.atproto.moderation.defs#reasonViolation",
            ReportReason::Misleading => "com.atproto.moderation.defs#reasonMisleading",
            ReportReason::Sexual => "com.atproto.moderation.defs#reasonSexual",
            ReportReason::Rude => "com.atproto.moderation.defs#reasonRude",
            ReportReason::Other => "com.atproto.moderation.defs#reasonOther",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum AppPasswordAction {
    /// Make one; its password is printed once.
    Add {
        name: String,
        /// Allow it direct messages.
        #[arg(long)]
        privileged: bool,
    },
    /// Revoke one by its name.
    Revoke { name: String },
}

/// What a command runs with.
pub struct Ctx<'a> {
    pub dir: &'a std::path::Path,
    pub accounts: &'a AccountStore,
    /// The account `-a` named, or the one in use.
    pub session: Option<Session>,
    pub service: &'a str,
    pub json: bool,
}

impl Ctx<'_> {
    fn client(&self) -> Result<Client> {
        let session = self.session.clone().ok_or_else(|| {
            Error::new(Kind::Usage, "no account is logged in")
                .with_hint("run `bsky login`, or `bsky` to log in in the client")
        })?;
        let store = self.accounts.store_for(&session.did);
        Ok(Client::new(session, Some(store)))
    }
}

/// Run `cmd`, writing to stdout.
pub fn run(cmd: Command, ctx: &Ctx) -> Result<()> {
    // A reader that stopped reading (`| head`) is not an error: what it
    // wanted it has, so the command ends there, as the tools it is piped
    // into expect.
    match run_to(cmd, ctx) {
        Err(e) if e.message() == PIPE_CLOSED => Ok(()),
        r => r,
    }
}

/// What `write_err` says when the reader has closed the pipe.
const PIPE_CLOSED: &str = "the output was closed";

fn run_to(cmd: Command, ctx: &Ctx) -> Result<()> {
    let mut out = io::stdout().lock();
    let o = &mut out;
    match cmd {
        Command::Timeline { limit } => timeline(ctx, o, limit),
        Command::Feed { feed, limit } => custom_feed(ctx, o, &feed, limit),
        Command::Thread { post } => thread(ctx, o, &post),
        Command::Notifications { limit, seen } => notifications(ctx, o, limit, seen),
        Command::Search {
            query,
            accounts,
            limit,
        } => search(ctx, o, &query, accounts, limit),
        Command::Profile { actor } => profile(ctx, o, actor.as_deref()),
        Command::Followers { actor, limit } => graph(
            ctx,
            o,
            actor.as_deref(),
            limit,
            "app.bsky.graph.getFollowers",
            "followers",
        ),
        Command::Follows { actor, limit } => graph(
            ctx,
            o,
            actor.as_deref(),
            limit,
            "app.bsky.graph.getFollows",
            "follows",
        ),
        Command::Post {
            text,
            images,
            alts,
            video,
            reply,
            quote,
        } => post(ctx, o, &text, images, alts, video, reply, quote),
        Command::Likes { post, limit } => likes(ctx, o, &post, limit),
        Command::Reposts { post, limit } => reposts(ctx, o, &post, limit),
        Command::Lists { actor, limit } => lists(ctx, o, actor.as_deref(), limit),
        Command::List { list, limit } => list_members(ctx, o, &list, limit),
        Command::Like { post } => like(ctx, o, &post, true),
        Command::Unlike { post } => like(ctx, o, &post, false),
        Command::Repost { post } => repost(ctx, o, &post, true),
        Command::Unrepost { post } => repost(ctx, o, &post, false),
        Command::Follow { actor } => follow(ctx, o, &actor, true),
        Command::Unfollow { actor } => follow(ctx, o, &actor, false),
        Command::Mute { actor } => mute(ctx, o, &actor, true),
        Command::Unmute { actor } => mute(ctx, o, &actor, false),
        Command::Mutes { limit } => own_list(ctx, o, limit, "app.bsky.graph.getMutes", "mutes"),
        Command::Block { actor } => block(ctx, o, &actor, true),
        Command::Unblock { actor } => block(ctx, o, &actor, false),
        Command::Blocks { limit } => own_list(ctx, o, limit, "app.bsky.graph.getBlocks", "blocks"),
        Command::Delete { post } => delete(ctx, o, &post),
        Command::Login {
            identifier,
            password_stdin,
        } => login(ctx, o, identifier, password_stdin),
        Command::Accounts => accounts(ctx, o),
        Command::Report {
            target,
            reason,
            comment,
        } => report(ctx, o, &target, reason, comment.as_deref()),
        Command::AppPasswords { action } => app_passwords(ctx, o, action),
        Command::Chat { actor, text, limit } => chat(ctx, o, actor.as_deref(), text, limit),
    }
}

/// Print `v` as one line of JSON.
fn json_line(out: &mut dyn Write, v: &Value) -> Result<()> {
    writeln!(out, "{v}").map_err(write_err)
}

fn write_err(e: io::Error) -> Error {
    if e.kind() == io::ErrorKind::BrokenPipe {
        return Error::io(PIPE_CLOSED);
    }
    Error::io(format!("cannot write the output: {e}"))
}

fn text(out: &mut dyn Write, s: &str) -> Result<()> {
    out.write_all(s.as_bytes()).map_err(write_err)
}

/// Pages of `nsid` until `limit` items of `field` have come, or there are
/// no more; each raw item with what it reads as, `None` for one bsky cannot
/// read (printed with --json all the same).
fn collect(
    client: &Client,
    nsid: &str,
    query: &[(&str, &str)],
    field: &str,
    limit: usize,
) -> Result<Vec<Value>> {
    collect_by(|q| client.get_value(nsid, q), query, field, limit)
}

/// [`collect`] for a chat call, which goes through the chat service.
fn collect_chat(
    client: &Client,
    nsid: &str,
    query: &[(&str, &str)],
    field: &str,
    limit: usize,
) -> Result<Vec<Value>> {
    collect_by(|q| client.chat_value(nsid, q), query, field, limit)
}

fn collect_by(
    get: impl Fn(&[(&str, &str)]) -> Result<Value>,
    query: &[(&str, &str)],
    field: &str,
    limit: usize,
) -> Result<Vec<Value>> {
    let mut items = Vec::new();
    let mut cursor: Option<String> = None;
    while items.len() < limit {
        let page = (limit - items.len()).min(100).to_string();
        let mut q: Vec<(&str, &str)> = query.to_vec();
        q.push(("limit", &page));
        if let Some(c) = &cursor {
            q.push(("cursor", c));
        }
        let v = get(&q)?;
        let got = v
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let next = v.get("cursor").and_then(Value::as_str).map(str::to_string);
        let empty = got.is_empty();
        items.extend(got);
        if empty || next.is_none() || next == cursor {
            break;
        }
        cursor = next;
    }
    items.truncate(limit);
    Ok(items)
}

fn print_posts<'a>(
    ctx: &Ctx,
    out: &mut dyn Write,
    items: impl Iterator<Item = (&'a Value, Post)>,
) -> Result<()> {
    for (i, (raw, post)) in items.enumerate() {
        if ctx.json {
            json_line(out, raw)?;
        } else {
            if i > 0 {
                text(out, "\n")?;
            }
            text(out, &format::post(&post))?;
        }
    }
    Ok(())
}

fn timeline(ctx: &Ctx, out: &mut dyn Write, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let did = client.did().to_string();
    // As the client shows it: posts by the accounts followed and your own,
    // not reposts. A page can be mostly reposts, so pages are read until
    // `limit` posts are kept or there are no more.
    let mut kept: Vec<(Value, Post)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGES {
        if kept.len() >= limit {
            break;
        }
        let page = (limit - kept.len()).clamp(1, 100).to_string();
        let mut q: Vec<(&str, &str)> = vec![("limit", &page)];
        if let Some(c) = &cursor {
            q.push(("cursor", c));
        }
        let v = client.get_value("app.bsky.feed.getTimeline", &q)?;
        let got = v
            .get("feed")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let next = v.get("cursor").and_then(Value::as_str).map(str::to_string);
        let empty = got.is_empty();
        for raw in got {
            let Ok(item) = serde_json::from_value::<FeedItem>(raw.clone()) else {
                continue;
            };
            let Some(p) = crate::timeline::followed_posts(vec![item], &did)
                .into_iter()
                .next()
            else {
                continue;
            };
            if seen.insert(p.uri.clone()) {
                kept.push((raw, p));
            }
        }
        if empty || next.is_none() || next == cursor {
            break;
        }
        cursor = next;
    }
    kept.truncate(limit);
    print_posts(ctx, out, kept.iter().map(|(v, p)| (v, p.clone())))
}

/// How many pages the timeline reads at most for one `-n`: a timeline of
/// nothing but reposts would otherwise be read to its end.
const MAX_PAGES: usize = 20;

fn custom_feed(ctx: &Ctx, out: &mut dyn Write, feed: &str, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let uri = feed_uri(&client, feed)?;
    let raw = collect(
        &client,
        "app.bsky.feed.getFeed",
        &[("feed", &uri)],
        "feed",
        limit,
    )?;
    let posts: Vec<(Value, Post)> = raw
        .into_iter()
        .filter_map(|v| {
            let item: FeedItem = serde_json::from_value(v.clone()).ok()?;
            Some((v, item.post))
        })
        .collect();
    print_posts(ctx, out, posts.iter().map(|(v, p)| (v, p.clone())))
}

/// A feed's at:// URI from what was typed: the URI, a bsky.app feed
/// address, or the name of one of the account's pinned feeds.
fn feed_uri(client: &Client, feed: &str) -> Result<String> {
    if feed.starts_with("at://") {
        return Ok(feed.to_string());
    }
    if let Some((actor, rkey)) = format::bsky_app_path(feed, "feed") {
        let did = resolve_actor(client, &actor)?;
        return Ok(format!("at://{did}/app.bsky.feed.generator/{rkey}"));
    }
    let pinned = client.pinned_feeds()?;
    pinned
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case(feed.trim()))
        .map(|f| f.uri.clone())
        .ok_or_else(|| no_pinned_feed(feed, &pinned))
}

/// That no pinned feed is named `feed`, with the names there are.
fn no_pinned_feed(feed: &str, pinned: &[crate::api::types::FeedInfo]) -> Error {
    // Each name as text only: a feed's name is what its maker wrote, and a
    // control character in it would reach the terminal as a command.
    let names: Vec<String> = pinned.iter().map(|f| format::one_line(&f.name)).collect();
    Error::new(Kind::Usage, format!("no pinned feed is named {feed:?}")).with_hint(
        if names.is_empty() {
            "give the feed's at:// URI or its bsky.app address".to_string()
        } else {
            format!("pinned: {}", names.join(", "))
        },
    )
}

/// A DID from a handle, `@handle` or DID.
fn resolve_actor(client: &Client, actor: &str) -> Result<String> {
    let a = actor.trim().trim_start_matches('@');
    if a.starts_with("did:") {
        return Ok(a.to_string());
    }
    client.resolve_handle(a)
}

/// An at:// URI as given, without what a copied address can carry after
/// the record key: a trailing slash, a query, a fragment.
fn bare_at_uri(uri: &str) -> String {
    let uri = uri.split(['?', '#']).next().unwrap_or(uri);
    uri.trim_end_matches('/').to_string()
}

/// A post's at:// URI from its URI or its bsky.app address.
/// An at:// URI with its handle, if written with one, put as the DID it
/// stands for: the server and the checks of whose a record is compare DIDs.
fn with_did(client: &Client, uri: &str) -> Result<String> {
    let uri = bare_at_uri(uri);
    // `at://` alone is cut to `at:` by the trailing slashes taken off.
    let rest = uri.strip_prefix("at://").unwrap_or_default();
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    if authority.is_empty() {
        return Err(Error::new(
            Kind::Usage,
            format!("{uri:?} names no account after at://"),
        ));
    }
    if authority.starts_with("did:") {
        return Ok(uri.clone());
    }
    Ok(format!("at://{}{path}", resolve_actor(client, authority)?))
}

fn post_uri(client: &Client, post: &str) -> Result<String> {
    let p = post.trim();
    if p.starts_with("at://") {
        return with_did(client, p);
    }
    match format::bsky_app_path(p, "post") {
        Some((actor, rkey)) => {
            let did = resolve_actor(client, &actor)?;
            Ok(format!("at://{did}/app.bsky.feed.post/{rkey}"))
        }
        None => Err(Error::new(Kind::Usage, format!("{p} is not a post"))
            .with_hint("give its at:// URI or its https://bsky.app/profile/.../post/... address")),
    }
}

/// The post `post` names, as the server shows it to you now.
fn fetch_post(client: &Client, post: &str) -> Result<Post> {
    let uri = post_uri(client, post)?;
    client
        .posts(std::slice::from_ref(&uri))?
        .into_iter()
        .next()
        .ok_or_else(|| Error::api(format!("{uri} was not found")))
}

fn thread(ctx: &Ctx, out: &mut dyn Write, post: &str) -> Result<()> {
    let client = ctx.client()?;
    let uri = post_uri(&client, post)?;
    let raw = client.get_value(
        "app.bsky.feed.getPostThread",
        &[("uri", &uri), ("depth", "10"), ("parentHeight", "20")],
    )?;
    let thread = raw.get("thread").cloned().unwrap_or(Value::Null);
    if ctx.json {
        return json_line(out, &thread);
    }
    let node: crate::api::types::ThreadNode = serde_json::from_value(thread)
        .map_err(|e| Error::api(format!("the thread could not be read: {e}")))?;
    text(out, &format::thread(&node))
}

fn notifications(ctx: &Ctx, out: &mut dyn Write, limit: usize, seen: bool) -> Result<()> {
    let client = ctx.client()?;
    let raw = collect(
        &client,
        "app.bsky.notification.listNotifications",
        &[],
        "notifications",
        limit,
    )?;
    for (i, v) in raw.iter().enumerate() {
        if ctx.json {
            json_line(out, v)?;
            continue;
        }
        let Ok(n) = serde_json::from_value::<Notification>(v.clone()) else {
            continue;
        };
        if i > 0 {
            text(out, "\n")?;
        }
        let what = v
            .get("record")
            .and_then(|r| r.get("text"))
            .and_then(Value::as_str);
        text(out, &format::notification(&n, what))?;
    }
    if seen {
        client.update_seen(&seen_at(&raw))?;
    }
    Ok(())
}

/// The time to mark notifications seen up to: the newest of `raw`, by the
/// time it is rather than how its text sorts, as the client marks them.
fn seen_at(raw: &[Value]) -> String {
    api::newest(
        raw.iter()
            .filter_map(|v| v.get("indexedAt").and_then(Value::as_str)),
    )
    .unwrap_or_else(api::now)
}

fn search(ctx: &Ctx, out: &mut dyn Write, q: &str, accounts: bool, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    if accounts {
        let raw = collect(
            &client,
            "app.bsky.actor.searchActors",
            &[("q", q)],
            "actors",
            limit,
        )?;
        return print_profiles(ctx, out, &raw);
    }
    let raw = collect(
        &client,
        "app.bsky.feed.searchPosts",
        &[("q", q)],
        "posts",
        limit,
    )?;
    let posts: Vec<(Value, Post)> = raw
        .into_iter()
        .filter_map(|v| Some((v.clone(), serde_json::from_value(v).ok()?)))
        .collect();
    print_posts(ctx, out, posts.iter().map(|(v, p)| (v, p.clone())))
}

fn print_profiles(ctx: &Ctx, out: &mut dyn Write, raw: &[Value]) -> Result<()> {
    for v in raw {
        if ctx.json {
            json_line(out, v)?;
        } else if let Ok(p) = serde_json::from_value::<Profile>(v.clone()) {
            text(out, &format::account_line(&p))?;
        }
    }
    Ok(())
}

fn likes(ctx: &Ctx, out: &mut dyn Write, post: &str, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let uri = post_uri(&client, post)?;
    let raw = collect(
        &client,
        "app.bsky.feed.getLikes",
        &[("uri", &uri)],
        "likes",
        limit,
    )?;
    // Each like is {actor, createdAt}: the JSON keeps it whole, the text
    // shows who.
    for v in &raw {
        if ctx.json {
            json_line(out, v)?;
        } else if let Some(p) = v
            .get("actor")
            .and_then(|a| serde_json::from_value::<Profile>(a.clone()).ok())
        {
            text(out, &format::account_line(&p))?;
        }
    }
    Ok(())
}

fn reposts(ctx: &Ctx, out: &mut dyn Write, post: &str, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let uri = post_uri(&client, post)?;
    let raw = collect(
        &client,
        "app.bsky.feed.getRepostedBy",
        &[("uri", &uri)],
        "repostedBy",
        limit,
    )?;
    print_profiles(ctx, out, &raw)
}

fn lists(ctx: &Ctx, out: &mut dyn Write, actor: Option<&str>, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let actor = match actor {
        Some(a) => resolve_actor(&client, a)?,
        None => client.did().to_string(),
    };
    let raw = collect(
        &client,
        "app.bsky.graph.getLists",
        &[("actor", &actor)],
        "lists",
        limit,
    )?;
    for (i, v) in raw.iter().enumerate() {
        if ctx.json {
            json_line(out, v)?;
        } else {
            if i > 0 {
                text(out, "\n")?;
            }
            text(out, &format::list(v))?;
        }
    }
    Ok(())
}

/// A list's at:// URI from its URI or its bsky.app address.
fn list_uri(client: &Client, list: &str) -> Result<String> {
    let l = list.trim();
    if l.starts_with("at://") {
        return with_did(client, l);
    }
    match format::bsky_app_path(l, "lists") {
        Some((actor, rkey)) => {
            let did = resolve_actor(client, &actor)?;
            Ok(format!("at://{did}/app.bsky.graph.list/{rkey}"))
        }
        None => Err(Error::new(Kind::Usage, format!("{l} is not a list"))
            .with_hint("give its at:// URI or its https://bsky.app/profile/.../lists/... address")),
    }
}

fn list_members(ctx: &Ctx, out: &mut dyn Write, list: &str, limit: usize) -> Result<()> {
    let client = ctx.client()?;
    let uri = list_uri(&client, list)?;
    let raw = collect(
        &client,
        "app.bsky.graph.getList",
        &[("list", &uri)],
        "items",
        limit,
    )?;
    // Each item is {uri, subject}: the JSON keeps it whole, the text shows
    // the account.
    for v in &raw {
        if ctx.json {
            json_line(out, v)?;
        } else if let Some(p) = v
            .get("subject")
            .and_then(|a| serde_json::from_value::<Profile>(a.clone()).ok())
        {
            text(out, &format::account_line(&p))?;
        }
    }
    Ok(())
}

fn profile(ctx: &Ctx, out: &mut dyn Write, actor: Option<&str>) -> Result<()> {
    let client = ctx.client()?;
    let actor = actor
        .map(|a| a.trim().trim_start_matches('@').to_string())
        .unwrap_or_else(|| client.did().to_string());
    let raw = client.get_value("app.bsky.actor.getProfile", &[("actor", &actor)])?;
    if ctx.json {
        return json_line(out, &raw);
    }
    let p: Profile = serde_json::from_value(raw)
        .map_err(|e| Error::api(format!("the profile could not be read: {e}")))?;
    text(out, &format::profile(&p))
}

fn graph(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    limit: usize,
    nsid: &str,
    field: &str,
) -> Result<()> {
    let client = ctx.client()?;
    let actor = actor
        .map(|a| a.trim().trim_start_matches('@').to_string())
        .unwrap_or_else(|| client.did().to_string());
    let raw = collect(&client, nsid, &[("actor", &actor)], field, limit)?;
    print_profiles(ctx, out, &raw)
}

/// What a write answered, printed.
fn wrote(ctx: &Ctx, out: &mut dyn Write, v: Value, said: &str) -> Result<()> {
    if ctx.json {
        json_line(out, &v)
    } else {
        text(out, &format!("{said}\n"))
    }
}

#[allow(clippy::too_many_arguments)]
fn post(
    ctx: &Ctx,
    out: &mut dyn Write,
    text_arg: &str,
    images: Vec<PathBuf>,
    alts: Vec<String>,
    video: Option<PathBuf>,
    reply: Option<String>,
    quote: Option<String>,
) -> Result<()> {
    let body = if text_arg == "-" {
        let mut s = String::new();
        io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| Error::io(format!("cannot read the post from stdin: {e}")))?;
        s
    } else {
        text_arg.to_string()
    };
    if video.is_some() && !images.is_empty() {
        return Err(Error::new(
            Kind::Usage,
            "a post can have up to 4 pictures or one video, not both",
        ));
    }
    if images.len() > crate::media::MAX_POST_IMAGES {
        return Err(Error::new(
            Kind::Usage,
            format!(
                "a post can have at most {} pictures",
                crate::media::MAX_POST_IMAGES
            ),
        ));
    }
    if let Some(why) = api::post_length_problem(body.trim_end()) {
        return Err(Error::new(Kind::Usage, why));
    }
    let files: Vec<PathBuf> = images.into_iter().chain(video).collect();
    if alts.len() > files.len() {
        return Err(Error::new(
            Kind::Usage,
            "there is more --alt than pictures and videos",
        ));
    }
    let media: Vec<Attachment> = files
        .into_iter()
        .enumerate()
        .map(|(i, path)| Attachment {
            path,
            alt: alts.get(i).cloned().unwrap_or_default(),
        })
        .collect();
    let client = ctx.client()?;
    let reply = match reply {
        Some(r) => Some(fetch_post(&client, &r)?.reply_ref()),
        None => None,
    };
    let quote = match quote {
        Some(q) => Some(fetch_post(&client, &q)?.strong_ref()),
        None => None,
    };
    let settings = SettingsStore::new(ctx.dir).load().0;
    let env = Environment::read();
    let video_service = crate::config::video_service(&env, &settings).0;
    // The commands write English, but the post is in the language the
    // client would be shown in.
    let writer = crate::config::language(&settings, &env);
    let created = send_post(
        &client,
        &body,
        reply.as_ref(),
        quote.as_ref(),
        &media,
        &video_service,
        writer,
    )?;
    let said = created.uri.clone();
    wrote(
        ctx,
        out,
        json!({"uri": created.uri, "cid": created.cid}),
        &said,
    )
}

fn like(ctx: &Ctx, out: &mut dyn Write, post: &str, on: bool) -> Result<()> {
    let client = ctx.client()?;
    let p = fetch_post(&client, post)?;
    if on {
        if let Some(uri) = p.like_uri() {
            return wrote(
                ctx,
                out,
                json!({"uri": uri, "already": true}),
                &format!("already liked: {uri}"),
            );
        }
        let uri = client.like(&p.strong_ref())?;
        wrote(ctx, out, json!({"uri": uri}), &uri)
    } else {
        let Some(uri) = p.like_uri().map(str::to_string) else {
            return Err(Error::new(Kind::Usage, format!("{} is not liked", p.uri)));
        };
        client.unlike(&uri)?;
        wrote(ctx, out, json!({"deleted": uri}), &format!("removed {uri}"))
    }
}

fn repost(ctx: &Ctx, out: &mut dyn Write, post: &str, on: bool) -> Result<()> {
    let client = ctx.client()?;
    let p = fetch_post(&client, post)?;
    if on {
        if let Some(uri) = p.repost_uri() {
            return wrote(
                ctx,
                out,
                json!({"uri": uri, "already": true}),
                &format!("already reposted: {uri}"),
            );
        }
        let uri = client.repost(&p.strong_ref())?;
        wrote(ctx, out, json!({"uri": uri}), &uri)
    } else {
        let Some(uri) = p.repost_uri().map(str::to_string) else {
            return Err(Error::new(
                Kind::Usage,
                format!("{} is not reposted", p.uri),
            ));
        };
        client.unrepost(&uri)?;
        wrote(ctx, out, json!({"deleted": uri}), &format!("removed {uri}"))
    }
}

fn follow(ctx: &Ctx, out: &mut dyn Write, actor: &str, on: bool) -> Result<()> {
    let client = ctx.client()?;
    let actor = actor.trim().trim_start_matches('@');
    let p = client.profile(actor)?;
    if p.did == client.did() {
        return Err(Error::new(Kind::Usage, "you cannot follow yourself"));
    }
    let following = p.following_uri().map(str::to_string);
    if on {
        if let Some(uri) = following {
            return wrote(
                ctx,
                out,
                json!({"uri": uri, "already": true}),
                &format!("already following @{}", p.handle),
            );
        }
        let uri = client.follow(&p.did)?;
        wrote(
            ctx,
            out,
            json!({"uri": uri}),
            &format!("following @{}", p.handle),
        )
    } else {
        let Some(uri) = following else {
            return Err(Error::new(
                Kind::Usage,
                format!("you do not follow @{}", p.handle),
            ));
        };
        client.unfollow(&uri)?;
        wrote(
            ctx,
            out,
            json!({"deleted": uri}),
            &format!("unfollowed @{}", p.handle),
        )
    }
}

/// The account `actor` names, which is not you: what `what` is done to.
fn other_account(client: &Client, actor: &str, what: &str) -> Result<Profile> {
    let p = client.profile(actor.trim().trim_start_matches('@'))?;
    if p.did == client.did() {
        return Err(Error::new(
            Kind::Usage,
            format!("you cannot {what} yourself"),
        ));
    }
    Ok(p)
}

fn mute(ctx: &Ctx, out: &mut dyn Write, actor: &str, on: bool) -> Result<()> {
    let client = ctx.client()?;
    let p = other_account(&client, actor, "mute")?;
    match (on, p.muted()) {
        (true, true) => wrote(
            ctx,
            out,
            json!({"muted": p.did, "already": true}),
            &format!("already muted @{}", p.handle),
        ),
        (true, false) => {
            client.mute(&p.did)?;
            wrote(
                ctx,
                out,
                json!({"muted": p.did}),
                &format!("muted @{}", p.handle),
            )
        }
        (false, true) => {
            client.unmute(&p.did)?;
            wrote(
                ctx,
                out,
                json!({"unmuted": p.did}),
                &format!("unmuted @{}", p.handle),
            )
        }
        (false, false) => Err(match p.muting_list() {
            Some(list) => Error::new(
                Kind::Usage,
                format!("@{} is muted by your list {list:?}", p.handle),
            )
            .with_hint("take them off that list to unmute them"),
            None => Error::new(Kind::Usage, format!("you have not muted @{}", p.handle)),
        }),
    }
}

fn block(ctx: &Ctx, out: &mut dyn Write, actor: &str, on: bool) -> Result<()> {
    let client = ctx.client()?;
    let p = other_account(&client, actor, "block")?;
    let blocking = p.blocking_uri().map(str::to_string);
    match (on, blocking) {
        (true, Some(uri)) => wrote(
            ctx,
            out,
            json!({"uri": uri, "already": true}),
            &format!("already blocking @{}", p.handle),
        ),
        (true, None) => {
            let uri = client.block(&p.did)?;
            wrote(
                ctx,
                out,
                json!({"uri": uri}),
                &format!("blocked @{}", p.handle),
            )
        }
        (false, Some(uri)) => {
            client.unblock(&uri)?;
            wrote(
                ctx,
                out,
                json!({"deleted": uri}),
                &format!("unblocked @{}", p.handle),
            )
        }
        (false, None) => Err(Error::new(
            Kind::Usage,
            format!("you do not block @{}", p.handle),
        )),
    }
}

/// A list of accounts that belongs to the account in use (its mutes, its
/// blocks), newest first.
fn own_list(ctx: &Ctx, out: &mut dyn Write, limit: usize, nsid: &str, field: &str) -> Result<()> {
    let client = ctx.client()?;
    let raw = collect(&client, nsid, &[], field, limit)?;
    print_profiles(ctx, out, &raw)
}

/// The most a report's comment may hold (`com.atproto.moderation.createReport`).
const MAX_REPORT_GRAPHEMES: usize = 2000;
const MAX_REPORT_BYTES: usize = 20000;

fn report(
    ctx: &Ctx,
    out: &mut dyn Write,
    target: &str,
    reason: ReportReason,
    comment: Option<&str>,
) -> Result<()> {
    let comment = comment.map(str::trim).filter(|c| !c.is_empty());
    if let Some(c) = comment {
        // createReport's limits on the reason: over them the server
        // refuses it, and only after the post or account was looked up.
        let n = crate::api::grapheme_len(c);
        if n > MAX_REPORT_GRAPHEMES {
            return Err(Error::new(
                Kind::Usage,
                format!("the comment is {n} characters long; it can have {MAX_REPORT_GRAPHEMES}"),
            ));
        }
        if c.len() > MAX_REPORT_BYTES {
            return Err(Error::new(
                Kind::Usage,
                format!(
                    "the comment is {} bytes; the limit is {MAX_REPORT_BYTES}",
                    c.len()
                ),
            ));
        }
    }
    let client = ctx.client()?;
    let t = target.trim();
    let is_post = t.starts_with("at://") || format::bsky_app_path(t, "post").is_some();
    let (subject, what) = if is_post {
        let p = fetch_post(&client, t)?;
        (
            json!({"$type": "com.atproto.repo.strongRef", "uri": p.uri, "cid": p.cid}),
            p.uri.clone(),
        )
    } else {
        let did = resolve_actor(&client, t)?;
        (
            json!({"$type": "com.atproto.admin.defs#repoRef", "did": did}),
            format!("@{}", t.trim_start_matches('@')),
        )
    };
    let mut body = json!({"reasonType": reason.token(), "subject": subject});
    if let Some(c) = comment {
        body["reason"] = json!(c);
    }
    let answer = client.post_value("com.atproto.moderation.createReport", &body)?;
    wrote(ctx, out, answer, &format!("reported {what}"))
}

fn app_passwords(ctx: &Ctx, out: &mut dyn Write, action: Option<AppPasswordAction>) -> Result<()> {
    let client = ctx.client()?;
    // A session made with an app password may not manage app passwords.
    let hint = |e: Error| {
        let m = e.message();
        if m.contains("AuthRequired") || m.contains("InvalidToken") || m.contains("scope") {
            e.with_hint("log in with the account's password, not an app password, to manage them")
        } else {
            e
        }
    };
    match action {
        None => {
            let v = client
                .get_value("com.atproto.server.listAppPasswords", &[])
                .map_err(hint)?;
            let items = v
                .get("passwords")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for p in &items {
                if ctx.json {
                    json_line(out, p)?;
                } else {
                    let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("");
                    let privileged = if p.get("privileged").and_then(Value::as_bool) == Some(true) {
                        "  (direct messages)"
                    } else {
                        ""
                    };
                    text(
                        out,
                        &format!(
                            "{}  {}{privileged}\n",
                            s("name"),
                            format::time(s("createdAt"))
                        ),
                    )?;
                }
            }
            Ok(())
        }
        Some(AppPasswordAction::Add { name, privileged }) => {
            let v = client
                .post_value(
                    "com.atproto.server.createAppPassword",
                    &json!({"name": name, "privileged": privileged}),
                )
                .map_err(hint)?;
            let password = v.get("password").and_then(Value::as_str).unwrap_or("");
            wrote(
                ctx,
                out,
                v.clone(),
                &format!("{password}\n(the password of {name:?}; it is not shown again)"),
            )
        }
        Some(AppPasswordAction::Revoke { name }) => {
            client
                .post_value(
                    "com.atproto.server.revokeAppPassword",
                    &json!({"name": name}),
                )
                .map_err(hint)?;
            wrote(
                ctx,
                out,
                json!({"revoked": name}),
                &format!("revoked {name:?}"),
            )
        }
    }
}

fn delete(ctx: &Ctx, out: &mut dyn Write, post: &str) -> Result<()> {
    let client = ctx.client()?;
    let uri = post_uri(&client, post)?;
    let mine = format!("at://{}/", client.did());
    if !uri.starts_with(&mine) {
        return Err(Error::new(
            Kind::Usage,
            "you can only delete your own posts",
        ));
    }
    client.delete_post(&uri)?;
    wrote(ctx, out, json!({"deleted": uri}), &format!("deleted {uri}"))
}

fn login(
    ctx: &Ctx,
    out: &mut dyn Write,
    identifier: Option<String>,
    password_stdin: bool,
) -> Result<()> {
    let identifier = match identifier {
        Some(i) => i,
        None => prompt("Handle or email: ")?,
    };
    let password = if password_stdin {
        let mut line = String::new();
        io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|e| Error::io(format!("cannot read the password from stdin: {e}")))?;
        line.trim_end_matches(['\r', '\n']).to_string()
    } else {
        read_password()?
    };
    if identifier.trim().is_empty() || password.is_empty() {
        return Err(Error::new(
            Kind::Usage,
            "the handle and the password are both needed",
        ));
    }
    let session = api::login(ctx.service, identifier.trim(), &password)?;
    ctx.accounts.save(&session)?;
    wrote(
        ctx,
        out,
        json!({"did": session.did, "handle": session.handle, "service": session.service}),
        &format!("logged in as @{}", session.handle),
    )
}

fn prompt(label: &str) -> Result<String> {
    let mut err = io::stderr();
    let _ = write!(err, "{label}");
    let _ = err.flush();
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| Error::io(format!("cannot read from stdin: {e}")))?;
    Ok(line.trim().to_string())
}

/// The password, typed without echo. Without a terminal to type it in, use
/// --password-stdin.
fn read_password() -> Result<String> {
    use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, read};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    if !io::stdin().is_terminal() {
        return Err(
            Error::new(Kind::Usage, "there is no terminal to type the password in")
                .with_hint("pass it with --password-stdin"),
        );
    }
    let mut err = io::stderr();
    let _ = write!(err, "Password: ");
    let _ = err.flush();
    enable_raw_mode()
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot read the password: {e}")))?;
    let mut pw = String::new();
    let result = loop {
        match read() {
            Ok(Event::Key(k)) if k.kind != KeyEventKind::Release => match k.code {
                KeyCode::Enter => break Ok(()),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    break Err(Error::new(Kind::Usage, "cancelled"));
                }
                KeyCode::Backspace => {
                    pw.pop();
                }
                KeyCode::Char(c) => pw.push(c),
                _ => {}
            },
            Ok(Event::Paste(s)) => pw.push_str(&s),
            Ok(_) => {}
            Err(e) => break Err(Error::io(format!("cannot read the password: {e}"))),
        }
    };
    let _ = disable_raw_mode();
    let _ = writeln!(err);
    result.map(|()| pw)
}

fn accounts(ctx: &Ctx, out: &mut dyn Write) -> Result<()> {
    let current = ctx.accounts.current()?.map(|s| s.did);
    let all = ctx.accounts.list()?;
    if all.is_empty() && !ctx.json {
        return text(out, "not logged in\n");
    }
    for s in all {
        let used = current.as_deref() == Some(s.did.as_str());
        if ctx.json {
            json_line(
                out,
                &json!({"did": s.did, "handle": s.handle, "service": s.service, "current": used}),
            )?;
        } else {
            let mark = if used { "* " } else { "  " };
            text(out, &format!("{mark}@{}  {}\n", s.handle, s.did))?;
        }
    }
    Ok(())
}

fn chat(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    text_arg: Option<String>,
    limit: usize,
) -> Result<()> {
    let client = ctx.client()?;
    let me = client.did().to_string();
    let Some(actor) = actor else {
        let convos = collect_chat(&client, "chat.bsky.convo.listConvos", &[], "convos", limit)?;
        for v in &convos {
            if ctx.json {
                json_line(out, v)?;
            } else if let Ok(c) = serde_json::from_value::<crate::api::types::Convo>(v.clone()) {
                text(out, &format::convo(&c, &me))?;
            }
        }
        return Ok(());
    };
    // The message is read and checked before the conversation is asked
    // for: getConvoForMembers starts one when there is none, which a
    // message that cannot be sent must not leave behind.
    let body = match text_arg {
        Some(t) if t == "-" => {
            let mut s = String::new();
            io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| Error::io(format!("cannot read the message from stdin: {e}")))?;
            Some(s)
        }
        other => other,
    };
    if let Some(body) = &body {
        let body = body.trim_end();
        if body.trim().is_empty() {
            return Err(Error::new(
                Kind::Usage,
                crate::i18n::t("the message is empty"),
            ));
        }
        if let Some(why) = api::message_length_problem(body) {
            return Err(Error::new(Kind::Usage, why));
        }
    }
    let did = resolve_actor(&client, actor)?;
    let convo = client.convo_for(&did)?;
    if let Some(body) = body {
        let m = client.send_message(&convo.id, &body)?;
        return wrote(
            ctx,
            out,
            json!({"id": m.id, "convoId": convo.id, "text": m.text, "sentAt": m.sent_at}),
            &format!("sent to {}", format::convo_with(&convo, &me)),
        );
    }
    // The newest `limit`, oldest first, as a conversation reads.
    let mut messages = collect_chat(
        &client,
        "chat.bsky.convo.getMessages",
        &[("convoId", &convo.id)],
        "messages",
        limit,
    )?;
    messages.reverse();
    for v in &messages {
        if ctx.json {
            json_line(out, v)?;
        } else if let Some(m) = crate::api::types::ChatMessage::from_value(v) {
            text(out, &format::message(&m, &me, &convo))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // A URI copied with what follows the record key names the same record;
    // the key sent is the record's, not "p9/".
    #[rstest]
    #[case(
        "at://did:plc:me/app.bsky.feed.post/p9",
        "at://did:plc:me/app.bsky.feed.post/p9"
    )]
    #[case(
        "at://did:plc:me/app.bsky.feed.post/p9/",
        "at://did:plc:me/app.bsky.feed.post/p9"
    )]
    #[case(
        "at://did:plc:me/app.bsky.feed.post/p9?x=1",
        "at://did:plc:me/app.bsky.feed.post/p9"
    )]
    #[case(
        "at://did:plc:me/app.bsky.graph.list/l1#top",
        "at://did:plc:me/app.bsky.graph.list/l1"
    )]
    fn an_at_uri_loses_what_follows_its_record_key(#[case] given: &str, #[case] want: &str) {
        assert_eq!(bare_at_uri(given), want);
    }

    // An at:// URI with nothing after the scheme but what bare_at_uri takes
    // off was cut short and then sliced past its end.
    #[rstest]
    #[case("at://")]
    #[case("at:///")]
    #[case("at://?x=1")]
    #[case("at://#top")]
    #[case("at:///app.bsky.feed.post/p1")]
    fn an_at_uri_without_an_account_is_refused(#[case] given: &str) {
        let client = Client::new(
            crate::config::Session {
                service: "http://127.0.0.1:9".into(),
                did: "did:plc:me".into(),
                handle: "me.test".into(),
                access_jwt: "a".into(),
                refresh_jwt: "r".into(),
            },
            None,
        );
        let e = post_uri(&client, given).unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
        assert!(list_uri(&client, given).is_err());
    }

    // --seen marks up to the newest notification listed, by the time it
    // is, not by how its text sorts: a time without milliseconds sorts
    // after one with them in the same second, and one with an offset
    // anywhere.
    #[test]
    fn seen_is_the_newest_notification_by_time() {
        let listed = |times: &[&str]| -> Vec<Value> {
            times.iter().map(|t| json!({"indexedAt": t})).collect()
        };
        assert_eq!(
            seen_at(&listed(&[
                "2026-09-22T00:30:00Z",
                "2026-09-22T00:30:00.500Z"
            ])),
            "2026-09-22T00:30:00.500Z"
        );
        assert_eq!(
            seen_at(&listed(&[
                "2026-09-22T09:00:00+09:00",
                "2026-09-22T00:59:59.999Z"
            ])),
            "2026-09-22T00:59:59.999Z"
        );
    }

    // A message that cannot be sent is refused before anything reaches the
    // server: getConvoForMembers starts a conversation when there is none,
    // so asking for it first left an empty conversation behind, and a usage
    // error came out as a network one when the server could not be reached.
    #[rstest]
    #[case("")]
    #[case("  \n")]
    #[case(&"a".repeat(1001))]
    #[case(&"👨‍👩‍👧‍👦".repeat(401))]
    fn a_message_that_cannot_be_sent_starts_no_conversation(#[case] body: &str) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = AccountStore::open(dir.path()).unwrap();
        let ctx = Ctx {
            dir: dir.path(),
            accounts: &accounts,
            session: Some(crate::config::Session {
                service: "http://127.0.0.1:9".into(),
                did: "did:plc:me".into(),
                handle: "me.test".into(),
                access_jwt: "a".into(),
                refresh_jwt: "r".into(),
            }),
            service: "http://127.0.0.1:9",
            json: false,
        };
        let mut out = Vec::new();
        let e = chat(
            &ctx,
            &mut out,
            Some("did:plc:alice"),
            Some(body.to_string()),
            20,
        )
        .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
        assert!(out.is_empty());
    }

    // A comment over createReport's limits (2000 characters, 20000 bytes)
    // is refused before anything is asked of the server, not after it was
    // sent and refused as a network error.
    #[rstest]
    #[case(&"a".repeat(2001))]
    #[case(&"👨‍👩‍👧‍👦".repeat(801))]
    fn a_report_comment_over_the_limit_is_refused_before_it_is_sent(#[case] comment: &str) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = AccountStore::open(dir.path()).unwrap();
        let ctx = Ctx {
            dir: dir.path(),
            accounts: &accounts,
            session: Some(crate::config::Session {
                service: "http://127.0.0.1:9".into(),
                did: "did:plc:me".into(),
                handle: "me.test".into(),
                access_jwt: "a".into(),
                refresh_jwt: "r".into(),
            }),
            service: "http://127.0.0.1:9",
            json: false,
        };
        let mut out = Vec::new();
        let e = report(
            &ctx,
            &mut out,
            "at://did:plc:alice/app.bsky.feed.post/p",
            ReportReason::Spam,
            Some(comment),
        )
        .unwrap_err();
        assert_eq!(e.kind(), Kind::Usage, "{}", e.message());
        assert!(e.message().contains("the comment is"), "{}", e.message());
        assert!(out.is_empty());
    }

    // A feed's name is whatever its maker wrote: an escape sequence in it
    // (one sets the clipboard, one clears the screen) must not reach the
    // terminal with the list of pinned feeds.
    #[test]
    fn the_pinned_feed_names_in_the_hint_are_text_only() {
        let pinned = [
            crate::api::types::FeedInfo {
                uri: "at://did:plc:x/app.bsky.feed.generator/a".into(),
                name: "Cats 🐱\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J".into(),
            },
            crate::api::types::FeedInfo {
                uri: "at://did:plc:x/app.bsky.feed.generator/b".into(),
                name: "日本\n語\u{9b}".into(),
            },
        ];
        let e = no_pinned_feed("dogs", &pinned);
        assert_eq!(e.hint(), Some("pinned: Cats 🐱]52;c;cHduZWQ=[2J, 日本 語"));
    }
}
