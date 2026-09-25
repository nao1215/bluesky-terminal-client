//! bsky: a Bluesky client for the terminal.

mod api;
mod browser;
mod cli;
mod clock;
mod compose;
mod config;
mod error;
mod hls;
mod i18n;
mod langs;
mod media;
mod terminal;
mod timeline;
mod tui;
mod video;

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::{AccountStore, Session, SettingsStore};
use crate::error::{Error, Kind, Result};

/// Command line: `bsky` opens the client; the commands read and write
/// Bluesky from scripts (see `cli`).
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "bsky", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// PDS to log in to.
    #[arg(
        long,
        env = "BSKY_SERVICE",
        value_name = "URL",
        default_value = api::DEFAULT_SERVICE,
        global = true
    )]
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

    /// Print JSON: the server's objects, one per line for a list, what a
    /// write answered, and errors as {"error": ...} on stdout too.
    #[arg(long, global = true)]
    json: bool,

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
    #[command(flatten)]
    Other(cli::Command),
}

const AFTER_HELP: &str = "\
Pictures and videos are drawn with kitty graphics, sixel, or iTerm2 inline
images. On a terminal that supports none of them bsky runs as text, and
Space opens a post's pictures or video on bsky.app in the web browser.

Without a command bsky opens the client. The commands read and write
Bluesky from scripts, as the account in use or the one -a names; --json
prints the server's own objects.

Exit status: 0 success, 1 usage error, 2 not an interactive terminal,
3 local file error, 4 network or server error (a command's; the client
shows them and keeps running).";

fn run(cli: Cli) -> Result<()> {
    let service = api::normalize_service(&cli.service)?;
    let dir = config::config_dir()?;
    match cli.command {
        Some(Command::Logout { all }) => logout(&dir, cli.account.as_deref(), all, cli.json),
        Some(Command::Other(cmd)) => {
            let accounts = AccountStore::open(&dir)?;
            // Logging in and listing the accounts act as no account, so an
            // account named that is not logged in (yet) is no error there.
            let session = match cmd {
                cli::Command::Login { .. } | cli::Command::Accounts => accounts.current()?,
                _ => chosen_account(&accounts, cli.account.as_deref())?,
            };
            let ctx = cli::Ctx {
                dir: &dir,
                accounts: &accounts,
                session,
                service: &service,
                json: cli.json,
            };
            cli::run(cmd, &ctx)
        }
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
fn logout(dir: &std::path::Path, who: Option<&str>, all: bool, json: bool) -> Result<()> {
    let accounts = AccountStore::open(dir)?;
    let chosen: Vec<Session> = if all {
        accounts.remove_all()?
    } else {
        let chosen: Vec<Session> = chosen_account(&accounts, who)?.into_iter().collect();
        for s in &chosen {
            accounts.remove(&s.did)?;
        }
        chosen
    };
    if json {
        let gone: Vec<serde_json::Value> = chosen
            .iter()
            .map(|s| serde_json::json!({"did": s.did, "handle": s.handle}))
            .collect();
        say(&serde_json::json!({ "loggedOut": gone }).to_string());
    } else if chosen.is_empty() {
        say("not logged in");
    } else {
        for s in &chosen {
            say(&format!("logged out @{}", s.handle));
        }
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
            let err = Error::new(Kind::Usage, first_paragraph(&e.to_string()))
                .with_hint("run `bsky --help` for usage");
            eprintln!("{}", shown(&err));
            // The flags were not read, so --json is looked for by hand.
            if std::env::args_os().skip(1).any(|a| a == "--json") {
                print_json_error(&err);
            }
            return ExitCode::from(Kind::Usage.exit_code());
        }
    };
    let json = cli.json;
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = writeln!(std::io::stderr(), "{}", shown(&err));
            let status = err.kind().exit_code();
            if json {
                print_json_error(&err);
            }
            ExitCode::from(status)
        }
    }
}

/// An error as it is printed on stderr: as text only, since its message can
/// hold what a server wrote, and a control character would reach the
/// terminal as a command (set the clipboard or the title, clear the screen).
fn shown(err: &Error) -> String {
    crate::tui::text::drawable(&err.to_string()).into_owned()
}

/// An error as --json prints it on stdout.
fn print_json_error(err: &Error) {
    let v = serde_json::json!({
        "error": err.message(),
        "hint": err.hint(),
        "status": err.kind().exit_code(),
    });
    say(&v.to_string());
}

/// A line on stdout. A reader that has gone (`| head`) is no reason to
/// fail, and must not panic as `println!` does.
fn say(line: &str) {
    let _ = writeln!(std::io::stdout(), "{line}");
}

/// clap's message without its `error: ` prefix and trailing usage block:
/// its first paragraph on one line. A missing argument is named on the
/// lines after the first, which the message must keep.
fn first_paragraph(msg: &str) -> String {
    let para: Vec<&str> = msg
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect();
    let line = para.join(" ");
    line.strip_prefix("error: ").unwrap_or(&line).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage_error(args: &[&str]) -> String {
        let e = Cli::try_parse_from(args).unwrap_err();
        first_paragraph(&e.to_string())
    }

    // clap names the missing arguments on the lines after the first; the
    // message must keep them, or it tells the user something is missing
    // without saying what.
    #[test]
    fn a_missing_argument_is_named() {
        assert_eq!(
            usage_error(&["bsky", "post"]),
            "the following required arguments were not provided: <TEXT>"
        );
        assert_eq!(
            usage_error(&["bsky", "report", "x"]),
            "the following required arguments were not provided: --reason <REASON>"
        );
        assert_eq!(
            usage_error(&["bsky", "chat", "a", "b", "c"]),
            "unexpected argument 'c' found"
        );
    }

    // An error can carry what a server wrote (its message for a failed
    // call): it reaches the terminal as text, whatever escape sequence the
    // server put in it, with the hint still on its own line.
    #[test]
    fn a_server_message_in_an_error_is_printed_as_text() {
        let err = Error::new(
            Kind::Api,
            "app.bsky.feed.getTimeline failed: bad\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J",
        )
        .with_hint("try again");
        let shown = shown(&err);
        assert!(
            !shown.chars().any(|c| c.is_control() && c != '\n'),
            "{shown:?}"
        );
        assert_eq!(
            shown,
            "error: app.bsky.feed.getTimeline failed: bad]52;c;cHduZWQ=[2J\nhint: try again"
        );
    }
}
