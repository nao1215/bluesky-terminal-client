//! nstr: Nostr commands and a full-screen client.

mod api;
mod config;
mod error;
mod output;
mod tui;

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
    after_help = "Relays and publicKey come from config.json in NSTR_CONFIG_DIR when non-empty,\notherwise from nstr/config.json in the platform config directory.\nPosting additionally needs keyFile or a bunker URI.\nWithout a command, open the full-screen client.\nExit status: 0 success, 1 usage, 2 terminal, 3 local file, 4 network or relay, 5 unsupported feature."
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
    /// Search a hashtag using NIP-01, or text using advertised NIP-50 support.
    Search {
        query: String,
        /// Maximum number of matching notes to print.
        #[arg(short = 'n', long, default_value = "20")]
        limit: NonZeroUsize,
    },
    /// Sign and publish a public text note to the configured relays.
    Post { text: String },
    /// Read mentions, replies, reactions, reposts and contact lists tagging the account.
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
    let Some(command) = cli.command else {
        return tui::run();
    };
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
        Command::Search { query, limit } => {
            if query.trim().is_empty() || query == "#" {
                return Err(Error::usage("search query must not be empty"));
            }
            let mut filter = api::relay::Filter {
                kinds: vec![1],
                ..Default::default()
            };
            if let Some(tag) = query.strip_prefix('#') {
                filter.hashtags.push(tag.into());
            } else {
                filter.search = Some(query);
            }
            let found = client.relays().query(filter, limit.get())?;
            (
                found
                    .value
                    .iter()
                    .map(output::post)
                    .collect::<Result<_>>()?,
                found.warnings,
            )
        }
        Command::Post { text } => {
            let now = u64::try_from(jiff::Timestamp::now().as_second()).map_err(Error::usage)?;
            let key = if config.bunker.is_none() {
                Some(config.signing_key()?)
            } else {
                None
            };
            let event = if let Some(bunker) = &config.bunker {
                bunker.sign_note(&config.public, now, &text, config.timeout)?
            } else {
                Event::sign(
                    key.as_ref()
                        .ok_or_else(|| Error::usage("signing key unavailable"))?,
                    now,
                    1,
                    Vec::new(),
                    text,
                )?
            };
            let published = client.relays().publish(&event, key.as_ref())?;
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
