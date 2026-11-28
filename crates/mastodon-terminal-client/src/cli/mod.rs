//! The commands: what a Mastodon server shows to visitors, or to the
//! account `mstdn login` saved for it, from scripts and pipes. Every
//! command prints text for reading, or with `--json` one object per status
//! per line, in the same shape for every command (see
//! [`format::status_json`]). They are msky's commands, by the names msky
//! and bsky give them, where Mastodon has the same thing.

pub(crate) mod format;
mod post;

pub(crate) use post::{PostArgs, Visibility, counted};

use std::collections::HashSet;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::api::types::{Account, Notification, Status};
use crate::api::{self, Client, Next, Page, StatusFilter};
use crate::config::{AccountStore, Session};
use crate::error::{Error, Result};
use crate::i18n::{t, tf};
use crate::{browser, download, oauth};

/// The commands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// An account's posts, newest first.
    ///
    /// ACTOR is name, @name, @name@host, or the account's address
    /// (mastodon.social/@name). Pages are read until there are --limit
    /// posts or the account has no more.
    Author {
        actor: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Which of the account's posts.
        #[arg(long, value_enum, default_value_t = AuthorFilter::WithReplies)]
        filter: AuthorFilter,
        /// Keep the posts the account boosted too.
        #[arg(long)]
        reposts: bool,
        /// Save the files of the posts instead of printing them, in the
        /// folder -o names or the download folder, as `download` does.
        #[arg(long)]
        download: bool,
        /// The folder to save in instead of the download folder; giving it
        /// saves the files as --download does.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// One post: its ID or its address (mastodon.social/@name/ID).
    #[command(alias = "note")]
    Status { status: String },
    /// A post and its thread: what it answers, the post, and its replies.
    Thread { status: String },
    /// An account: name, counts and description; yours without ACTOR.
    Profile { actor: Option<String> },
    /// Posts that match QUERY, as the server finds them.
    ///
    /// A hashtag (#rust) lists the posts with it, newest first, as a
    /// visitor too. Other words are searched with the server's full-text
    /// search, which takes a login and finds only the posts the server
    /// lets it (on mastodon.social: your own, those you interacted with,
    /// and those of accounts that opted in).
    Search {
        query: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Save the files attached to a post in the download folder.
    ///
    /// STATUS is its ID or its address. The files of the post it boosts are
    /// saved for a boost, each named after the post it is attached to, its
    /// place there and its type (109-1.png); a name already taken gets a
    /// number, so nothing is overwritten. Each saved file is printed.
    Download {
        status: String,
        /// The folder to save in instead of the download folder.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// A timeline, newest first: home (the default), local or public.
    ///
    /// home is your account's and needs `mstdn login`; local (the server's
    /// accounts) and public (every post the server knows, Mastodon's
    /// federated timeline) are read as a visitor when there is no login,
    /// where the server shows them to visitors. Pages are read as `author`
    /// reads them.
    #[command(visible_alias = "tl")]
    Timeline {
        #[arg(value_enum, default_value_t = TimelineKind::Home)]
        kind: TimelineKind,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Keep the boosts too.
        #[arg(long)]
        reposts: bool,
    },
    /// Your notifications, newest first.
    #[command(visible_alias = "notif")]
    Notifications {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// The account logged in on the server.
    Whoami,
    /// Post a status; TEXT `-` reads it from stdin.
    ///
    /// The files are uploaded first, and the post waits until the server
    /// has processed them. The server makes the card of the first link
    /// itself. --dry-run shows what would be sent and sends nothing.
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
        #[arg(long, value_name = "STATUS")]
        reply: Option<String>,
        /// A content warning, shown in place of the text until it is opened.
        #[arg(long, value_name = "TEXT")]
        cw: Option<String>,
        /// Mark the pictures and the video sensitive, hidden until opened
        /// (a content warning marks them so too).
        #[arg(long)]
        sensitive: bool,
        /// Who may see the post.
        #[arg(long, value_enum, default_value_t = Visibility::Public)]
        visibility: Visibility,
        /// Print what would be sent, and send nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Log in with OAuth, and make the account the one in use.
    ///
    /// mstdn asks to read and write (`read write`). It registers itself
    /// on the server the first time, prints the server's page that asks for
    /// the access (and opens it in the browser when it can): open it,
    /// signed in there, and allow it. The browser is sent back to mstdn on
    /// this machine; with --oob the page shows a code to paste instead, for
    /// a browser on another machine. mstdn never sees the password. The
    /// token is saved for the server, readable by you alone; logging in on
    /// the same server again replaces it.
    Login {
        /// Paste the code the page shows, instead of the browser bringing
        /// it back to mstdn.
        #[arg(long)]
        oob: bool,
        /// How long to wait for the access to be allowed.
        #[arg(long, value_name = "SECONDS", default_value_t = oauth::DEFAULT_TIMEOUT.as_secs())]
        timeout: u64,
    },
    /// The accounts logged in, the one in use marked.
    ///
    /// --use makes the account on HOST the one in use: the commands read
    /// its server when --host does not name one.
    Accounts {
        /// The host of the account to use from now on.
        #[arg(long = "use", value_name = "HOST")]
        use_host: Option<String>,
    },
    /// Log the account in use (or the one on --host) out: its saved token
    /// is removed.
    Logout {
        /// Log every account out.
        #[arg(long)]
        all: bool,
    },
}

/// Which timeline `timeline` reads, by the names Mastodon gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum TimelineKind {
    /// The posts of the accounts you follow, and your own.
    Home,
    /// The posts of the server's accounts.
    Local,
    /// Every post the server knows (Mastodon's federated timeline).
    #[value(alias = "federated", alias = "global")]
    Public,
}

impl TimelineKind {
    /// The API path, and the query that says which part of it.
    pub(crate) fn path(self) -> (&'static str, &'static [(&'static str, &'static str)]) {
        match self {
            TimelineKind::Home => ("v1/timelines/home", &[]),
            TimelineKind::Local => ("v1/timelines/public", &[("local", "true")]),
            TimelineKind::Public => ("v1/timelines/public", &[]),
        }
    }

    /// Whether it is an account's: the server shows it to no visitor.
    pub(crate) fn needs_login(self) -> bool {
        self == TimelineKind::Home
    }
}

/// Which posts of an account `author` lists, by the names bsky's
/// `--filter` takes. Mastodon leaves out replies to others itself, and
/// takes files only; a reply to oneself and a video are told here, from
/// the posts it sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum AuthorFilter {
    /// Posts and replies.
    #[value(alias = "posts_with_replies")]
    WithReplies,
    /// Posts, not replies.
    #[value(alias = "posts_no_replies")]
    NoReplies,
    /// Posts with files attached (pictures, videos, any file).
    #[value(alias = "posts_with_media")]
    Media,
    /// Posts with a video attached.
    #[value(alias = "posts_with_video")]
    Video,
    /// Posts, and the account's replies to its own posts.
    #[value(alias = "posts_and_author_threads")]
    Threads,
}

impl AuthorFilter {
    /// What `accounts/:id/statuses` is asked for.
    fn asked(self, reposts: bool) -> StatusFilter {
        StatusFilter {
            // Mastodon's exclude_replies keeps the replies to oneself.
            exclude_replies: !matches!(self, AuthorFilter::WithReplies),
            exclude_reblogs: !reposts,
            only_media: matches!(self, AuthorFilter::Media | AuthorFilter::Video),
        }
    }

    /// Whether the server's answer is filtered again here, so a page brings
    /// fewer than it holds.
    fn narrows(self) -> bool {
        matches!(self, AuthorFilter::Video | AuthorFilter::NoReplies)
    }

    /// Whether `s`, a post of the account, is kept.
    fn keeps(self, s: &Status) -> bool {
        match self {
            AuthorFilter::Video => s
                .shown()
                .media_attachments
                .iter()
                .any(|m| matches!(m.kind.as_str(), "video" | "gifv")),
            AuthorFilter::NoReplies => s.in_reply_to_id.is_none(),
            _ => true,
        }
    }
}

/// What a command runs with.
pub(crate) struct Ctx {
    /// The server `--host` names, else that of the account in use.
    pub origin: String,
    pub json: bool,
    /// Whether `--host` (or `MSTDN_HOST`) named the server.
    pub host_given: bool,
    /// The saved logins, or why there is no config directory to keep them.
    pub accounts: Result<AccountStore>,
    /// The token `MSTDN_TOKEN` holds: used on `origin` in place of a saved
    /// login.
    pub env_token: Option<String>,
}

impl Ctx {
    /// The logins, for the commands that save or remove one.
    fn store(&self) -> Result<&AccountStore> {
        self.accounts.as_ref().map_err(Clone::clone)
    }

    /// The account saved for the server at `origin`. A token is only ever
    /// sent to the server that gave it, scheme and port included.
    pub(crate) fn session(&self, origin: &str) -> Result<Option<Session>> {
        if let Some(token) = &self.env_token
            && origin == self.origin
        {
            return Ok(Some(Session {
                service: origin.to_string(),
                user_id: String::new(),
                username: String::new(),
                token: token.clone(),
                scopes: Vec::new(),
            }));
        }
        let Ok(store) = &self.accounts else {
            return Ok(None);
        };
        Ok(store
            .find(api::authority(origin))?
            .filter(|s| s.service == origin))
    }

    /// A client of `origin`, or of the server `--host` names, as the account
    /// saved for it when there is one, else as a visitor.
    fn client(&self, origin: Option<&str>) -> Result<Client> {
        let origin = origin.unwrap_or(&self.origin);
        let token = self.session(origin)?.map(|s| s.token);
        Ok(Client::new(origin).with_token(token))
    }

    /// A client of the server `--host` names, as its account; none saved is
    /// a usage error, before anything is asked.
    fn account_client(&self) -> Result<Client> {
        let client = self.client(None)?;
        if client.logged_in() {
            Ok(client)
        } else {
            Err(not_logged_in(client.host()))
        }
    }

    /// A client of the server `--host` names, as its account, that may do
    /// `scope` (`write:statuses`...). A login saved with fewer scopes is a
    /// usage error before anything is sent. A token from `MSTDN_TOKEN` is
    /// taken as it is; the server says what it lacks.
    fn writer(&self, scopes: &[&str], what: &str) -> Result<Client> {
        let client = self.account_client()?;
        if self.env_token.is_some() {
            return Ok(client);
        }
        if let Some(s) = self.session(&self.origin)?
            && !s.scopes.is_empty()
        {
            let missing: Vec<&str> = scopes.iter().copied().filter(|p| !s.allows(p)).collect();
            if !missing.is_empty() {
                let host = client.host();
                return Err(Error::usage(format!(
                    "the login saved for {host} does not allow mstdn to {what}: it lacks {}",
                    missing.join(", ")
                ))
                .with_hint(format!(
                    "run `mstdn login --host {host}` again and allow the access"
                )));
            }
        }
        Ok(client)
    }

    /// `e` with what to do about `MSTDN_TOKEN` when it is the token the
    /// server turned down.
    fn explained(&self, e: Error) -> Error {
        if self.env_token.is_none() {
            return e;
        }
        let host = api::authority(&self.origin);
        if e.message().contains("access token is invalid")
            || e.hint()
                .is_some_and(|h| h.contains("no longer takes the token"))
        {
            e.with_hint(format!(
                "{host} does not take the token in MSTDN_TOKEN; check it, or make a new one in Preferences > Development"
            ))
        } else if e.message().contains("scope") {
            e.with_hint(format!(
                "the token in MSTDN_TOKEN was not given that scope; make one in Preferences > Development on {host} with read and write"
            ))
        } else {
            e
        }
    }
}

/// That no account is logged in on `host`, and how to log one in.
fn not_logged_in(host: &str) -> Error {
    Error::usage(format!("not logged in on {host}"))
        .with_hint(format!("run `mstdn login --host {host}`"))
}

/// Run `cmd`, writing to stdout.
pub(crate) fn run(cmd: Command, ctx: &Ctx) -> Result<()> {
    let mut out = io::stdout().lock();
    ended_by_reader(run_to(cmd, ctx, &mut out))
}

/// A command's result, where a reader that stopped reading (`| head`) is
/// not an error: what it wanted it has, so the command ends there, as the
/// tools it is piped into expect.
pub(crate) fn ended_by_reader(result: Result<()>) -> Result<()> {
    match result {
        Err(e) if e.message() == PIPE_CLOSED => Ok(()),
        r => r,
    }
}

/// What `write_err` says when the reader has closed the pipe.
const PIPE_CLOSED: &str = "the output was closed";

fn run_to(cmd: Command, ctx: &Ctx, out: &mut dyn Write) -> Result<()> {
    match cmd {
        Command::Author {
            actor,
            limit,
            filter,
            reposts,
            download,
            output,
        } => {
            let save_in = if download || output.is_some() {
                Some(download::download_dir(output)?)
            } else {
                None
            };
            author(ctx, out, &actor, limit, filter, reposts, save_in)
        }
        Command::Status { status } => show_status(ctx, out, &status),
        Command::Thread { status } => thread(ctx, out, &status),
        Command::Profile { actor } => profile(ctx, out, actor.as_deref()),
        Command::Search { query, limit } => search(ctx, out, &query, limit),
        Command::Download { status, output } => {
            download_status(ctx, out, &status, &download::download_dir(output)?)
        }
        Command::Timeline {
            kind,
            limit,
            reposts,
        } => timeline(ctx, out, kind, limit, reposts),
        Command::Notifications { limit } => notifications(ctx, out, limit),
        Command::Whoami => whoami(ctx, out),
        Command::Post {
            text,
            images,
            alts,
            video,
            reply,
            cw,
            sensitive,
            visibility,
            dry_run,
        } => post::post(
            ctx,
            out,
            &PostArgs {
                text,
                images,
                alts,
                video,
                reply,
                cw,
                sensitive,
                visibility,
                dry_run,
            },
            &mut io::stdin(),
            &mut post::WallClock,
        ),
        Command::Accounts { use_host } => accounts(ctx, out, use_host.as_deref()),
        Command::Login { oob, timeout } => {
            let mut err = io::stderr();
            // The browser is opened for a person at the terminal, or when
            // MSTDN_BROWSER names the program to open it with.
            let open = io::stderr().is_terminal() || browser::named().is_some();
            login(
                ctx,
                out,
                &mut err,
                &mut io::stdin().lock(),
                Duration::from_secs(timeout),
                oob,
                open,
            )
        }
        Command::Logout { all } => logout(ctx, out, all),
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

/// What `pages` reads: an item with an ID, the one Mastodon pages by.
trait Paged {
    fn id(&self) -> &str;
}

impl Paged for Status {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Paged for Notification {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Pages from `fetch` (how many to ask for, and the ID to list older items
/// than) until `limit` items that `keep` keeps have come or there are no
/// more. The next page starts where the `Link` header says, else after the
/// last item. An item a later page repeats is kept once, and a page that
/// brings nothing new, starts from an ID seen before, or is the last the
/// `Link` header names ends the reading: a server that ignores `max_id`
/// would otherwise be asked for the same page for good. A page may hold
/// fewer than were asked for while there are more (what the visitor may
/// not see is left out), so only an empty page or the header says the end
/// was reached. When `keep` leaves items out, every page is a full one.
fn pages<T: Paged>(
    limit: usize,
    full_pages: bool,
    keep: impl Fn(&T) -> bool,
    mut fetch: impl FnMut(usize, Option<&str>) -> Result<Page<T>>,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut cursors = HashSet::new();
    let mut max_id: Option<String> = None;
    while items.len() < limit {
        let want = if full_pages {
            api::MAX_PAGE
        } else {
            (limit - items.len()).min(api::MAX_PAGE)
        };
        let page = fetch(want, max_id.as_deref())?;
        let Some(last) = page.items.last().map(|n| n.id().to_string()) else {
            break;
        };
        let mut new = false;
        for n in page.items {
            if seen.insert(n.id().to_string()) {
                new = true;
                if keep(&n) && items.len() < limit {
                    items.push(n);
                }
            }
        }
        let next = match page.next {
            Next::End => break,
            Next::MaxId(id) => id,
            Next::Unsaid => last,
        };
        if !new || !cursors.insert(next.clone()) {
            break;
        }
        max_id = Some(next);
    }
    Ok(items)
}

/// An account as typed: on which server to ask, its name, and the server it
/// is on when that was written.
#[derive(Debug, PartialEq, Eq)]
struct UserRef {
    /// The server of an address; `None` asks the one `--host` names.
    origin: Option<String>,
    username: String,
    host: Option<String>,
}

/// `name`, `@name`, `@name@host`, `name@host`, or an account's address,
/// `https://<server>/@name`, `https://<server>/@name@host` or
/// `https://<server>/users/name`.
fn parse_user(typed: &str) -> Result<UserRef> {
    let t = typed.trim();
    let not_a_user = || {
        Error::usage(format!("{t:?} is not an account")).with_hint(
            "give name, @name, @name@host, or its address such as https://mastodon.social/@name",
        )
    };
    let (origin, acct) = match web_address(t) {
        Some((origin, path)) => {
            let acct = path
                .strip_prefix('@')
                .or_else(|| path.strip_prefix("users/"))
                .ok_or_else(not_a_user)?;
            if acct.contains('/') {
                return Err(not_a_user());
            }
            (Some(origin), acct.to_string())
        }
        None => (None, t.strip_prefix('@').unwrap_or(t).to_string()),
    };
    let (username, host) = match acct.split_once('@') {
        Some((name, host)) => (name, Some(host)),
        None => (acct.as_str(), None),
    };
    let name_ok = !username.is_empty()
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !name_ok {
        return Err(not_a_user());
    }
    // The host after the name is a host alone: a scheme or a path there
    // is a typo, not a server to ask.
    let host = match host {
        Some(h) => match api::normalize_host(h) {
            Ok(origin) if !h.contains("://") && !h.starts_with('@') => {
                Some(api::authority(&origin).to_string())
            }
            _ => return Err(not_a_user()),
        },
        None => None,
    };
    Ok(UserRef {
        origin,
        username: username.to_string(),
        host,
    })
}

/// The origin and the path of an http(s) address, the path without its
/// query, fragment or trailing slash. One written without its scheme
/// (`mastodon.social/@name/ID`) is taken as https.
fn web_address(t: &str) -> Option<(String, String)> {
    let lower = t.to_ascii_lowercase();
    let (scheme, rest) = if lower.starts_with("https://") || lower.starts_with("http://") {
        t.split_once("://")?
    } else if !t.contains("://") && t.find('/').is_some_and(|i| i > 0) {
        ("https", t)
    } else {
        return None;
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let origin = api::normalize_host(&format!("{scheme}://{authority}")).ok()?;
    let path = path.trim_start_matches('/').trim_end_matches('/');
    Some((origin, path.to_string()))
}

/// A status as typed: on which server to ask, and its ID there. An address
/// is `https://<server>/@name/ID`, `https://<server>/@name@host/ID` (a
/// status of another server, as the server shows it) or
/// `https://<server>/users/name/statuses/ID`.
fn parse_status(typed: &str) -> Result<(Option<String>, String)> {
    let t = typed.trim();
    let not_a_status = || {
        Error::usage(format!("{t:?} is not a post")).with_hint(
            "give its ID, or its address such as https://mastodon.social/@name/109372...",
        )
    };
    let (origin, id) = match web_address(t) {
        Some((origin, path)) => {
            let parts: Vec<&str> = path.split('/').collect();
            let id = match parts.as_slice() {
                [who, id] if who.starts_with('@') && who.len() > 1 => *id,
                ["users", _, "statuses", id] => *id,
                _ => return Err(not_a_status()),
            };
            (Some(origin), id.to_string())
        }
        None => (None, t.to_string()),
    };
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(not_a_status());
    }
    Ok((origin, id))
}

/// The account `who` names, asked of `client`'s server: by `lookup`, and,
/// for an account of another server it does not know yet, by a search that
/// has it fetch the account, which takes a login.
fn find_account(client: &Client, who: &UserRef) -> Result<Account> {
    // An account of the server asked is named without a host.
    let remote = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let acct = match remote {
        Some(h) => format!("{}@{h}", who.username),
        None => who.username.clone(),
    };
    match client.lookup(&acct) {
        Ok(a) => Ok(a),
        Err(e) if remote.is_some() && e.message().contains("failed: Record not found") => {
            let host = client.host();
            if !client.logged_in() {
                return Err(e.with_hint(format!(
                    "{host} does not know @{acct} yet; log in (`mstdn login --host {host}`) to have it fetched, or read it on its own server with --host {}",
                    remote.unwrap_or_default()
                )));
            }
            client.resolve_account(&acct)?.ok_or_else(|| {
                Error::api(format!("{host} cannot find @{acct}"))
                    .with_hint("check the name and its server")
            })
        }
        Err(e) => Err(e),
    }
}

fn author(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: &str,
    limit: usize,
    filter: AuthorFilter,
    reposts: bool,
    save_in: Option<PathBuf>,
) -> Result<()> {
    let who = parse_user(actor)?;
    let client = ctx.client(who.origin.as_deref())?;
    let statuses = if limit == 0 {
        Vec::new()
    } else {
        let account = find_account(&client, &who)?;
        let asked = filter.asked(reposts);
        pages(
            limit,
            filter.narrows(),
            |s| filter.keeps(s),
            |n, max_id| client.account_statuses(&account.id, &asked, n, max_id),
        )?
    };
    if let Some(dir) = save_in {
        let agent = api::agent();
        for s in &statuses {
            save_files(ctx, out, &agent, client.origin(), s, &dir)?;
        }
        return Ok(());
    }
    print_statuses(ctx, out, client.origin(), &statuses)
}

/// Print statuses: one JSON line each, or text with a blank line between.
fn print_statuses(ctx: &Ctx, out: &mut dyn Write, origin: &str, statuses: &[Status]) -> Result<()> {
    for (i, s) in statuses.iter().enumerate() {
        if ctx.json {
            json_line(out, &format::status_json(s, origin))?;
            continue;
        }
        if i > 0 {
            text(out, "\n")?;
        }
        text(out, &format::status(s, origin))?;
    }
    Ok(())
}

/// What a file is, as `download --json` says it: `image`, `video`,
/// `audio`, or `file`; a GIF Mastodon made a video is a video.
fn kind(m: &str) -> &'static str {
    match m {
        "image" => "image",
        "video" | "gifv" => "video",
        "audio" => "audio",
        _ => "file",
    }
}

/// Save every file `s` carries (that of the status it boosts, for a boost)
/// in `dir`, printing each as it is written.
fn save_files(
    ctx: &Ctx,
    out: &mut dyn Write,
    agent: &ureq::Agent,
    origin: &str,
    s: &Status,
    dir: &std::path::Path,
) -> Result<()> {
    for (owner, i, m) in s.attached() {
        let path = download::save(agent, m, dir, &download::file_stem(&owner.id, i))?;
        let shown = path.to_string_lossy();
        if ctx.json {
            json_line(
                out,
                &json!({
                    "status": format::status_url(origin, owner),
                    "kind": kind(&m.kind),
                    "url": m.source(),
                    "path": shown,
                }),
            )?;
        } else {
            text(out, &format!("{shown}\n"))?;
        }
    }
    Ok(())
}

fn show_status(ctx: &Ctx, out: &mut dyn Write, typed: &str) -> Result<()> {
    let (origin, id) = parse_status(typed)?;
    let client = ctx.client(origin.as_deref())?;
    let s = client.status(&id)?;
    print_statuses(ctx, out, client.origin(), std::slice::from_ref(&s))
}

/// `mstdn thread`: what the status answers, oldest first, the status, and
/// the replies under it, in the order the server sends them.
fn thread(ctx: &Ctx, out: &mut dyn Write, typed: &str) -> Result<()> {
    let (origin, id) = parse_status(typed)?;
    let client = ctx.client(origin.as_deref())?;
    let s = client.status(&id)?;
    let c = client.context(&id)?;
    let all: Vec<Status> = c
        .ancestors
        .into_iter()
        .chain(std::iter::once(s))
        .chain(c.descendants)
        .collect();
    print_statuses(ctx, out, client.origin(), &all)
}

fn profile(ctx: &Ctx, out: &mut dyn Write, actor: Option<&str>) -> Result<()> {
    let (client, a) = match actor {
        Some(typed) => {
            let who = parse_user(typed)?;
            let client = ctx.client(who.origin.as_deref())?;
            let a = find_account(&client, &who)?;
            (client, a)
        }
        None => {
            let client = ctx.account_client()?;
            let a = client.me().map_err(|e| ctx.explained(e))?;
            (client, a)
        }
    };
    if ctx.json {
        return json_line(out, &format::account_json(&a, client.origin()));
    }
    text(out, &format::account(&a, client.origin()))
}

/// The hashtag `query` is, without its `#`: `#rust`, a word of letters,
/// digits and `_`.
fn hashtag(query: &str) -> Option<&str> {
    let tag = query.trim().strip_prefix('#')?;
    (!tag.is_empty() && tag.chars().all(|c| c.is_alphanumeric() || c == '_')).then_some(tag)
}

fn search(ctx: &Ctx, out: &mut dyn Write, query: &str, limit: usize) -> Result<()> {
    if query.trim().is_empty() {
        return Err(Error::usage("the search query is empty"));
    }
    let client = ctx.client(None)?;
    if let Some(tag) = hashtag(query) {
        let path = format!("v1/timelines/tag/{}", api::percent_encode(tag));
        let statuses = pages(
            limit,
            false,
            |_| true,
            |n, max_id| client.timeline(&path, &[], n, max_id),
        )?;
        return print_statuses(ctx, out, client.origin(), &statuses);
    }
    // Mastodon answers a visitor's full-text search with no posts at all,
    // whatever matches: that is said before anything is asked.
    if !client.logged_in() {
        let host = client.host();
        return Err(Error::usage(format!(
            "{host} searches the text of posts only for an account that is logged in"
        ))
        .with_hint(format!(
            "run `mstdn login --host {host}`, or search a hashtag such as '#rust', which visitors may"
        )));
    }
    // Search pages by offset, not by ID.
    let mut statuses: Vec<Status> = Vec::new();
    let mut seen = HashSet::new();
    while statuses.len() < limit {
        let want = (limit - statuses.len()).min(api::MAX_PAGE);
        let found = client
            .search(query, "statuses", false, want, seen.len())
            .map_err(|e| ctx.explained(e))?;
        if found.statuses.is_empty() {
            break;
        }
        let mut new = false;
        for s in found.statuses {
            if seen.insert(s.id.clone()) {
                new = true;
                if statuses.len() < limit {
                    statuses.push(s);
                }
            }
        }
        if !new {
            break;
        }
    }
    print_statuses(ctx, out, client.origin(), &statuses)
}

fn download_status(
    ctx: &Ctx,
    out: &mut dyn Write,
    typed: &str,
    dir: &std::path::Path,
) -> Result<()> {
    let (origin, id) = parse_status(typed)?;
    let client = ctx.client(origin.as_deref())?;
    let s = client.status(&id)?;
    if s.attached().is_empty() {
        return Err(Error::usage(format!(
            "{} has no files",
            format::status_url(client.origin(), s.shown())
        )));
    }
    save_files(ctx, out, &api::agent(), client.origin(), &s, dir)
}

fn timeline(
    ctx: &Ctx,
    out: &mut dyn Write,
    kind: TimelineKind,
    limit: usize,
    reposts: bool,
) -> Result<()> {
    let client = if kind.needs_login() {
        ctx.account_client()?
    } else {
        ctx.client(None)?
    };
    let (path, extra) = kind.path();
    let statuses = pages(
        limit,
        !reposts,
        |s: &Status| reposts || s.reblog.is_none(),
        |n, max_id| client.timeline(path, extra, n, max_id),
    )
    .map_err(|e| ctx.explained(e))?;
    print_statuses(ctx, out, client.origin(), &statuses)
}

fn notifications(ctx: &Ctx, out: &mut dyn Write, limit: usize) -> Result<()> {
    let client = ctx.account_client()?;
    let all = pages(
        limit,
        false,
        |_| true,
        |n, max_id| client.notifications(n, max_id),
    )
    .map_err(|e| ctx.explained(e))?;
    for (i, n) in all.iter().enumerate() {
        if ctx.json {
            json_line(out, &format::notification_json(n, client.origin()))?;
            continue;
        }
        if i > 0 {
            text(out, "\n")?;
        }
        text(out, &format::notification(n, client.origin()))?;
    }
    Ok(())
}

/// `mstdn accounts`: the accounts logged in, by host, the one in use
/// marked; with `use_host`, that one made the one in use first.
fn accounts(ctx: &Ctx, out: &mut dyn Write, use_host: Option<&str>) -> Result<()> {
    let store = ctx.store()?;
    if let Some(typed) = use_host {
        let host = api::authority(&api::normalize_host(typed)?).to_string();
        if store.find(&host)?.is_none() {
            let known: Vec<String> = store.list()?.iter().map(|s| s.host().to_string()).collect();
            let hint = if known.is_empty() {
                "no account is logged in; run `mstdn login --host HOST`".to_string()
            } else {
                format!("logged in on: {}", known.join(", "))
            };
            return Err(Error::usage(format!("no account is logged in on {host}")).with_hint(hint));
        }
        store.set_current(&host)?;
    }
    let current = store.current()?.map(|s| s.service);
    let all = store.list()?;
    if all.is_empty() && !ctx.json {
        return text(out, "not logged in\n");
    }
    for s in all {
        let used = current.as_deref() == Some(s.service.as_str());
        if ctx.json {
            json_line(
                out,
                &json!({
                    "id": s.user_id,
                    "username": s.username,
                    "host": s.host(),
                    "service": s.service,
                    "current": used,
                }),
            )?;
        } else {
            let mark = if used { "* " } else { "  " };
            text(
                out,
                &format!(
                    "{mark}@{}@{}  {}\n",
                    crate::text::one_line(&s.username),
                    s.host(),
                    crate::text::one_line(&s.user_id)
                ),
            )?;
        }
    }
    Ok(())
}

fn whoami(ctx: &Ctx, out: &mut dyn Write) -> Result<()> {
    profile(ctx, out, None)
}

/// Say `s` on `err`, where it does not mix with the output. One that cannot
/// be shown there is no reason to stop: the address is what matters, and a
/// login can be allowed from another device all the same.
fn say(err: &mut dyn Write, s: &str) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "stderr is the only place for what the user is to do, and the login goes on whether it showed or not"
    )]
    let _: io::Result<()> = err.write_all(s.as_bytes()).and_then(|()| err.flush());
}

/// `mstdn login`: register on the server when mstdn has not yet, ask for
/// the access on its page, take the code back (from the browser, or pasted
/// with `oob`), trade it for the token, and save the token with the account
/// it is of.
fn login(
    ctx: &Ctx,
    out: &mut dyn Write,
    err: &mut dyn Write,
    input: &mut dyn BufRead,
    timeout: Duration,
    oob: bool,
    open_browser: bool,
) -> Result<()> {
    let store = ctx.store()?;
    // The app and the code are asked for as a visitor: an old token is not
    // sent along.
    let client = Client::new(&ctx.origin);
    let host = client.host().to_string();
    let (app, redirect) = oauth::app_for(store, &client, oob)?;
    let redirect_uri = redirect.uri()?;
    let state = api::random_hex("")?;
    let pkce = oauth::pkce()?;
    let url = oauth::authorize_url(
        client.origin(),
        &app.client_id,
        &redirect_uri,
        &state,
        &pkce.challenge,
    );
    say(
        err,
        &format!(
            "{}\n{url}\n",
            tf(
                "To log in on {}, open this address, signed in there, and allow mstdn to read and write:",
                &[&host]
            )
        ),
    );
    if open_browser && browser::open(&url) {
        say(err, &format!("{}\n", t("(opened in the web browser)")));
    }
    let code = match &redirect {
        oauth::Redirect::Loopback(listener) => {
            say(
                err,
                &format!(
                    "{}\n",
                    tf(
                        "Waiting for the browser to come back (up to {} s; Ctrl-C to give up)...",
                        &[&timeout.as_secs().to_string()]
                    )
                ),
            );
            oauth::wait_for_code(listener, &state, &host, timeout)?
        }
        oauth::Redirect::Oob => {
            say(err, t("Paste the code the page shows, then press Enter: "));
            oauth::read_code(input)?
        }
    };
    let token = oauth::exchange(store, &client, &app, &code, &redirect_uri, &pkce.verifier)?;
    let me = Client::new(client.origin())
        .with_token(Some(token.access_token.clone()))
        .me()?;
    let scopes: Vec<String> = if token.scope.trim().is_empty() {
        oauth::SCOPES.split(' ').map(str::to_string).collect()
    } else {
        token.scope.split_whitespace().map(str::to_string).collect()
    };
    let saved = Session {
        service: client.origin().to_string(),
        user_id: me.id,
        username: me.username,
        token: token.access_token,
        scopes,
    };
    store.save(&saved)?;
    if ctx.json {
        return json_line(
            out,
            &json!({"id": saved.user_id, "username": saved.username, "host": host}),
        );
    }
    text(
        out,
        &format!(
            "logged in as @{}@{host}\n",
            crate::text::one_line(&saved.username)
        ),
    )
}

/// `mstdn logout`: the account in use (or the one on `--host`), or all of
/// them. Only the saved token goes; the access stays listed on the server
/// (Preferences, Account, Authorized apps), where it can be revoked.
fn logout(ctx: &Ctx, out: &mut dyn Write, all: bool) -> Result<()> {
    let store = ctx.store()?;
    // (account, host) of each one logged out; an account whose file could
    // not be read is named by its host alone.
    let gone: Vec<(Option<Session>, String)> = if all {
        store.remove_all()?
    } else {
        let host = match store.current_host() {
            Some(h) if !ctx.host_given => h,
            _ => api::authority(&ctx.origin).to_string(),
        };
        let was = store.find(&host).ok().flatten();
        if store.remove(&host)? {
            vec![(was, host)]
        } else {
            Vec::new()
        }
    };
    if ctx.json {
        let list: Vec<Value> = gone
            .iter()
            .map(|(s, host)| {
                json!({
                    "id": s.as_ref().map(|s| &s.user_id),
                    "username": s.as_ref().map(|s| &s.username),
                    "host": host,
                })
            })
            .collect();
        return json_line(out, &json!({ "loggedOut": list }));
    }
    if gone.is_empty() {
        return text(out, "not logged in\n");
    }
    for (s, host) in &gone {
        let line = match s {
            Some(s) => format!(
                "logged out @{}@{host}\n",
                crate::text::one_line(&s.username)
            ),
            None => format!("logged out of {host}\n"),
        };
        text(out, &line)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
