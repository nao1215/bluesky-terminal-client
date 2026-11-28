//! The full-screen client `mstdn` opens without a command, built as bsky's
//! and msky's: terminal setup and teardown, and the event loop that feeds
//! keys and answers to the state machine ([`app`]) and draws it ([`view`]).
//!
//! It reads one server: the one `--host` names, else that of the account
//! in use, as the commands do; as that account, or as a visitor.

pub(crate) mod app;
pub(crate) mod columns;
pub(crate) mod keys;
pub(crate) mod list;
pub(crate) mod theme;
pub(crate) mod view;
pub(crate) mod worker;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::event::{EnableBracketedPaste, Event as TermEvent, KeyEventKind};
use crossterm::execute;
use terminal_kit::screen::{Term, init_terminal, restore_terminal, write_now};

use crate::api::Client;
use crate::config::{AccountStore, Session};
use crate::error::{Error, Kind, Result};
use app::App;
use terminal_kit::media::images::{CACHE_BYTES, DiskCache, Images};
use worker::Worker;

/// How long the loop waits for a key before checking the worker again.
const TICK: Duration = Duration::from_millis(50);
/// How long it waits while answers are on their way.
const ANSWER_TICK: Duration = Duration::from_millis(5);
/// Most input events handled before the screen is drawn again: keys that
/// come faster than a frame (a held j) are applied together.
const EVENTS_PER_FRAME: usize = 64;
/// Longest q waits for writes still on their way, as bsky's and msky's: a
/// server that never answers does not keep mstdn open.
const QUIT_WAIT: Duration = Duration::from_secs(3);

const RESET_HINT: &str = "run `reset` to put the terminal back";

/// Fail unless stdin and stdout are both terminals.
fn ensure_interactive() -> Result<()> {
    if terminal_kit::terminal::is_interactive() {
        Ok(())
    } else {
        Err(
            Error::new(Kind::Terminal, "mstdn needs an interactive terminal")
                .with_hint("run mstdn directly in a terminal, not through a pipe or redirect"),
        )
    }
}

/// Run the client until the user quits, on the server at `origin`, as
/// `session` when there is one. `accounts` keeps the settings, when there
/// is a config directory.
pub(crate) fn run(
    accounts: Option<&AccountStore>,
    session: Option<Session>,
    origin: &str,
) -> Result<()> {
    ensure_interactive()?;
    // The first pages are asked for before the terminal is set up: the
    // server's answers need nothing from it.
    let client = Client::new(origin).with_token(session.as_ref().map(|s| s.token.clone()));
    let mut worker = Worker::spawn(client)
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot start the client: {e}")))?;
    let (mut app, jobs) = App::new(session, origin);
    for (seq, job) in jobs {
        worker.send(seq, job);
    }
    if let Some(store) = accounts {
        match store.list() {
            Ok(list) => app.accounts = list,
            Err(e) => app.fail(e.message().to_string()),
        }
    }
    let depth = theme::color_depth(|k| std::env::var(k).ok());
    match accounts.map(AccountStore::settings) {
        Some(Ok(s)) => app.apply_settings(Some(s), depth),
        Some(Err(e)) => {
            // A broken file is not lost: it is not written over this run.
            app.apply_settings(None, depth);
            app.fail(e.message().to_string());
        }
        None => app.apply_settings(None, depth),
    }
    for (seq, job) in app.load_deck() {
        worker.send(seq, job);
    }
    let default_hook = std::panic::take_hook();
    let setup_failed = |e: io::Error| {
        restored(Err(Error::new(
            Kind::Terminal,
            format!("cannot set up the terminal: {e}"),
        )))
    };
    let mut term = match init_terminal() {
        Ok(term) => term,
        Err(e) => return setup_failed(e),
    };
    // The terminal is put back when the thread drawing panics, and only
    // then: another thread's panic is caught where its work runs.
    let drawing = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() == drawing {
            if let Err(why) = restore_terminal() {
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "stderr is the only place left to say it, and a panic is already on its way out"
                )]
                let _: io::Result<()> = writeln!(
                    io::stderr(),
                    "error: cannot restore the terminal: {why}\nhint: {RESET_HINT}"
                );
            }
            default_hook(info);
        }
    }));
    // A paste comes as one piece, not as keys: a pasted `q` would quit.
    if let Err(e) = execute!(io::stdout(), EnableBracketedPaste) {
        return setup_failed(e);
    }
    let forced = std::env::var("MSTDN_GRAPHICS")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let protocol = forced
        .as_deref()
        .and_then(terminal_kit::terminal::parse_protocol);
    if forced.is_some() && protocol.is_none() {
        return restored(Err(Error::usage(
            "MSTDN_GRAPHICS must be kitty, sixel, or iterm2",
        )));
    }
    let mut images = match terminal_kit::terminal::detect_graphics(protocol) {
        Some(p) => {
            let dir = match std::env::var_os("MSTDN_CACHE_DIR") {
                Some(v) if v == "off" => None,
                Some(v) if !v.is_empty() => Some(std::path::PathBuf::from(v)),
                _ => dirs::cache_dir().map(|p| p.join("mstdn")),
            };
            match Images::new(
                p,
                dir.map(|p| DiskCache::new(p.join("images"), CACHE_BYTES)),
            ) {
                Ok(images) => images,
                Err(e) => {
                    app.fail(format!("cannot show pictures: {e}"));
                    Images::none()
                }
            }
        }
        None => Images::none(),
    };
    let result = event_loop(&mut term, &mut app, &mut worker, accounts, &mut images);
    restored(result)
}

