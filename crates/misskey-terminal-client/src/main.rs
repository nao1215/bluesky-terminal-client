//! msky: a Misskey client for the terminal.
#![cfg_attr(
    test,
    allow(
        clippy::let_underscore_must_use,
        reason = "the stand-in servers of the tests answer best effort; a test fails on what it asserts"
    )
)]

mod api;
mod browser;
mod cli;
mod config;
mod download;
mod error;
mod i18n;
// The keys every client prints with --json (doc/json.md), which the tests
// check this client's output against. Each test uses some of the lists.
#[cfg(test)]
#[allow(
    dead_code,
    reason = "each client's tests read only the lists of what it prints"
)]
#[path = "../../../doc/json_keys.rs"]
mod json_keys;
mod metadata;
mod mfm;
mod miauth;
#[cfg(test)]
mod properties;
mod servers;
mod terminal;
mod text;
mod tui;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
// Times as msky shows them, terminal-kit's as bsky's are.
use terminal_kit::clock;

use crate::error::{Error, Kind, Result};

/// Command line: the commands read a Misskey server, as a visitor or as
/// the account `msky login` saved for it.
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "msky", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// Misskey server: a host such as misskey.io, or its URL [default: the
    /// server of the account in use, else misskey.io]
    #[arg(long, env = "MSKY_HOST", value_name = "HOST", global = true)]
    host: Option<String>,

    /// Print JSON: one note per line, in the same shape for every command,
    /// and errors as {"error": ...} on stdout too.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

const AFTER_HELP: &str = "\
Without a command msky opens the client: the timelines, the threads and
the notifications, with pictures drawn with kitty graphics, sixel, or
iTerm2 inline images (? lists its keys). The commands read and write from
scripts.

msky reads what a Misskey server shows to visitors, or, after `msky login`
(or with a token in MSKY_TOKEN), what it shows to your account there, and
posts and follows as that account. A note is its
ID or its address (https://misskey.io/notes/ID); an account is name, @name,
@name@host, or its address (https://misskey.io/@name). An address is read
on its own server, whatever --host says.

Exit status: 0 success, 1 usage error, 2 not an interactive terminal,
3 local file error, 4 network, authentication, permission or server error,
5 this server does not have this feature. The full-screen client shows
errors and keeps running.";

fn run(cli: Cli) -> Result<()> {
    let accounts = config::config_dir().map(config::AccountStore::open);
    // A broken login file is what logout takes away, so it does not stop it.
    let logout = matches!(cli.command, Some(cli::Command::Logout { .. }));
    let env_token = config::env_token();
    let origin = match cli.host.as_deref() {
        Some(h) => api::normalize_host(h)?,
        // A token from the environment is not of the account in use: it
        // is of the server --host names, else of misskey.io.
        None if env_token.is_some() => api::normalize_host(api::DEFAULT_HOST)?,
        // The account in use names the server when nothing else does.
        None => match &accounts {
            Ok(a) => match a.current() {
                Ok(s) => s.map(|s| s.service),
                Err(_) if logout => None,
                Err(e) => return Err(e),
            },
            Err(_) => None,
        }
        .map_or_else(|| api::normalize_host(api::DEFAULT_HOST), Ok)?,
    };
    let ctx = cli::Ctx {
        origin,
        json: cli.json,
        host_given: cli.host.is_some(),
        accounts,
        env_token,
    };
    let Some(cmd) = cli.command else {
        // Without a command msky opens the client, as bsky does.
        let session = ctx.account_in_use()?;
        return tui::run(ctx.accounts.ok(), session, &ctx.origin);
    };
    cli::run(cmd, &ctx)
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                // Help cut short by its reader (`| head`) is fine; help that
                // could not be written (a full disk) is not.
                return match e.print() {
                    Err(err) if err.kind() != std::io::ErrorKind::BrokenPipe => {
                        let err = Error::io(format!("cannot write the output: {err}"));
                        report(&err);
                        ExitCode::from(err.kind().exit_code())
                    }
                    _ => ExitCode::SUCCESS,
                };
            }
            let err = Error::new(Kind::Usage, first_paragraph(&e.to_string()))
                .with_hint("run `msky --help` for usage");
            report(&err);
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
            report(&err);
            let status = err.kind().exit_code();
            if json {
                print_json_error(&err);
            }
            ExitCode::from(status)
        }
    }
}

/// Print `err` on stderr.
fn report(err: &Error) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "stderr is where errors go; when it cannot be written, the exit status is all that is left, and it is returned"
    )]
    let _: std::io::Result<()> = writeln!(std::io::stderr(), "{}", shown(err));
}

/// An error as it is printed on stderr: as text only, since its message can
/// hold what a server wrote, and a control character would reach the
/// terminal as a command (set the clipboard or the title, clear the screen).
fn shown(err: &Error) -> String {
    crate::text::drawable(&err.to_string()).into_owned()
}

/// An error as --json prints it on stdout.
fn print_json_error(err: &Error) {
    let v = serde_json::json!({
        "error": err.message(),
        "hint": err.hint(),
        "status": err.kind().exit_code(),
    });
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the error is on stderr already and the exit status says it; a reader gone from stdout (`| head`) must not make it panic as `println!` would"
    )]
    let _: std::io::Result<()> = writeln!(std::io::stdout(), "{v}");
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
    use clap::CommandFactory;

    fn usage_error(args: &[&str]) -> String {
        let e = Cli::try_parse_from(args).unwrap_err();
        first_paragraph(&e.to_string())
    }

    // -n asks for how many: none is a mistake said before anything is
    // asked, not an empty answer.
    #[test]
    fn a_limit_of_none_is_a_usage_error() {
        assert_eq!(
            usage_error(&["msky", "timeline", "-n", "0"]),
            "invalid value '0' for '--limit <LIMIT>': it is how many to list, so 1 or more"
        );
        assert!(Cli::try_parse_from(["msky", "timeline", "-n", "1"]).is_ok());
    }

    // clap names the missing arguments on the lines after the first; the
    // message must keep them, or it tells the user something is missing
    // without saying what.
    #[test]
    fn a_missing_argument_is_named() {
        assert_eq!(
            usage_error(&["msky", "author"]),
            "the following required arguments were not provided: <ACTOR>"
        );
        assert_eq!(
            usage_error(&["msky", "note", "a", "b"]),
            "unexpected argument 'b' found"
        );
    }

    // A filter is one of the names bsky's --filter takes, or its lexicon
    // name; anything else is a usage error before anything is asked.
    #[test]
    fn a_filter_is_one_bsky_takes() {
        for f in [
            "with-replies",
            "no-replies",
            "media",
            "video",
            "threads",
            "posts_no_replies",
        ] {
            assert!(
                Cli::try_parse_from(["msky", "author", "a", "--filter", f]).is_ok(),
                "{f}"
            );
        }
        assert!(
            usage_error(&["msky", "author", "a", "--filter", "replies"])
                .contains("invalid value 'replies' for '--filter <FILTER>'"),
        );
    }

    // An error can carry what a server wrote: it reaches the terminal as
    // text, whatever escape sequence the server put in it, with the hint
    // still on its own line.
    #[test]
    fn a_server_message_in_an_error_is_printed_as_text() {
        let err = Error::api("notes/show failed: bad\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J")
            .with_hint("try again");
        let shown = shown(&err);
        assert!(
            !shown.chars().any(|c| c.is_control() && c != '\n'),
            "{shown:?}"
        );
        assert_eq!(
            shown,
            "error: notes/show failed: bad]52;c;cHduZWQ=[2J\nhint: try again"
        );
    }

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }
}
