//! The commands: what a Misskey server shows to visitors, or to the account
//! `msky login` saved for it, from scripts and pipes. Every command prints
//! text for reading, or with `--json` one object per note per line, in the
//! same shape for every command (see [`format::note_json`]).

mod format;
mod more;
mod post;

pub use post::{PostArgs, Visibility};

use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::api::types::{Note, Notification};
use crate::api::{self, Client, NoteFilter};
use crate::config::{AccountStore, Session};
use crate::error::{Error, Result};
use crate::miauth::{self, Clock};
use crate::{browser, download};

/// The commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// An account's notes, newest first.
    ///
    /// ACTOR is name, @name, @name@host, or the account's address
    /// (misskey.io/@name). Pages are read until there are --limit notes or
    /// the account has no more.
    Author {
        actor: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Which of the account's notes.
        #[arg(long, value_enum, default_value_t = AuthorFilter::WithReplies)]
        filter: AuthorFilter,
        /// Keep the notes the account renoted too.
        #[arg(long)]
        reposts: bool,
        /// Save the files of the notes instead of printing them, in the
        /// folder -o names or the download folder, as `download` does.
        #[arg(long)]
        download: bool,
        /// The folder to save in instead of the download folder; giving it
        /// saves the files as --download does.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// One note: its ID or its address (misskey.io/notes/ID).
    Note { note: String },
    /// A note and its thread: what it answers, the note, and its replies.
    Thread { note: String },
    /// An account: name, counts and description; yours without ACTOR.
    Profile { actor: Option<String> },
    /// Who reacted to a note, and with what, newest first (bsky's `likes`).
    #[command(alias = "likes")]
    Reactions {
        note: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// The renotes of a note, newest first (bsky's `reposts`).
    #[command(alias = "reposts")]
    Renotes {
        note: String,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
    },
    /// React to a note: ❤ (Misskey's like), or --reaction; one given
    /// before is replaced.
    #[command(alias = "react")]
    Like {
        note: String,
        /// The reaction: a Unicode emoji, or :name: for a custom one.
        #[arg(long)]
        reaction: Option<String>,
    },
    /// Take back your reaction to a note.
    #[command(alias = "unreact")]
    Unlike { note: String },
    /// Renote a note, as bsky's repost.
    #[command(alias = "renote")]
    Repost { note: String },
    /// Take back your renotes of a note.
    #[command(alias = "unrenote")]
    Unrepost { note: String },
    /// Delete one of your own notes.
    Delete { note: String },
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
    /// Chat: the conversations; with ACTOR the messages with them; with
    /// TEXT too, send it (`-` reads it from stdin).
    Chat {
        actor: Option<String>,
        text: Option<String>,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Notes that match QUERY, newest first.
    Search {
        query: String,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Save the files attached to a note in the download folder.
    ///
    /// NOTE is its ID or its address. The files of the note it renotes or
    /// quotes are saved too, each named after the note it is attached to,
    /// its place there and its type (9abc-1.png); a name already taken gets
    /// a number, so nothing is overwritten. Each saved file is printed.
    Download {
        note: String,
        /// The folder to save in instead of the download folder.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// A timeline, newest first: home (the default), local, social or
    /// global.
    ///
    /// home and social are your account's and need `msky login`; local and
    /// global are read as a visitor when there is no login, where the
    /// server shows them to visitors. Pages are read as `author` reads them.
    Timeline {
        #[arg(value_enum, default_value_t = TimelineKind::Home)]
        kind: TimelineKind,
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Keep the renotes too.
        #[arg(long)]
        reposts: bool,
    },
    /// Your notifications, newest first. They are left unread.
    Notifications {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// The account logged in on the server.
    Whoami,
    /// Post a note; TEXT `-` reads it from stdin.
    ///
    /// The files are uploaded to your drive and attached. Misskey makes no
    /// card: each viewer's server shows a preview of the addresses in the
    /// text. --dry-run shows what would be sent and that preview, and sends
    /// nothing.
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
        /// The note this one replies to.
        #[arg(long, value_name = "NOTE")]
        reply: Option<String>,
        /// The note this one quotes (a quote renote).
        #[arg(long, value_name = "NOTE")]
        quote: Option<String>,
        /// Show no preview under the links: they are written as silent
        /// links, `?[address](address)`, which Misskey shows no preview of.
        #[arg(long)]
        no_card: bool,
        /// A content warning, shown in place of the text until it is opened.
        #[arg(long, value_name = "TEXT")]
        cw: Option<String>,
        /// Who may see the note.
        #[arg(long, value_enum, default_value_t = Visibility::Public)]
        visibility: Visibility,
        /// Print what would be sent and the server's preview of the first
        /// link, and send nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// The accounts that follow ACTOR (you by default).
    ///
    /// With --pending, the accounts that asked to follow you and wait for
    /// your approval.
    Followers {
        actor: Option<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        /// The requests to follow you not yet approved.
        #[arg(long)]
        pending: bool,
    },
    /// The accounts ACTOR follows (you by default).
    ///
    /// An account of another server is listed once its server accepts the
    /// follow; with --pending, the accounts you asked to follow that have
    /// not accepted yet (one that approves its followers, or a server that
    /// has not answered).
    Follows {
        actor: Option<String>,
        #[arg(short = 'n', long, default_value_t = 50)]
        limit: usize,
        /// Your requests to follow not yet accepted.
        #[arg(long)]
        pending: bool,
    },
    /// Follow an account: name, @name, @name@host, or its address.
    Follow { actor: String },
    /// Unfollow an account, or take back a request to follow it.
    Unfollow { actor: String },
    /// Log in with MiAuth, and make the account the one in use.
    ///
    /// msky asks to read your account, timelines and notifications, to
    /// post notes and upload their files, and to follow and unfollow.
    ///
    /// msky prints an address on the server (and opens it in the browser
    /// when it can): open it, signed in there, and allow the access. msky
    /// never sees the password. The token is saved for the server, readable
    /// by you alone; logging in on the same server again replaces it.
    ///
    /// Without --host, in a terminal, msky lists servers (`msky servers`)
    /// and asks which one; elsewhere it logs in on misskey.io, or on the
    /// server of the account in use.
    Login {
        /// How long to wait for the access to be allowed.
        #[arg(long, value_name = "SECONDS", default_value_t = miauth::DEFAULT_TIMEOUT.as_secs())]
        timeout: u64,
    },
    /// Servers to log in to, the most accounts first: Misskey Hub's list
    /// of servers that run Misskey, or, when it cannot be read, a short
    /// one msky carries.
    Servers {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// The accounts logged in, the one in use marked.
    ///
    /// --use makes the account on HOST the one in use: the commands and
    /// the client read its server when --host does not name one.
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

/// Which timeline `timeline` reads, by the names Misskey's tabs have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TimelineKind {
    /// The notes of the accounts you follow, and your own.
    Home,
    /// The notes of the server's accounts.
    Local,
    /// home and local together.
    #[value(alias = "hybrid")]
    Social,
    /// Every note the server knows.
    Global,
}

impl TimelineKind {
    /// Every timeline, in the order of Misskey's tabs, which is the order
    /// the client steps through them.
    pub const ALL: [TimelineKind; 4] = [
        TimelineKind::Home,
        TimelineKind::Local,
        TimelineKind::Social,
        TimelineKind::Global,
    ];

    /// Its name, as the command line takes it.
    pub fn name(self) -> &'static str {
        match self {
            TimelineKind::Home => "home",
            TimelineKind::Local => "local",
            TimelineKind::Social => "social",
            TimelineKind::Global => "global",
        }
    }

    /// Where it is in [`TimelineKind::ALL`].
    pub fn index(self) -> usize {
        match self {
            TimelineKind::Home => 0,
            TimelineKind::Local => 1,
            TimelineKind::Social => 2,
            TimelineKind::Global => 3,
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            TimelineKind::Home => "notes/timeline",
            TimelineKind::Local => "notes/local-timeline",
            TimelineKind::Social => "notes/hybrid-timeline",
            TimelineKind::Global => "notes/global-timeline",
        }
    }

    /// Whether it is an account's: the server shows it to no visitor.
    pub fn needs_login(self) -> bool {
        matches!(self, TimelineKind::Home | TimelineKind::Social)
    }
}

/// Which notes of an account `author` lists, by the names bsky's `--filter`
/// takes. Misskey's `users/notes` filters by replies and by files; a video
/// and an account's own threads are told here, from the notes it sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AuthorFilter {
    /// Notes and replies.
    #[value(alias = "posts_with_replies")]
    WithReplies,
    /// Notes, not replies.
    #[value(alias = "posts_no_replies")]
    NoReplies,
    /// Notes with files attached (pictures, videos, any file).
    #[value(alias = "posts_with_media")]
    Media,
    /// Notes with a video attached.
    #[value(alias = "posts_with_video")]
    Video,
    /// Notes, and the account's replies to its own notes.
    #[value(alias = "posts_and_author_threads")]
    Threads,
}

impl AuthorFilter {
    /// What `users/notes` is asked for.
    fn asked(self, reposts: bool) -> NoteFilter {
        let (replies, files) = match self {
            AuthorFilter::WithReplies | AuthorFilter::Threads => (true, false),
            AuthorFilter::NoReplies => (false, false),
            // Misskey takes no replies with files only.
            AuthorFilter::Media | AuthorFilter::Video => (false, true),
        };
        NoteFilter {
            replies,
            renotes: reposts,
            files,
        }
    }

    /// Whether the server's answer is filtered again here, so a page brings
    /// fewer than it holds.
    fn narrows(self) -> bool {
        matches!(self, AuthorFilter::Video | AuthorFilter::Threads)
    }

    /// Whether `n`, a note of the account `account`, is kept.
    fn keeps(self, n: &Note, account: &str) -> bool {
        match self {
            AuthorFilter::Video => n
                .attached()
                .iter()
                .any(|(_, _, f)| f.mime.starts_with("video/")),
            // A reply is kept when it answers the account itself; one whose
            // note the server did not send along cannot be told, and is not.
            AuthorFilter::Threads => {
                n.reply_id.is_none() || n.reply.as_ref().is_some_and(|r| r.author_id() == account)
            }
            _ => true,
        }
    }
}

/// What a command runs with.
pub struct Ctx {
    /// The server `--host` names, else that of the account in use.
    pub origin: String,
    pub json: bool,
    /// Whether `--host` (or `MSKY_HOST`) named the server.
    pub host_given: bool,
    /// The saved logins, or why there is no config directory to keep them.
    pub accounts: Result<AccountStore>,
    /// The token `MSKY_TOKEN` holds: used on `origin` in place of a saved
    /// login.
    pub env_token: Option<String>,
}

impl Ctx {
    /// The account the client opens as: the one saved for the server
    /// `--host` names (or the token in `MSKY_TOKEN`), as the commands use.
    pub fn account_in_use(&self) -> Result<Option<Session>> {
        self.session(&self.origin)
    }

    /// The logins, for the commands that save or remove one.
    fn store(&self) -> Result<&AccountStore> {
        self.accounts.as_ref().map_err(Clone::clone)
    }

    /// The account saved for the server at `origin`. A token is only ever
    /// sent to the server that gave it, scheme and port included.
    fn session(&self, origin: &str) -> Result<Option<Session>> {
        if let Some(token) = &self.env_token
            && origin == self.origin
        {
            return Ok(Some(Session {
                service: origin.to_string(),
                user_id: String::new(),
                username: String::new(),
                token: token.clone(),
                permissions: Vec::new(),
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
}

impl Ctx {
    /// A client of the server `--host` names, as its account, that may do
    /// what `needs` names (`write:notes`, ...), for `what` the command
    /// does. A login saved with fewer permissions (one made before msky
    /// could write) is a usage error before anything is sent. A token from
    /// `MSKY_TOKEN` is taken as it is; the server says what it lacks.
    fn writer(&self, needs: &[&str], what: &str) -> Result<Client> {
        let client = self.account_client()?;
        if self.env_token.is_some() {
            return Ok(client);
        }
        if let Some(s) = self.session(&self.origin)? {
            let missing: Vec<&str> = needs
                .iter()
                .copied()
                .filter(|p| !s.permissions.iter().any(|q| q == p))
                .collect();
            if !missing.is_empty() {
                let host = client.host();
                return Err(Error::usage(format!(
                    "the login saved for {host} does not allow msky to {what}: it lacks {}",
                    missing.join(", ")
                ))
                .with_hint(format!(
                    "it was made when msky could only read; run `msky login --host {host}` again and allow the access"
                )));
            }
        }
        Ok(client)
    }

    /// `e` with what to do about `MSKY_TOKEN` when it is the token the
    /// server turned down.
    fn explained(&self, e: Error) -> Error {
        if self.env_token.is_none() {
            return e;
        }
        let host = api::authority(&self.origin);
        if e.message().contains("AUTHENTICATION_FAILED") {
            e.with_hint(format!(
                "{host} does not take the token in MSKY_TOKEN; check it, or make a new one in Settings > API"
            ))
        } else if e.message().contains("PERMISSION_DENIED") {
            e.with_hint(format!(
                "the token in MSKY_TOKEN was not given that permission; make one in Settings > API on {host} with read:account, write:notes, write:drive and write:following"
            ))
        } else {
            e
        }
    }
}

/// That no account is logged in on `host`, and how to log one in.
fn not_logged_in(host: &str) -> Error {
    Error::usage(format!("not logged in on {host}"))
        .with_hint(format!("run `msky login --host {host}`"))
}

/// Run `cmd`, writing to stdout.
pub fn run(cmd: Command, ctx: &Ctx) -> Result<()> {
    let mut out = io::stdout().lock();
    ended_by_reader(run_to(cmd, ctx, &mut out))
}

/// A command's result, where a reader that stopped reading (`| head`) is
/// not an error: what it wanted it has, so the command ends there, as the
/// tools it is piped into expect.
pub fn ended_by_reader(result: Result<()>) -> Result<()> {
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
        Command::Note { note } => show_note(ctx, out, &note),
        Command::Thread { note } => more::thread(ctx, out, &note),
        Command::Profile { actor } => more::profile(ctx, out, actor.as_deref()),
        Command::Reactions { note, limit } => more::reactions(ctx, out, &note, limit),
        Command::Renotes { note, limit } => more::renotes(ctx, out, &note, limit),
        Command::Like { note, reaction } => more::like(ctx, out, &note, reaction.as_deref()),
        Command::Unlike { note } => more::unlike(ctx, out, &note),
        Command::Repost { note } => more::repost(ctx, out, &note, true),
        Command::Unrepost { note } => more::repost(ctx, out, &note, false),
        Command::Delete { note } => more::delete(ctx, out, &note),
        Command::Mute { actor } => more::guard(ctx, out, &actor, more::Guard::Mute, true),
        Command::Unmute { actor } => more::guard(ctx, out, &actor, more::Guard::Mute, false),
        Command::Mutes { limit } => more::guarded(ctx, out, more::Guard::Mute, limit),
        Command::Block { actor } => more::guard(ctx, out, &actor, more::Guard::Block, true),
        Command::Unblock { actor } => more::guard(ctx, out, &actor, more::Guard::Block, false),
        Command::Blocks { limit } => more::guarded(ctx, out, more::Guard::Block, limit),
        Command::Chat { actor, text, limit } => {
            more::chat(ctx, out, actor.as_deref(), text, limit, &mut io::stdin())
        }
        Command::Search { query, limit } => search(ctx, out, &query, limit),
        Command::Download { note, output } => {
            download_note(ctx, out, &note, &download::download_dir(output)?)
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
            quote,
            no_card,
            cw,
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
                quote,
                no_card,
                cw,
                visibility,
                dry_run,
            },
            &mut io::stdin(),
        ),
        Command::Followers {
            actor,
            limit,
            pending,
        } => relations(ctx, out, actor.as_deref(), limit, pending, Side::Followers),
        Command::Follows {
            actor,
            limit,
            pending,
        } => relations(ctx, out, actor.as_deref(), limit, pending, Side::Follows),
        Command::Accounts { use_host } => accounts(ctx, out, use_host.as_deref()),
        Command::Servers { limit } => {
            let (list, failed) = crate::servers::list();
            if let Some(e) = failed {
                say(
                    &mut io::stderr(),
                    &format!("{}; showing the servers msky knows\n", e.message()),
                );
            }
            servers(ctx, out, &list, limit)
        }
        Command::Follow { actor } => post::follow(ctx, out, &actor, true),
        Command::Unfollow { actor } => post::follow(ctx, out, &actor, false),
        Command::Login { timeout } => {
            let mut err = io::stderr();
            let origin =
                if !ctx.host_given && io::stdin().is_terminal() && io::stderr().is_terminal() {
                    let (list, failed) = crate::servers::list();
                    if let Some(e) = failed {
                        say(&mut err, &format!("{}\n", e.message()));
                    }
                    choose_server(&list, &mut io::stdin().lock(), &mut err)?
                } else {
                    ctx.origin.clone()
                };
            login(
                ctx,
                &origin,
                out,
                &mut err,
                Duration::from_secs(timeout),
                miauth::INTERVAL,
                &mut miauth::WallClock::start(),
                io::stderr().is_terminal(),
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

/// What `pages` reads: an item with an ID, older than which the next page is
/// asked for.
trait Paged {
    fn id(&self) -> &str;
}

impl Paged for Note {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Paged for crate::api::types::Relation {
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
/// more. A note a later page repeats is kept once, and a page that brings
/// nothing new or starts from an ID seen before ends the reading: a server
/// that ignores `untilId` would otherwise be asked for the same page for
/// good. Misskey may answer with fewer notes than asked for while there
/// are more, so only an empty page says the end was reached. When `keep`
/// leaves notes out, every page is a full one: a page of one could take as
/// many requests as the account has notes left out.
fn pages<T: Paged>(
    limit: usize,
    full_pages: bool,
    keep: impl Fn(&T) -> bool,
    mut fetch: impl FnMut(usize, Option<&str>) -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    let mut notes = Vec::new();
    let mut seen = HashSet::new();
    let mut untils = HashSet::new();
    let mut until: Option<String> = None;
    while notes.len() < limit {
        let want = if full_pages {
            api::MAX_PAGE
        } else {
            (limit - notes.len()).min(api::MAX_PAGE)
        };
        let page = fetch(want, until.as_deref())?;
        let Some(last) = page.last().map(|n| n.id().to_string()) else {
            break;
        };
        let mut new = false;
        for n in page {
            if seen.insert(n.id().to_string()) {
                new = true;
                if keep(&n) && notes.len() < limit {
                    notes.push(n);
                }
            }
        }
        if !new || !untils.insert(last.clone()) {
            break;
        }
        until = Some(last);
    }
    Ok(notes)
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
/// `https://<server>/@name` or `https://<server>/@name@host`.
fn parse_user(typed: &str) -> Result<UserRef> {
    let t = typed.trim();
    let not_an_account = || {
        Error::usage(format!("{t:?} is not an account")).with_hint(
            "give name, @name, @name@host, or its address such as https://misskey.io/@name",
        )
    };
    let (origin, acct) = match web_address(t) {
        Some((origin, path)) => {
            let acct = path.strip_prefix('@').ok_or_else(not_an_account)?;
            if acct.contains('/') {
                return Err(not_an_account());
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
        return Err(not_an_account());
    }
    // The host after the name is a host alone: a scheme or a path there
    // is a typo, not a server to ask.
    let host = match host {
        Some(h) => match api::normalize_host(h) {
            Ok(origin) if !h.contains("://") && !h.starts_with('@') => {
                Some(api::authority(&origin).to_string())
            }
            _ => return Err(not_an_account()),
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
/// (`misskey.io/notes/ID`) is taken as https.
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

/// A note as typed: on which server to ask, and its ID.
fn parse_note(typed: &str) -> Result<(Option<String>, String)> {
    let t = typed.trim();
    let not_a_note = || {
        Error::usage(format!("{t:?} is not a note"))
            .with_hint("give its ID, or its address such as https://misskey.io/notes/ID")
    };
    let (origin, id) = match web_address(t) {
        Some((origin, path)) => {
            let id = path.strip_prefix("notes/").ok_or_else(not_a_note)?;
            (Some(origin), id.to_string())
        }
        None => (None, t.to_string()),
    };
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(not_a_note());
    }
    Ok((origin, id))
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
    // An account of the server asked is named without a host.
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let notes = if limit == 0 {
        Vec::new()
    } else {
        let account = client.user(&who.username, host)?;
        let asked = filter.asked(reposts);
        pages(
            limit,
            filter.narrows(),
            |n| filter.keeps(n, &account.id),
            |n, until| client.user_notes(&account.id, &asked, n, until),
        )?
    };
    if let Some(dir) = save_in {
        let agent = api::agent();
        for n in &notes {
            save_files(ctx, out, &agent, client.origin(), n, &dir)?;
        }
        return Ok(());
    }
    print_notes(ctx, out, client.origin(), &notes)
}

/// Print notes: one JSON line each, or text with a blank line between.
fn print_notes(ctx: &Ctx, out: &mut dyn Write, origin: &str, notes: &[Note]) -> Result<()> {
    for (i, n) in notes.iter().enumerate() {
        if ctx.json {
            json_line(out, &format::note_json(n, origin))?;
            continue;
        }
        if i > 0 {
            text(out, "\n")?;
        }
        text(out, &format::note(n, origin))?;
    }
    Ok(())
}

/// What a file is, as `download --json` says it: `image`, `video`,
/// `audio`, or `file`.
fn kind(mime: &str) -> &'static str {
    match mime.split('/').next().unwrap_or("") {
        "image" => "image",
        "video" => "video",
        "audio" => "audio",
        _ => "file",
    }
}

/// Save every file `n` carries, its own and those of the note it renotes
/// or quotes, in `dir`, printing each as it is written.
fn save_files(
    ctx: &Ctx,
    out: &mut dyn Write,
    agent: &ureq::Agent,
    origin: &str,
    n: &Note,
    dir: &std::path::Path,
) -> Result<()> {
    for (owner, i, f) in n.attached() {
        let path = download::save(agent, f, dir, &download::file_name(&owner.id, i, f))?;
        let shown = path.to_string_lossy();
        if ctx.json {
            json_line(
                out,
                &json!({
                    "note": format::note_url(origin, &n.id),
                    "kind": kind(&f.mime),
                    "url": f.url,
                    "path": shown,
                }),
            )?;
        } else {
            text(out, &format!("{shown}\n"))?;
        }
    }
    Ok(())
}

fn show_note(ctx: &Ctx, out: &mut dyn Write, note: &str) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref())?;
    let n = client.note(&id)?;
    print_notes(ctx, out, client.origin(), std::slice::from_ref(&n))
}

fn search(ctx: &Ctx, out: &mut dyn Write, query: &str, limit: usize) -> Result<()> {
    if query.trim().is_empty() {
        return Err(Error::usage("the search query is empty"));
    }
    let client = ctx.client(None)?;
    let notes = pages(
        limit,
        false,
        |_| true,
        |n, until| client.search(query, n, until),
    )?;
    print_notes(ctx, out, client.origin(), &notes)
}

fn download_note(ctx: &Ctx, out: &mut dyn Write, note: &str, dir: &std::path::Path) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref())?;
    let n = client.note(&id)?;
    if n.attached().is_empty() {
        return Err(Error::usage(format!(
            "{} has no files",
            format::note_url(client.origin(), &n.id)
        )));
    }
    save_files(ctx, out, &api::agent(), client.origin(), &n, dir)
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
    let notes = pages(
        limit,
        false,
        |_| true,
        |n, until| client.timeline(kind.endpoint(), reposts, n, until),
    )?;
    print_notes(ctx, out, client.origin(), &notes)
}

fn notifications(ctx: &Ctx, out: &mut dyn Write, limit: usize) -> Result<()> {
    let client = ctx.account_client()?;
    let all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.notifications(n, until),
    )?;
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

/// Which side of a follow `follows` and `followers` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The accounts followed.
    Follows,
    /// The accounts that follow.
    Followers,
}

/// `msky follows` and `msky followers`: the accounts on one side of an
/// account's follows, or, with `pending`, of your requests not yet
/// accepted, newest first.
fn relations(
    ctx: &Ctx,
    out: &mut dyn Write,
    actor: Option<&str>,
    limit: usize,
    pending: bool,
    side: Side,
) -> Result<()> {
    let (client, endpoint, body) = if pending {
        if actor.is_some() {
            return Err(Error::usage(
                "--pending lists your own requests, of the account logged in",
            )
            .with_hint("leave ACTOR out"));
        }
        let client = ctx.account_client()?;
        let endpoint = match side {
            Side::Follows => "following/requests/sent",
            Side::Followers => "following/requests/list",
        };
        (client, endpoint, json!({}))
    } else {
        let (client, id) = match actor {
            Some(a) => {
                let who = parse_user(a)?;
                let client = ctx.client(who.origin.as_deref())?;
                let host = who
                    .host
                    .as_deref()
                    .filter(|h| !h.eq_ignore_ascii_case(client.host()));
                let id = client.user(&who.username, host)?.id;
                (client, id)
            }
            None => {
                let client = ctx.account_client()?;
                let id = match ctx.session(&ctx.origin)?.map(|s| s.user_id) {
                    Some(id) if !id.is_empty() => id,
                    _ => client.me().map_err(|e| ctx.explained(e))?.id,
                };
                (client, id)
            }
        };
        let endpoint = match side {
            Side::Follows => "users/following",
            Side::Followers => "users/followers",
        };
        (client, endpoint, json!({"userId": id}))
    };
    let all = pages(
        limit,
        false,
        |_| true,
        |n, until| client.relations(endpoint, body.clone(), n, until),
    )
    .map_err(|e| ctx.explained(e))?;
    for r in &all {
        let who = match side {
            Side::Follows => r.followee.as_ref(),
            Side::Followers => r.follower.as_ref(),
        };
        let Some(u) = who else { continue };
        if ctx.json {
            json_line(out, &format::account_json(u, client.origin()))?;
        } else {
            text(out, &format::account_line(u, client.origin()))?;
        }
    }
    Ok(())
}

/// `msky accounts`: the accounts logged in, by host, the one in use
/// marked; with `use_host`, that one made the one in use first.
fn accounts(ctx: &Ctx, out: &mut dyn Write, use_host: Option<&str>) -> Result<()> {
    let store = ctx.store()?;
    if let Some(typed) = use_host {
        let host = api::authority(&api::normalize_host(typed)?).to_string();
        if store.find(&host)?.is_none() {
            let known: Vec<String> = store.list()?.iter().map(|s| s.host().to_string()).collect();
            let hint = if known.is_empty() {
                "no account is logged in; run `msky login --host HOST`".to_string()
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
    let client = ctx.account_client()?;
    let me = client.me()?;
    if ctx.json {
        return json_line(out, &format::account_json(&me, client.origin()));
    }
    text(out, &format::account(&me, client.origin()))
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

/// How many servers the login lists to choose from.
const CHOICES: usize = 15;

/// Ask on `err` which of `servers` to log in on, and read the answer from
/// `input`: its number, or any host; nothing is the first.
fn choose_server(
    servers: &[crate::servers::Server],
    input: &mut dyn io::BufRead,
    err: &mut dyn Write,
) -> Result<String> {
    let mut menu = String::from("Which server do you log in on?\n");
    let shown: Vec<_> = servers.iter().take(CHOICES).collect();
    for (i, s) in shown.iter().enumerate() {
        let users = s
            .users
            .map(|n| format!("  {n} accounts"))
            .unwrap_or_default();
        menu.push_str(&format!(
            "{:>3}. {}  {}{users}\n",
            i + 1,
            s.host,
            crate::text::one_line(&s.name)
        ));
    }
    menu.push_str("Number, or another server's host [1]: ");
    say(err, &menu);
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|e| Error::io(format!("cannot read the answer: {e}")))?;
    let answer = line.trim();
    let host = match answer.parse::<usize>() {
        _ if answer.is_empty() => shown.first().map(|s| s.host.clone()),
        Ok(n) => n
            .checked_sub(1)
            .and_then(|i| shown.get(i))
            .map(|s| s.host.clone()),
        Err(_) => Some(answer.to_string()),
    };
    let Some(host) = host else {
        return Err(
            Error::usage(format!("{answer} is not one of the servers listed"))
                .with_hint("give its number, or its host"),
        );
    };
    api::normalize_host(&host)
}

/// `msky servers`: the servers to log in to, one a line, or one JSON
/// object a line.
fn servers(
    ctx: &Ctx,
    out: &mut dyn Write,
    list: &[crate::servers::Server],
    limit: usize,
) -> Result<()> {
    for s in list.iter().take(limit) {
        if ctx.json {
            json_line(
                out,
                &json!({
                    "host": s.host,
                    "name": s.name,
                    "users": s.users,
                    "langs": s.langs,
                    "openRegistrations": s.open,
                }),
            )?;
        } else {
            let users = s
                .users
                .map(|n| format!("  {n} accounts"))
                .unwrap_or_default();
            let closed = if s.open == Some(false) {
                "  (closed to new accounts)"
            } else {
                ""
            };
            text(
                out,
                &format!(
                    "{}  {}{users}{closed}\n",
                    s.host,
                    crate::text::one_line(&s.name)
                ),
            )?;
        }
    }
    Ok(())
}

/// `msky login`: ask the server for access with MiAuth, wait for it, and
/// save the token for the server with the permissions it was asked for.
#[expect(
    clippy::too_many_arguments,
    reason = "what the login reads and writes, each one a test gives its own"
)]
fn login(
    ctx: &Ctx,
    origin: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
    timeout: Duration,
    interval: Duration,
    clock: &mut dyn Clock,
    open_browser: bool,
) -> Result<()> {
    let store = ctx.store()?;
    // The check is asked as a visitor: an old token is not sent along.
    let client = Client::new(origin);
    let host = client.host().to_string();
    let session = miauth::session()?;
    let url = miauth::auth_url(client.origin(), &session);
    say(
        err,
        &format!(
            "To log in on {host}, open this address, signed in there, and allow msky to read your account, post notes and follow:\n{url}\n"
        ),
    );
    if open_browser && browser::open(&url) {
        say(err, "(opened in the web browser)\n");
    }
    say(
        err,
        &format!(
            "Waiting for the access to be allowed (up to {} s; Ctrl-C to give up)...\n",
            timeout.as_secs()
        ),
    );
    let access = miauth::wait_for_access(&client, &session, timeout, interval, clock)?;
    let saved = miauth::session_of(&client, access)?;
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

/// `msky logout`: the account in use (or the one on `--host`), or all of
/// them. Only the saved token goes; the access stays listed on the server,
/// where it can be revoked.
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
mod tests {
    use super::*;
    use rstest::rstest;

    fn user(origin: Option<&str>, name: &str, host: Option<&str>) -> UserRef {
        UserRef {
            origin: origin.map(str::to_string),
            username: name.to_string(),
            host: host.map(str::to_string),
        }
    }

    #[rstest]
    #[case("alice", user(None, "alice", None))]
    #[case(" @alice ", user(None, "alice", None))]
    #[case("@alice@Misskey.Design", user(None, "alice", Some("misskey.design")))]
    #[case("alice@mstdn.jp", user(None, "alice", Some("mstdn.jp")))]
    #[case("@a_b-c.d", user(None, "a_b-c.d", None))]
    #[case(
        "https://misskey.io/@alice",
        user(Some("https://misskey.io"), "alice", None)
    )]
    #[case(
        "https://misskey.io/@alice@mstdn.jp/?tab=notes#x",
        user(Some("https://misskey.io"), "alice", Some("mstdn.jp"))
    )]
    #[case(
        "http://127.0.0.1:8080/@bob",
        user(Some("http://127.0.0.1:8080"), "bob", None)
    )]
    #[case("misskey.io/@alice", user(Some("https://misskey.io"), "alice", None))]
    fn an_account_is_read_however_it_is_written(#[case] typed: &str, #[case] want: UserRef) {
        assert_eq!(parse_user(typed).unwrap(), want);
    }

    #[rstest]
    #[case("")]
    #[case("@")]
    #[case("@@host")]
    #[case("@alice@")]
    #[case("@alice@host/x")]
    #[case("あいう")]
    #[case("alice 🐈")]
    #[case("https://misskey.io/notes/9abc")]
    #[case("https://misskey.io/@alice/following")]
    #[case("https://misskey.io/")]
    fn what_is_not_an_account_is_a_usage_error(#[case] typed: &str) {
        let e = parse_user(typed).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage, "{}", e.message());
    }

    #[rstest]
    #[case("9abcdef", None, "9abcdef")]
    #[case(" 9abc ", None, "9abc")]
    #[case("https://misskey.io/notes/9abc", Some("https://misskey.io"), "9abc")]
    #[case(
        "https://Misskey.IO/notes/9abc/?x=1#r",
        Some("https://misskey.io"),
        "9abc"
    )]
    #[case("http://127.0.0.1:9/notes/a1", Some("http://127.0.0.1:9"), "a1")]
    #[case("misskey.io/notes/9abc", Some("https://misskey.io"), "9abc")]
    fn a_note_is_read_however_it_is_written(
        #[case] typed: &str,
        #[case] origin: Option<&str>,
        #[case] id: &str,
    ) {
        assert_eq!(
            parse_note(typed).unwrap(),
            (origin.map(str::to_string), id.to_string())
        );
    }

    #[rstest]
    #[case("")]
    #[case("../x")]
    #[case("9abc?x")]
    #[case("ノート")]
    #[case("https://misskey.io/@alice")]
    #[case("https://misskey.io/notes/")]
    #[case("https://misskey.io/notes/a/b")]
    #[case("ftp://misskey.io/notes/a")]
    #[case("/notes/a")]
    fn what_is_not_a_note_is_a_usage_error(#[case] typed: &str) {
        let e = parse_note(typed).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage, "{}", e.message());
    }

    fn page(ids: &[&str]) -> Vec<Note> {
        ids.iter()
            .map(|id| Note {
                id: id.to_string(),
                ..Default::default()
            })
            .collect()
    }

    // Pages are asked for until -n notes came, each older than the last
    // note of the one before, and no more than the API's 100 at a time.
    #[test]
    fn pages_go_back_until_the_limit() {
        let mut asked = Vec::new();
        let got = pages(
            250,
            false,
            |_| true,
            |n, until| {
                asked.push((n, until.map(str::to_string)));
                let start = asked.len() * 1000;
                Ok((0..n)
                    .map(|i| Note {
                        id: format!("{}", start - i),
                        ..Default::default()
                    })
                    .collect())
            },
        )
        .unwrap();
        assert_eq!(got.len(), 250);
        assert_eq!(
            asked,
            [
                (100, None),
                (100, Some("901".into())),
                (50, Some("1901".into()))
            ]
        );
    }

    // A short page is not the end (Misskey leaves out what the visitor may
    // not see); an empty page is, and so is one that brings nothing new.
    #[test]
    fn pages_end_on_an_empty_page_or_one_with_nothing_new() {
        let mut calls = 0;
        let got = pages(
            10,
            false,
            |_| true,
            |_, until| {
                calls += 1;
                Ok(match until {
                    None => page(&["c", "b"]),
                    Some("b") => page(&["a"]),
                    _ => Vec::new(),
                })
            },
        )
        .unwrap();
        assert_eq!(
            got.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
        assert_eq!(calls, 3);
        let mut calls = 0;
        let got = pages(
            10,
            false,
            |_| true,
            |_, _| {
                calls += 1;
                Ok(page(&["b", "a"]))
            },
        )
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(calls, 2, "the same page twice ends it");
    }

    // A filter told here leaves notes out of each page, so every page asked
    // for is a full one, and pages go on until enough are kept; a page that
    // starts from an ID seen before ends it.
    #[test]
    fn a_filter_told_here_reads_full_pages_until_enough_are_kept() {
        let mut asked = Vec::new();
        let got = pages(
            2,
            true,
            |n: &Note| n.id.ends_with('v'),
            |n, until| {
                asked.push((n, until.map(str::to_string)));
                Ok(match until {
                    None => page(&["9x", "8v", "7x"]),
                    Some("7x") => page(&["6x", "5x"]),
                    Some("5x") => page(&["4v", "3v", "2x"]),
                    _ => Vec::new(),
                })
            },
        )
        .unwrap();
        assert_eq!(
            got.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["8v", "4v"]
        );
        assert_eq!(
            asked.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [100, 100, 100]
        );
        let mut calls = 0;
        pages(
            10,
            true,
            |_| true,
            |_, until| {
                calls += 1;
                Ok(match until {
                    None => page(&["b", "a"]),
                    _ => page(&["c", "a"]),
                })
            },
        )
        .unwrap();
        assert_eq!(calls, 2, "a page from an ID seen before ends it");
    }

    fn note(v: Value) -> Note {
        serde_json::from_value(v).unwrap()
    }

    #[rstest]
    #[case::with_replies(AuthorFilter::WithReplies, true, false)]
    #[case::no_replies(AuthorFilter::NoReplies, false, false)]
    #[case::media(AuthorFilter::Media, false, true)]
    #[case::video(AuthorFilter::Video, false, true)]
    #[case::threads(AuthorFilter::Threads, true, false)]
    fn each_filter_asks_for_what_misskey_can_filter(
        #[case] f: AuthorFilter,
        #[case] replies: bool,
        #[case] files: bool,
    ) {
        assert_eq!(
            f.asked(true),
            NoteFilter {
                replies,
                renotes: true,
                files
            }
        );
        assert!(!f.asked(false).renotes);
    }

    // video keeps the notes with a video, a renoted or quoted one's
    // included; threads keeps notes and the replies the account made to
    // itself.
    #[test]
    fn video_and_threads_are_told_from_the_notes() {
        let clip = json!({"type": "video/mp4"});
        let pic = json!({"type": "image/png"});
        let v = AuthorFilter::Video;
        assert!(v.keeps(&note(json!({"id": "a", "files": [pic, clip]})), "me"));
        assert!(!v.keeps(&note(json!({"id": "a", "files": [pic]})), "me"));
        assert!(v.keeps(
            &note(json!({"id": "q", "renoteId": "o", "renote": {"id": "o", "files": [clip]}})),
            "me"
        ));
        let t = AuthorFilter::Threads;
        assert!(t.keeps(&note(json!({"id": "a"})), "me"));
        assert!(t.keeps(
            &note(json!({"id": "a", "replyId": "r", "reply": {"id": "r", "userId": "me"}})),
            "me"
        ));
        assert!(!t.keeps(
            &note(json!({"id": "a", "replyId": "r", "reply": {"id": "r", "userId": "bob"}})),
            "me"
        ));
        assert!(!t.keeps(&note(json!({"id": "a", "replyId": "r"})), "me"));
        assert!(AuthorFilter::Media.keeps(&note(json!({"id": "a", "replyId": "r"})), "me"));
    }

    #[test]
    fn an_error_on_a_later_page_is_returned() {
        let e = pages(
            10,
            false,
            |_| true,
            |_, until| match until {
                None => Ok(page(&["b"])),
                Some(_) => Err(Error::api("boom")),
            },
        )
        .unwrap_err();
        assert_eq!(e.message(), "boom");
    }

    /// A stdout that refuses every write with `kind`.
    struct Refusing(io::ErrorKind);

    impl Write for Refusing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(self.0, "refused"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // A reader that stopped reading (`| head`) ends the command quietly; an
    // output that cannot be written (a full disk) is an error.
    #[test]
    fn a_closed_pipe_ends_the_command_and_a_full_disk_fails_it() {
        let gone = &mut Refusing(io::ErrorKind::BrokenPipe);
        assert_eq!(ended_by_reader(text(gone, "x")), Ok(()));
        let full = &mut Refusing(io::ErrorKind::StorageFull);
        let e = ended_by_reader(text(full, "x")).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Io);
        assert_eq!(e.message(), "cannot write the output: refused");
    }

    #[test]
    fn an_empty_query_is_refused_before_anything_is_asked() {
        let ctx = Ctx {
            origin: "http://127.0.0.1:9".into(),
            json: false,
            host_given: true,
            accounts: Err(Error::io("no config directory")),
            env_token: None,
        };
        let e = search(&ctx, &mut Vec::new(), "  ", 5).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage);
    }

    use crate::api::stub_tests::{Calls, reply, serve};
    use std::sync::Arc;

    /// A throwaway value in the place of a token; no server gave it.
    const FAKE: &str = "fake-token-for-tests";

    /// Time that goes by only when slept.
    #[derive(Default)]
    struct FakeClock {
        now: Duration,
        slept: Vec<Duration>,
    }

    impl Clock for FakeClock {
        fn elapsed(&self) -> Duration {
            self.now
        }
        fn sleep(&mut self, d: Duration) {
            self.now += d;
            self.slept.push(d);
        }
    }

    fn ctx_of(origin: &str, dir: &std::path::Path, json: bool) -> Ctx {
        Ctx {
            origin: origin.into(),
            json,
            host_given: true,
            accounts: Ok(AccountStore::open(dir)),
            env_token: None,
        }
    }

    fn run_login(ctx: &Ctx, timeout: u64, clock: &mut FakeClock) -> (Result<()>, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = login(
            ctx,
            &ctx.origin,
            &mut out,
            &mut err,
            Duration::from_secs(timeout),
            Duration::from_secs(2),
            clock,
            false,
        );
        (
            r,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    // The address asks for what msky does; the check is asked every two
    // seconds until the access is given, and the token is saved for the
    // server, with the account the check names.
    #[test]
    fn login_waits_for_the_access_and_saves_the_token_for_the_server() {
        let (url, log) = serve(Arc::new(|path, _, n| match (path, n) {
            (p, 0 | 1) if p.ends_with("/check") => reply(200, r#"{"ok":false}"#),
            (p, _) if p.ends_with("/check") => reply(
                200,
                &format!(
                    r#"{{"ok":true,"token":"{FAKE}","user":{{"id":"u1","username":"alice","name":"アリス 🐈"}}}}"#
                ),
            ),
            _ => reply(404, "{}"),
        }));
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_of(&url, dir.path(), false);
        let mut clock = FakeClock::default();
        let (r, out, err) = run_login(&ctx, 300, &mut clock);
        r.unwrap();
        let host = api::authority(&url);
        assert_eq!(out, format!("logged in as @alice@{host}\n"));
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 3);
        let session = log[0]
            .0
            .strip_prefix("/api/miauth/")
            .unwrap()
            .strip_suffix("/check")
            .unwrap();
        assert!(
            log.iter().all(|(p, body)| p == &log[0].0 && body == "{}"),
            "the check carries no token"
        );
        assert!(
            err.contains(&format!("{url}/miauth/{session}?name=msky&permission=read:account,read:notifications,read:following,read:reactions,write:notes,write:drive,write:following,write:reactions,write:notifications,read:mutes,write:mutes,read:blocks,write:blocks,read:chat,write:chat\n")),
            "{err}"
        );
        assert!(!err.contains(FAKE) && !out.contains(FAKE));
        assert_eq!(clock.slept, [Duration::from_secs(2); 2]);
        let saved = AccountStore::open(dir.path()).find(host).unwrap().unwrap();
        assert_eq!(
            (
                saved.service.as_str(),
                saved.user_id.as_str(),
                saved.username.as_str(),
                saved.token.as_str()
            ),
            (url.as_str(), "u1", "alice", FAKE)
        );
        assert_eq!(
            AccountStore::open(dir.path()).current_host().as_deref(),
            Some(host)
        );
    }

    // A check that names no account is followed by `i`, with the new token.
    #[test]
    fn login_asks_the_server_who_it_is_when_the_check_does_not_say() {
        let (url, log) = serve(Arc::new(|path, _, _| match path {
            "/api/i" => reply(200, r#"{"id":"u2","username":"bob"}"#),
            _ => reply(200, &format!(r#"{{"ok":true,"token":"{FAKE}"}}"#)),
        }));
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_of(&url, dir.path(), true);
        let (r, out, _) = run_login(&ctx, 300, &mut FakeClock::default());
        r.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            v,
            json!({"id": "u2", "username": "bob", "host": api::authority(&url)})
        );
        let log = log.lock().unwrap();
        assert_eq!(log[1].0, "/api/i");
        assert_eq!(
            serde_json::from_str::<Value>(&log[1].1).unwrap(),
            json!({"i": FAKE})
        );
    }

    // Nothing allowed within the time is exit 1 with what to do, and
    // nothing is saved; a server that turns the check down is too.
    #[rstest]
    #[case::time_runs_out(200, r#"{"ok":false}"#, "was not given on", 4)]
    #[case::denied(
        403,
        r#"{"error":{"code":"ACCESS_DENIED","message":"The user denied it."}}"#,
        "did not give msky the access: ACCESS_DENIED: The user denied it.",
        1
    )]
    fn login_that_is_not_allowed_is_a_usage_error_and_saves_nothing(
        #[case] status: u16,
        #[case] body: &'static str,
        #[case] said: &str,
        #[case] checks: usize,
    ) {
        let (url, log) = serve(Arc::new(move |_, _, _| reply(status, body)));
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_of(&url, dir.path(), false);
        let mut clock = FakeClock::default();
        let (r, out, _) = run_login(&ctx, 5, &mut clock);
        let e = r.unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage);
        assert_eq!(e.kind().exit_code(), 1);
        assert!(e.message().contains(said), "{e:?}");
        assert!(e.hint().unwrap().contains("nothing was saved"));
        assert!(out.is_empty());
        assert_eq!(log.lock().unwrap().len(), checks);
        assert!(!dir.path().join("accounts").exists());
        if checks > 1 {
            // 2 + 2 + 1: the last wait is cut to the time left.
            assert_eq!(clock.slept.iter().sum::<Duration>(), Duration::from_secs(5));
        }
    }

    fn save(dir: &std::path::Path, service: &str, name: &str) {
        AccountStore::open(dir)
            .save(&Session {
                service: service.into(),
                user_id: format!("id-{name}"),
                username: name.into(),
                token: format!("{FAKE}-{name}"),
                permissions: miauth::PERMISSIONS.iter().map(|p| p.to_string()).collect(),
            })
            .unwrap();
    }

    // A token is sent only to the server it was saved for: not to another
    // host, nor to the same host on another scheme.
    #[test]
    fn a_token_goes_only_to_the_server_that_gave_it() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), "https://a.test", "alice");
        let ctx = ctx_of("https://a.test", dir.path(), false);
        assert!(ctx.client(None).unwrap().logged_in());
        assert!(!ctx.client(Some("https://b.test")).unwrap().logged_in());
        assert!(!ctx.client(Some("http://a.test")).unwrap().logged_in());
        let none = Ctx {
            accounts: Err(Error::io("no config directory")),
            ..ctx_of("https://a.test", dir.path(), false)
        };
        assert!(!none.client(None).unwrap().logged_in());
    }

    // The commands of an account say so before anything is asked when
    // there is none; local and global are read as a visitor.
    #[test]
    fn the_commands_of_an_account_need_one_and_the_others_do_not() {
        let (url, log) = serve(Arc::new(|_, _, _| reply(200, "[]")));
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_of(&url, dir.path(), false);
        let host = api::authority(&url).to_string();
        for e in [
            timeline(&ctx, &mut Vec::new(), TimelineKind::Home, 5, false).unwrap_err(),
            timeline(&ctx, &mut Vec::new(), TimelineKind::Social, 5, false).unwrap_err(),
            notifications(&ctx, &mut Vec::new(), 5).unwrap_err(),
            whoami(&ctx, &mut Vec::new()).unwrap_err(),
        ] {
            assert_eq!(e.kind(), crate::error::Kind::Usage);
            assert_eq!(e.message(), format!("not logged in on {host}"));
            assert_eq!(
                e.hint(),
                Some(format!("run `msky login --host {host}`").as_str())
            );
        }
        assert!(log.lock().unwrap().is_empty());
        timeline(&ctx, &mut Vec::new(), TimelineKind::Local, 5, true).unwrap();
        timeline(&ctx, &mut Vec::new(), TimelineKind::Global, 5, false).unwrap();
        let log = log.lock().unwrap();
        assert_eq!(log[0].0, "/api/notes/local-timeline");
        assert_eq!(
            serde_json::from_str::<Value>(&log[0].1).unwrap(),
            json!({"limit": 5, "withRenotes": true})
        );
        assert_eq!(log[1].0, "/api/notes/global-timeline");
    }

    // Notifications are paged as author pages notes: by untilId, each once.
    #[test]
    fn notifications_are_paged_like_notes() {
        let (url, log) = serve(Arc::new(|_, body, _| {
            let v: Value = serde_json::from_str(body).unwrap();
            match v["untilId"].as_str() {
                None => reply(
                    200,
                    r#"[{"id":"n3","type":"follow","user":{"username":"bob"}},{"id":"n2","type":"follow","user":{"username":"carol"}}]"#,
                ),
                Some("n2") => reply(
                    200,
                    r#"[{"id":"n2","type":"follow"},{"id":"n1","type":"follow","user":{"username":"dave"}}]"#,
                ),
                _ => reply(200, "[]"),
            }
        }));
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &url, "alice");
        let ctx = ctx_of(&url, dir.path(), true);
        let mut out = Vec::new();
        notifications(&ctx, &mut out, 3).unwrap();
        let ids: Vec<String> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(ids, ["n3", "n2", "n1"]);
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        let second: Value = serde_json::from_str(&log[1].1).unwrap();
        assert_eq!(second["limit"], 1);
        assert_eq!(second["markAsRead"], false);
        assert_eq!(second["i"], format!("{FAKE}-alice"));
    }

    // logout takes the account in use, or the one on --host, or all of
    // them, and says who; with nothing saved it says so.
    #[test]
    fn logout_says_who_was_logged_out() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), "https://a.test", "alice");
        save(dir.path(), "https://b.test", "bob");
        let run = |ctx: &Ctx, all: bool| {
            let mut out = Vec::new();
            logout(ctx, &mut out, all).unwrap();
            String::from_utf8(out).unwrap()
        };
        let in_use = Ctx {
            host_given: false,
            ..ctx_of("https://b.test", dir.path(), false)
        };
        assert_eq!(run(&in_use, false), "logged out @bob@b.test\n");
        let other = ctx_of("https://c.test", dir.path(), true);
        assert_eq!(run(&other, false), "{\"loggedOut\":[]}\n");
        save(dir.path(), "https://b.test", "bob");
        assert_eq!(
            run(&other, true),
            "{\"loggedOut\":[{\"host\":\"a.test\",\"id\":\"id-alice\",\"username\":\"alice\"},{\"host\":\"b.test\",\"id\":\"id-bob\",\"username\":\"bob\"}]}\n"
        );
        assert_eq!(run(&in_use, false), "not logged in\n");
    }

    fn relation_server() -> (String, Calls) {
        serve(Arc::new(|path, body, _| {
            let v: Value = serde_json::from_str(body).unwrap();
            let page = |k: &str| match v["untilId"].as_str() {
                None => format!(
                    r#"[{{"id":"r2","{k}":{{"id":"u-bob","username":"bob","host":"mstdn.test","name":"ボブ 👨‍👩‍👧","isFollowing":true}}}},{{"id":"r1","{k}":{{"id":"u-cat","username":"cat"}}}}]"#
                ),
                Some("r1") => format!(r#"[{{"id":"r0","{k}":{{"id":"u-dan","username":"dan"}}}}]"#),
                _ => "[]".into(),
            };
            match path {
                "/api/users/show" => reply(200, r#"{"id":"u-alice","username":"alice"}"#),
                "/api/users/following" | "/api/following/requests/sent" => {
                    reply(200, &page("followee"))
                }
                "/api/users/followers" | "/api/following/requests/list" => {
                    reply(200, &page("follower"))
                }
                _ => reply(404, "{}"),
            }
        }))
    }

    fn run_relations(
        ctx: &Ctx,
        actor: Option<&str>,
        pending: bool,
        side: Side,
    ) -> (Result<()>, String) {
        let mut out = Vec::new();
        let r = relations(ctx, &mut out, actor, 10, pending, side);
        (r, String::from_utf8(out).unwrap())
    }

    // follows and followers page by untilId as bsky's page by cursor, say
    // who is followed, and print the account shape whoami prints.
    #[test]
    fn follows_and_followers_list_the_accounts_page_by_page() {
        let (url, log) = relation_server();
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &url, "me");
        let host = api::authority(&url).to_string();
        let ctx = ctx_of(&url, dir.path(), false);
        let (r, out) = run_relations(&ctx, Some("@alice"), false, Side::Follows);
        r.unwrap();
        assert_eq!(
            out,
            format!(
                "ボブ 👨‍👩‍👧 @bob@mstdn.test  (following)  {url}/@bob@mstdn.test\n@cat@{host}  {url}/@cat\n@dan@{host}  {url}/@dan\n"
            )
        );
        {
            let log = log.lock().unwrap();
            assert_eq!(log[0].0, "/api/users/show");
            assert_eq!(log[1].0, "/api/users/following");
            let second: Value = serde_json::from_str(&log[2].1).unwrap();
            assert_eq!(second["userId"], "u-alice");
            assert_eq!(second["untilId"], "r1");
        }
        // Without ACTOR it is your own, by the ID saved at login.
        let json_ctx = ctx_of(&url, dir.path(), true);
        let (r, out) = run_relations(&json_ctx, None, false, Side::Followers);
        r.unwrap();
        let first: Value = serde_json::from_str(out.lines().next().unwrap()).unwrap();
        assert_eq!(
            first,
            format::account_json(
                &serde_json::from_value(
                    json!({"id":"u-bob","username":"bob","host":"mstdn.test","name":"ボブ 👨‍👩‍👧"})
                )
                .unwrap(),
                &url
            )
        );
        let log = log.lock().unwrap();
        let asked = log
            .iter()
            .find(|(p, _)| p == "/api/users/followers")
            .unwrap();
        let v: Value = serde_json::from_str(&asked.1).unwrap();
        assert_eq!(v["userId"], "id-me");
    }

    // --pending lists your own requests: those sent (follows) and those
    // received (followers); it needs an account, and no ACTOR.
    #[test]
    fn pending_lists_the_requests_not_yet_accepted() {
        let (url, log) = relation_server();
        let dir = tempfile::tempdir().unwrap();
        let ctx = ctx_of(&url, dir.path(), false);
        let e = run_relations(&ctx, None, true, Side::Follows)
            .0
            .unwrap_err();
        assert!(e.message().starts_with("not logged in on"), "{e:?}");
        save(dir.path(), &url, "me");
        let e = run_relations(&ctx, Some("bob"), true, Side::Follows)
            .0
            .unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage);
        assert!(log.lock().unwrap().is_empty());
        let (r, out) = run_relations(&ctx, None, true, Side::Follows);
        r.unwrap();
        assert_eq!(out.lines().count(), 3);
        assert!(out.starts_with("ボブ 👨‍👩‍👧 @bob@mstdn.test"), "{out}");
        run_relations(&ctx, None, true, Side::Followers).0.unwrap();
        let log = log.lock().unwrap();
        assert_eq!(log[0].0, "/api/following/requests/sent");
        let v: Value = serde_json::from_str(&log[0].1).unwrap();
        assert_eq!(v, json!({"i": format!("{FAKE}-me"), "limit": 10}));
        assert!(log.iter().any(|(p, _)| p == "/api/following/requests/list"));
    }

    // accounts lists the logins as bsky's does, the one in use marked;
    // --use makes another the one in use, and one not logged in is a usage
    // error that names those that are.
    #[test]
    fn accounts_lists_the_logins_and_use_switches_between_them() {
        let dir = tempfile::tempdir().unwrap();
        let run = |ctx: &Ctx, use_host: Option<&str>| {
            let mut out = Vec::new();
            let r = accounts(ctx, &mut out, use_host);
            (r, String::from_utf8(out).unwrap())
        };
        let ctx = ctx_of("https://a.test", dir.path(), false);
        assert_eq!(run(&ctx, None).1, "not logged in\n");
        save(dir.path(), "https://a.test", "alice");
        save(dir.path(), "http://127.0.0.1:8080", "bob");
        assert_eq!(
            run(&ctx, None).1,
            "* @bob@127.0.0.1:8080  id-bob\n  @alice@a.test  id-alice\n"
        );
        let (r, out) = run(&ctx, Some("https://A.test/"));
        r.unwrap();
        assert_eq!(
            out,
            "  @bob@127.0.0.1:8080  id-bob\n* @alice@a.test  id-alice\n"
        );
        let e = run(&ctx, Some("c.test")).0.unwrap_err();
        assert_eq!(e.message(), "no account is logged in on c.test");
        assert_eq!(e.hint(), Some("logged in on: 127.0.0.1:8080, a.test"));
        let json_ctx = ctx_of("https://a.test", dir.path(), true);
        let lines: Vec<Value> = run(&json_ctx, None)
            .1
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            lines[1],
            json!({"id": "id-alice", "username": "alice", "host": "a.test", "service": "https://a.test", "current": true})
        );
    }

    fn server_list() -> Vec<crate::servers::Server> {
        crate::servers::builtin()
    }

    // The login asks which server: a number, nothing for the first, or any
    // host; a number not listed is a usage error.
    #[rstest]
    #[case("\n", "https://misskey.io")]
    #[case("3\n", "https://takusuki.com")]
    #[case(" Mi.Example.Test \n", "https://mi.example.test")]
    #[case("http://127.0.0.1:9\n", "http://127.0.0.1:9")]
    fn the_login_lists_servers_and_takes_a_number_or_a_host(
        #[case] typed: &str,
        #[case] want: &str,
    ) {
        let mut err = Vec::new();
        let got = choose_server(&server_list(), &mut typed.as_bytes(), &mut err).unwrap();
        assert_eq!(got, want);
        let err = String::from_utf8(err).unwrap();
        assert!(
            err.starts_with("Which server do you log in on?\n  1. misskey.io  Misskey.io\n"),
            "{err}"
        );
        assert!(
            err.ends_with("Number, or another server's host [1]: "),
            "{err}"
        );
    }

    #[test]
    fn a_number_not_listed_is_a_usage_error() {
        for typed in ["0\n", "99\n"] {
            let e =
                choose_server(&server_list(), &mut typed.as_bytes(), &mut Vec::new()).unwrap_err();
            assert_eq!(e.kind(), crate::error::Kind::Usage, "{typed}");
        }
        let e =
            choose_server(&server_list(), &mut "a/b\n".as_bytes(), &mut Vec::new()).unwrap_err();
        assert!(e.message().starts_with("invalid host"), "{e:?}");
    }

    #[test]
    fn servers_prints_each_with_its_accounts_or_json() {
        let list = vec![crate::servers::Server {
            host: "big.test".into(),
            name: "大きい🐈".into(),
            users: Some(9000),
            langs: vec!["ja".into()],
            open: Some(false),
        }];
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        servers(
            &ctx_of("https://a.test", dir.path(), false),
            &mut out,
            &list,
            5,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "big.test  大きい🐈  9000 accounts  (closed to new accounts)\n"
        );
        let mut out = Vec::new();
        servers(
            &ctx_of("https://a.test", dir.path(), true),
            &mut out,
            &list,
            5,
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            v,
            json!({"host": "big.test", "name": "大きい🐈", "users": 9000, "langs": ["ja"], "openRegistrations": false})
        );
    }
}
