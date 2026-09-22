//! bs: a Bluesky client for the terminal.

mod api;
mod config;
mod error;
mod media;
mod terminal;
mod timeline;
mod tui;

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::{SessionStore, SettingsStore};
use crate::error::{Error, Kind, Result};

/// Command line: `bs` opens the client, `bs logout` forgets the session.
/// The `--help` text comes from the package description and `AFTER_HELP`.
#[derive(Debug, Parser)]
#[command(name = "bs", version, about, long_about = None, after_help = AFTER_HELP)]
struct Cli {
    /// PDS to log in to.
    #[arg(long, env = "BS_SERVICE", value_name = "URL", default_value = api::DEFAULT_SERVICE)]
    service: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Forget the saved session.
    Logout,
}

const AFTER_HELP: &str = "\
The terminal must display images: kitty graphics, sixel, or iTerm2 inline
images. bs exits with status 2 on a terminal that supports none of them.

Exit status: 0 success, 1 usage error, 2 terminal unsupported,
3 local file error. Network and server errors are shown in the client and
do not end it.";

fn run(cli: Cli) -> Result<()> {
    let service = api::normalize_service(&cli.service)?;
    let dir = config::config_dir()?;
    let store = SessionStore::new(&dir);
    match cli.command {
        Some(Command::Logout) => {
            if store.clear()? {
                println!("logged out");
            } else {
                println!("not logged in");
            }
            Ok(())
        }
        None => tui::run(store, SettingsStore::new(&dir), &service),
    }
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
                .with_hint("run `bs --help` for usage");
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
