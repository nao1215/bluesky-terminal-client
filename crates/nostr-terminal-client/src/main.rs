//! nstr: synchronous Nostr API commands without a full-screen interface.

mod api;
mod config;
mod error;
mod output;

use crate::{
    api::{Client, event::Event},
    config::Config,
    error::{Error, Result},
};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::{
    io::{self, Write},
    num::NonZeroUsize,
    process::ExitCode,
};

#[derive(Parser)]
#[command(
    name = "nstr",
    version,
    about,
    after_help = "Relays and publicKey come from config.json in NSTR_CONFIG_DIR when non-empty,\notherwise from nstr/config.json in the platform config directory.\nPosting additionally needs keyFile, placed by its owner.\nExit status: 0 success, 1 usage, 3 local file, 4 network or relay."
)]
struct Cli {
    /// Print JSON lines in the shared client format, including errors.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Read the configured public key's profile metadata.
    Whoami,
    /// Read notes by the configured account and its kind-3 contacts, newest first.
    Timeline {
        /// Maximum number of notes to print; must be at least 1.
        #[arg(short = 'n', long, default_value = "20")]
        limit: NonZeroUsize,
    },
    /// Sign and publish a public text note to the configured relays.
    Post { text: String },
    /// Read kind-1 mentions and replies tagging the configured public key.
    Notifications {
        /// Maximum number of notifications to print; must be at least 1.
        #[arg(short = 'n', long, default_value = "20")]
        limit: NonZeroUsize,
    },
}

fn diagnostics(message: &str) {
    if writeln!(
        io::stderr().lock(),
        "{}",
        terminal_kit::text::drawable(message)
    )
    .is_err()
    {
        // The process exit code remains available when stderr is closed.
    }
}

fn run(cli: Cli) -> Result<()> {
    let command = cli
        .command
        .ok_or_else(|| Error::usage("no command given"))?;
    if let Command::Post { text } = &command
        && text.trim().is_empty()
    {
        return Err(Error::usage("post text must not be empty"));
    }
    let config = Config::load()?;
    let client = Client { config: &config };
    let (values, warnings): (Vec<Value>, _) = match command {
        Command::Whoami => {
            let profile = client.latest(0)?;
            (
                vec![output::account(&config.public, profile.value.as_ref())?],
                profile.warnings,
            )
        }
        Command::Timeline { limit } | Command::Notifications { limit } => {
            let notifications = matches!(command, Command::Notifications { .. });
            let notes = if notifications {
                client.notifications(limit.get())?
            } else {
                client.timeline(limit.get())?
            };
            let format = if notifications {
                output::notification
            } else {
                output::post
            };
            (
                notes.value.iter().map(format).collect::<Result<_>>()?,
                notes.warnings,
            )
        }
        Command::Post { text } => {
            let key = config.signing_key()?;
            let now = u64::try_from(jiff::Timestamp::now().as_second()).map_err(Error::usage)?;
            let event = Event::sign(&key, now, 1, Vec::new(), text);
            drop(key);
            let event = event?;
            let published = client.relays().publish(&event)?;
            (
                vec![output::publication(&event, published.value)?],
                published.warnings,
            )
        }
    };
    for warning in warnings {
        diagnostics(&format!("warning: {warning}"));
    }
    let mut out = io::stdout().lock();
    for value in values {
        if !output::emit(&mut out, &value, cli.json)? {
            break;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let json_requested = std::env::args_os().any(|a| a == "--json");
    let result = match Cli::try_parse() {
        Ok(cli) => run(cli),
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            match e.print() {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
                Err(e) => Err(Error::file(e)),
            }
        }
        Err(e) => {
            let message = e.to_string();
            let first = message
                .lines()
                .take_while(|line| !line.trim().is_empty())
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            Err(Error::usage(
                first.strip_prefix("error: ").unwrap_or(&first),
            ))
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            diagnostics(&e.to_string());
            if json_requested {
                match serde_json::to_value(&e)
                    .map_err(Error::file)
                    .and_then(|v| output::emit(&mut io::stdout().lock(), &v, true))
                {
                    Ok(_) => (),
                    Err(output_error) => diagnostics(&output_error.to_string()),
                }
            }
            ExitCode::from(e.status)
        }
    }
}

#[cfg(test)]
mod tests;
