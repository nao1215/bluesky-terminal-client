//! bsky: a Bluesky client for the terminal.

mod api;
mod browser;
mod config;
mod error;
mod hls;
mod media;
mod terminal;
mod timeline;
mod tui;
mod video;

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::{AccountStore, Session, SessionStore, SettingsStore};
use crate::error::{Error, Kind, Result};

/// Command line: `bsky` opens the client, `bsky logout` logs an account out.
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "bsky", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// PDS to log in to.
    #[arg(long, env = "BSKY_SERVICE", value_name = "URL", default_value = api::DEFAULT_SERVICE)]
    service: String,

    /// Account to use for this run: its handle or DID (default: the one in use).
    #[arg(
        short = 'a',
        long,
        env = "BSKY_ACCOUNT",
        value_name = "HANDLE",
        global = true
    )]
    account: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Log the account in use (or the one -a names) out.
    Logout {
        /// Log every account out.
        #[arg(long)]
        all: bool,
    },
}

const AFTER_HELP: &str = "\
Pictures and videos are drawn with kitty graphics, sixel, or iTerm2 inline
images. On a terminal that supports none of them bsky runs as text, and
Space opens a post's pictures or video on bsky.app in the web browser.

Exit status: 0 success, 1 usage error, 2 not an interactive terminal,
3 local file error. Network and server errors are shown in the client and
do not end it.";

fn run(cli: Cli) -> Result<()> {
    let service = api::normalize_service(&cli.service)?;
    let dir = config::config_dir()?;
    match cli.command {
        Some(Command::Logout { all }) => logout(&dir, cli.account.as_deref(), all),
        None => {
            let accounts = AccountStore::open(&dir)?;
            let session = chosen_account(&accounts, cli.account.as_deref())?;
            tui::run(accounts, session, SettingsStore::new(&dir), &service)
        }
    }
}

/// The account `-a` names, or the one in use. An account that is not
/// logged in is a usage error naming those that are.
fn chosen_account(accounts: &AccountStore, who: Option<&str>) -> Result<Option<Session>> {
    let Some(who) = who.filter(|w| !w.trim().is_empty()) else {
        return accounts.current();
    };
    if let Some(s) = accounts.find(who)? {
        return Ok(Some(s));
    }
    let known: Vec<String> = accounts
        .list()?
        .iter()
        .map(|s| format!("@{}", s.handle))
        .collect();
    let hint = if known.is_empty() {
        "no account is logged in; run bsky to log in".to_string()
    } else {
        format!("logged in: {}", known.join(", "))
    };
    Err(Error::new(Kind::Usage, format!("{who} is not logged in")).with_hint(hint))
}

/// `bsky logout`: the account in use, the one `-a` names, or all of them.
fn logout(dir: &std::path::Path, who: Option<&str>, all: bool) -> Result<()> {
    // A session.json of a version with one login, readable or not, is that
    // one login: it goes, and nothing else is touched.
    let old = SessionStore::new(dir);
    if who.is_none() && old.path().exists() {
        old.clear()?;
        println!("logged out");
        return Ok(());
    }
    let accounts = AccountStore::open(dir)?;
    let chosen: Vec<Session> = if all {
        accounts.list()?
    } else {
        chosen_account(&accounts, who)?.into_iter().collect()
    };
    if chosen.is_empty() {
        println!("not logged in");
        return Ok(());
    }
    for s in chosen {
        accounts.remove(&s.did)?;
        println!("logged out @{}", s.handle);
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                let _ = e.print();
                return ExitCode::SUCCESS;
            }
            let err = Error::new(Kind::Usage, first_line(&e.to_string()))
                .with_hint("run `bsky --help` for usage");
            eprintln!("{err}");
            return ExitCode::from(Kind::Usage.exit_code());
        }
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "{err}");
            ExitCode::from(err.kind().exit_code())
        }
    }
}

/// clap's message without its `error: ` prefix and trailing usage block.
fn first_line(msg: &str) -> String {
    let line = msg.lines().next().unwrap_or(msg);
    line.strip_prefix("error: ").unwrap_or(line).to_string()
}
