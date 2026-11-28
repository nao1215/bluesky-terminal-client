//! mstdn: a Mastodon client for the terminal: commands, and without one a
//! full-screen client.

#![cfg_attr(
    test,
    allow(
        clippy::let_underscore_must_use,
        reason = "the stand-in servers of the tests answer best effort; a test fails on what it asserts"
    )
)]

use terminal_kit::output::first_paragraph;

mod api;
mod browser;
mod cli;
mod config;
mod download;
mod error;
mod html;
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
mod oauth;
#[cfg(test)]
mod properties;
mod text;
mod tui;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

use crate::error::{Error, Kind, Result};

/// Command line: the commands read a Mastodon server, as a visitor or as
/// the account `mstdn login` saved for it.
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "mstdn", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// Mastodon server: a host such as mastodon.social, or its URL
    /// [default: the server of the account in use, else mastodon.social]
    #[arg(long, env = "MSTDN_HOST", value_name = "HOST", global = true)]
    host: Option<String>,

    /// Print JSON: one post per line, in the same shape for every command,
    /// and errors as {"error": ...} on stdout too.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

const AFTER_HELP: &str = "\
Without a command, mstdn opens a full-screen client: the home, local and
federated timelines and the notifications, with ? listing its keys.

mstdn reads what a Mastodon server shows to visitors, or, after `mstdn
login` (or with a token in MSTDN_TOKEN), what it shows to your account
there, and posts as that account. A post is its ID or its address
(https://mastodon.social/@name/ID); an account is name, @name,
@name@host, or its address (https://mastodon.social/@name). An address is
read on its own server, whatever --host says.

Exit status: 0 success, 1 usage error, 2 terminal error, 3 local file
error, 4 network or server error, 5 unsupported server feature.";

fn run(cli: Cli) -> Result<()> {
    let accounts = config::config_dir().map(config::AccountStore::open);
    // A broken login file is what logout takes away, so it does not stop it.
    let logout = matches!(cli.command, Some(cli::Command::Logout { .. }));
    let env_token = config::env_token();
    let origin = match cli.host.as_deref() {
        Some(h) => api::normalize_host(h)?,
        // A token from the environment is not of the account in use: it
        // is of the server --host names, else of mastodon.social.
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
        // Without a command, the full-screen client.
        let session = ctx.session(&ctx.origin)?;
        return tui::run(ctx.accounts.as_ref().ok(), session, &ctx.origin);
    };
    cli::run(cmd, &ctx)
}

fn main() -> ExitCode {
    i18n::set(i18n::from_locale(|k| std::env::var(k).ok()));
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
                .with_hint("run `mstdn --help` for usage");
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
    let v = terminal_kit::output::error_json(
        &serde_json::Value::from(err.message()).to_string(),
        err.hint()
            .map(|hint| serde_json::Value::from(hint).to_string())
            .as_deref(),
        err.kind().exit_code(),
    );
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the error is on stderr already and the exit status says it; a reader gone from stdout (`| head`) must not make it panic as `println!` would"
    )]
    let _: std::io::Result<()> = terminal_kit::output::write_json_line(&mut std::io::stdout(), &v);
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
            usage_error(&["mstdn", "timeline", "-n", "0"]),
            "invalid value '0' for '--limit <LIMIT>': it is how many to list, so 1 or more"
        );
        assert!(Cli::try_parse_from(["mstdn", "timeline", "-n", "1"]).is_ok());
    }

    // clap names the missing arguments on the lines after the first; the
    // message must keep them, or it tells the user something is missing
    // without saying what.
    #[test]
    fn a_missing_argument_is_named() {
        assert_eq!(
            usage_error(&["mstdn", "author"]),
            "the following required arguments were not provided: <ACTOR>"
        );
        assert_eq!(
            usage_error(&["mstdn", "status", "a", "b"]),
            "unexpected argument 'b' found"
        );
    }

    // A filter is one of the names bsky's --filter takes, or its lexicon
    // name; a visibility Mastodon's name or msky's; a timeline Mastodon's
    // name or msky's global.
    #[test]
    fn the_names_are_bskys_and_mskys() {
        for f in [
            "with-replies",
            "no-replies",
            "media",
            "video",
            "threads",
            "posts_no_replies",
        ] {
            assert!(
                Cli::try_parse_from(["mstdn", "author", "a", "--filter", f]).is_ok(),
                "{f}"
            );
        }
        for v in [
            "public",
            "unlisted",
            "home",
            "private",
            "followers",
            "direct",
        ] {
            assert!(
                Cli::try_parse_from(["mstdn", "post", "x", "--visibility", v]).is_ok(),
                "{v}"
            );
        }
        for k in ["home", "local", "public", "federated", "global"] {
            assert!(Cli::try_parse_from(["mstdn", "timeline", k]).is_ok(), "{k}");
        }
        assert!(Cli::try_parse_from(["mstdn", "note", "1"]).is_ok());
        assert!(
            usage_error(&["mstdn", "author", "a", "--filter", "replies"])
                .contains("invalid value 'replies' for '--filter <FILTER>'"),
        );
    }

    // An error can carry what a server wrote: it reaches the terminal as
    // text, whatever escape sequence the server put in it, with the hint
    // still on its own line.
    #[test]
    fn a_server_message_in_an_error_is_printed_as_text() {
        let err = Error::api("v1/statuses/1 failed: bad\u{1b}]52;c;cHduZWQ=\u{7}\u{1b}[2J")
            .with_hint("try again");
        let shown = shown(&err);
        assert!(
            !shown.chars().any(|c| c.is_control() && c != '\n'),
            "{shown:?}"
        );
        assert_eq!(
            shown,
            "error: v1/statuses/1 failed: bad]52;c;cHduZWQ=[2J\nhint: try again"
        );
    }

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }
}
