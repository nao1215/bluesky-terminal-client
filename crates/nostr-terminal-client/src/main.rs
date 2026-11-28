//! nstr: Nostr commands and a full-screen client.

mod accounts;
mod api;
mod commands;
mod config;
mod error;
mod i18n;
mod output;
mod read_state;
mod tui;

use crate::error::{Error, Result};
use clap::Parser;
use std::{
    io::{self, Write},
    process::ExitCode,
};
#[cfg(test)]
use {api::event::Event, config::Config, serde_json::Value};

#[derive(Parser)]
#[command(
    name = "nstr",
    version,
    about,
    after_help = "Relays and publicKey come from config.json in NSTR_CONFIG_DIR when non-empty,\notherwise from nstr/config.json in the platform config directory.\nPosting additionally needs keyFile or a bunker URI.\nWithout a command, open the full-screen client.\nExit status: 0 success, 1 usage, 2 terminal, 3 local file, 4 network or relay, 5 unavailable."
)]
struct Cli {
    /// Print JSON lines in the shared client format, including errors.
    #[arg(long, global = true)]
    json: bool,
    /// Use a saved account.
    #[arg(short, long, global = true)]
    account: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

use commands::Command;

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
        return tui::run(cli.account.as_deref());
    };
    let (values, warnings) = commands::run(command, cli.account.as_deref())?;
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
            Err(Error::usage(terminal_kit::output::first_paragraph(
                &message,
            )))
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            diagnostics(&e.to_string());
            if json_requested {
                match terminal_kit::output::write_json_line(
                    &mut io::stdout().lock(),
                    &terminal_kit::output::error_json(
                        &serde_json::Value::from(e.error.as_str()).to_string(),
                        e.hint
                            .as_deref()
                            .map(|hint| serde_json::Value::from(hint).to_string())
                            .as_deref(),
                        e.status,
                    ),
                ) {
                    Ok(_) => (),
                    Err(error) if error.kind() == io::ErrorKind::BrokenPipe => (),
                    Err(error) => diagnostics(
                        &Error::file(format!("cannot write output: {error}")).to_string(),
                    ),
                }
            }
            ExitCode::from(e.status)
        }
    }
}

#[cfg(test)]
mod tests;
