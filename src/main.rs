//! msky: a Misskey client for the terminal.
#![cfg_attr(
    test,
    allow(
        clippy::let_underscore_must_use,
        reason = "the stand-in servers of the tests answer best effort; a test fails on what it asserts"
    )
)]

mod api;
mod cli;
mod clock;
mod download;
mod error;
mod text;

use std::io::Write;
use std::process::ExitCode;

use clap::{CommandFactory, Parser};

use crate::error::{Error, Kind, Result};

/// Command line: the commands read a Misskey server as a visitor does.
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "msky", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// Misskey server to read: a host such as misskey.io, or its URL.
    #[arg(
        long,
        env = "MSKY_HOST",
        value_name = "HOST",
        default_value = api::DEFAULT_HOST,
        global = true
    )]
    host: String,

    /// Print JSON: one note per line, in the same shape for every command,
    /// and errors as {"error": ...} on stdout too.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

const AFTER_HELP: &str = "\
msky reads what a Misskey server shows to visitors; it does not log in.
A note is its ID or its address (https://misskey.io/notes/ID); an account
is name, @name, @name@host, or its address (https://misskey.io/@name). An
address is read on its own server, whatever --host says.

Exit status: 0 success, 1 usage error, 3 local file error, 4 network or
server error.";

fn run(cli: Cli) -> Result<()> {
    let origin = api::normalize_host(&cli.host)?;
    let Some(cmd) = cli.command else {
        // Nothing to do yet without a command: the full-screen client is
        // still to come, so the help says what there is.
        let help = Cli::command().render_help().to_string();
        let mut out = std::io::stdout().lock();
        return cli::ended_by_reader(
            out.write_all(help.as_bytes())
                .map_err(|e| Error::io(format!("cannot write the output: {e}"))),
        );
    };
    let ctx = cli::Ctx {
        origin,
        json: cli.json,
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
            usage_error(&["msky", "author"]),
            "the following required arguments were not provided: <USER>"
        );
        assert_eq!(
            usage_error(&["msky", "note", "a", "b"]),
            "unexpected argument 'b' found"
        );
    }

    // Misskey refuses withReplies and withFiles together; msky says so
    // before anything is asked.
    #[test]
    fn replies_and_files_only_cannot_go_together() {
        assert!(
            usage_error(&["msky", "author", "a", "--replies", "--with-files"])
                .contains("cannot be used with"),
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