/// `result`, once the terminal is put back.
fn restored(result: Result<()>) -> Result<()> {
    with_restore(result, restore_terminal())
}

/// `result`, and what putting the terminal back said after it.
fn with_restore(result: Result<()>, restore: std::result::Result<(), String>) -> Result<()> {
    let Err(why) = restore else {
        return result;
    };
    let not_restored = format!("cannot restore the terminal: {why}");
    Err(match result {
        Ok(()) => Error::new(Kind::Terminal, not_restored).with_hint(RESET_HINT),
        Err(e) => Error::new(e.kind(), format!("{}; {not_restored}", e.message()))
            .with_hint(e.hint().unwrap_or(RESET_HINT)),
    })
}

fn event_loop(
    term: &mut Term,
    app: &mut App,
    worker: &mut Worker,
    accounts: Option<&AccountStore>,
    images: &mut Images,
) -> Result<()> {
    let io_err = |e: io::Error| Error::new(Kind::Terminal, format!("terminal I/O failed: {e}"));
    let mut input = terminal_kit::events::Input::new().map_err(io_err)?;
    let mut dirty = true;
    while !app.quit {
        if dirty {
            let viewer = matches!(app.overlay, Some(app::Overlay::Viewer { .. }));
            if !viewer {
                images.stop_video();
            }
            if let Some(delete) = images.viewer_closed(viewer) {
                write_now(delete).map_err(io_err)?;
                term.clear().map_err(io_err)?;
            }
            term.draw(|f| view::draw_with_images(f, app, images))
                .map_err(io_err)?;
            dirty = false;
        }
        let mut wait = if app.pending() > 0 || images.loading() || images.playing() {
            ANSWER_TICK
        } else {
            TICK
        };
        for _ in 0..EVENTS_PER_FRAME {
            if app.quit {
                break;
            }
            let Some(ev) = input.next(wait).map_err(io_err)? else {
                break;
            };
            match ev {
                TermEvent::Key(k) if k.kind != KeyEventKind::Release => {
                    for (seq, job) in app.handle_key(k) {
                        worker.send(seq, job);
                    }
                }
                TermEvent::Paste(text) => app.handle_paste(&text),
                _ => {}
            }
            if let Some(session) = app.take_switch() {
                let result = accounts
                    .ok_or_else(|| Error::io("no config directory to keep the account in"))
                    .and_then(|store| store.set_current(session.host()));
                match result {
                    Ok(()) => {
                        worker.set_client(
                            Client::new(&session.service).with_token(Some(session.token.clone())),
                        );
                        for (seq, job) in app.switched(session) {
                            worker.send(seq, job);
                        }
                        images.stop_video();
                        if let Some(delete) = images.delete_all() {
                            write_now(delete).map_err(io_err)?;
                        }
                        term.clear().map_err(io_err)?;
                    }
                    Err(e) => app.fail(e.message().to_string()),
                }
            }
            dirty = true;
            wait = Duration::ZERO;
        }
        if let Some(text) = app.take_copy() {
            write_now(&terminal_kit::clipboard::osc52(&text)).map_err(io_err)?;
        }
        if let Some(settings) = app.take_settings_save() {
            let saved = match accounts {
                Some(store) => store.save_settings(&settings),
                None => Ok(()),
            };
            app.settings_saved(saved);
            dirty = true;
        }
        while let Some((seq, ev)) = worker.try_recv() {
            for (seq, job) in app.handle_answer(seq, ev) {
                worker.send(seq, job);
            }
            dirty = true;
        }
        for (seq, job) in app.poll_chat(Instant::now()) {
            worker.send(seq, job);
        }
        for (seq, job) in app.poll_read_ahead(Instant::now()) {
            worker.send(seq, job);
        }
        if images.poll() {
            dirty = true;
        }
        if app.expire_message(Instant::now()) {
            dirty = true;
        }
    }
    // A favourite, a boost or a post sent just before q reaches the server
    // before mstdn ends.
    worker.finish_writes(QUIT_WAIT);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_terminal_not_put_back_is_an_error_even_after_a_clean_quit() {
        assert_eq!(with_restore(Ok(()), Ok(())), Ok(()));
        let err = with_restore(Ok(()), Err("leaving raw mode: gone".into())).unwrap_err();
        assert_eq!(err.kind(), Kind::Terminal);
        assert_eq!(err.kind().exit_code(), 2);
        let first = Error::new(Kind::Io, "cannot read x").with_hint("fix x");
        let err = with_restore(Err(first), Err("leaving raw mode: gone".into())).unwrap_err();
        assert_eq!(err.kind(), Kind::Io);
        assert_eq!(
            err.to_string(),
            "error: cannot read x; cannot restore the terminal: leaving raw mode: gone\nhint: fix x"
        );
    }
}
