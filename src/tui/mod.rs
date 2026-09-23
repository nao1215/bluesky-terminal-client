//! The interactive client: terminal setup, the event loop, and teardown.

pub mod app;
pub mod clipboard;
pub mod events;
pub mod files;
pub mod images;
pub mod input;
pub mod keys;
pub mod player;
pub mod scale;
pub mod text;
pub mod theme;
pub mod thread;
pub mod view;
pub mod worker;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event as TermEvent, KeyEventKind,
};
use crossterm::execute;

use crate::config::{AccountStore, Session, SettingsStore};
use crate::error::{Error, Kind, Result};
use crate::terminal;
use app::App;
use images::{DiskCache, Images};
use worker::Worker;

/// How long the loop waits for a key before checking the worker again.
const TICK: Duration = Duration::from_millis(50);
/// How long the loop waits while a video plays.
const VIDEO_TICK: Duration = Duration::from_millis(5);
/// Most input events handled before the screen is drawn again. Keys that
/// arrive faster than a frame draws (a held j) are applied together, so the
/// selection keeps up with the key instead of trailing behind it.
const EVENTS_PER_FRAME: usize = 64;

/// Run the client until the user quits.
/// `session` is the account to start as, if any; `accounts` holds every
/// logged-in one.
pub fn run(
    accounts: AccountStore,
    session: Option<Session>,
    settings: SettingsStore,
    service: &str,
) -> Result<()> {
    terminal::ensure_interactive()?;
    let depth = theme::color_depth(|k| std::env::var(k).ok());

    let mut term = ratatui::try_init()
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot set up the terminal: {e}")))?;
    let picker = match terminal::detect_graphics() {
        Ok(p) => p,
        Err(e) => {
            ratatui::restore();
            return Err(e);
        }
    };
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let result = event_loop(
        &mut term, picker, session, accounts, settings, depth, service,
    );
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn event_loop(
    term: &mut ratatui::DefaultTerminal,
    picker: Option<ratatui_image::picker::Picker>,
    session: Option<Session>,
    accounts: AccountStore,
    settings: SettingsStore,
    depth: theme::ColorDepth,
    service: &str,
) -> Result<()> {
    let env = crate::config::Environment::read();
    let (loaded, warning) = settings.load();
    // The file turns pictures off unless BSKY_GRAPHICS asks for them.
    let off = loaded.pictures_off() && env.graphics.is_none();
    // The picker is kept even with pictures off, so they can be turned
    // back on without asking the terminal again while keys are read.
    let make_images = |picker: &Option<ratatui_image::picker::Picker>, cache: Option<PathBuf>| {
        let cache = cache.map(|d| DiskCache::new(d.join("images"), images::CACHE_BYTES));
        let images = match picker {
            Some(picker) => Images::new(picker.clone(), cache),
            None => Images::none(),
        };
        if let Some(cdn) = picture_server(service) {
            images.connect(cdn);
        }
        images
    };
    let mut images = if off {
        Images::none()
    } else {
        make_images(&picker, crate::config::cache_dir(&env, &loaded).0)
    };
    let worker = Worker::spawn(session.clone(), accounts.clone());
    let (mut app, jobs) = App::new(session, service);
    for s in accounts.list().unwrap_or_default() {
        app.accounts.push((&s).into());
    }
    app.env = env;
    app.apply_settings(loaded, depth, warning);
    if off {
        app.pictures = false;
    } else if !images.shows() {
        app.without_pictures();
    }
    for job in jobs {
        worker.send(app.stamp(&job), job);
    }

    let io_err = |e: io::Error| Error::new(Kind::Terminal, format!("terminal I/O failed: {e}"));
    let mut input = events::Input::new().map_err(io_err)?;
    let mut dirty = true;
    while !app.quit {
        if dirty {
            if let Some(delete) = images.viewer_closed(app.viewer_open()) {
                let mut out = io::stdout();
                let _ = io::Write::write_all(&mut out, delete.as_bytes());
                let _ = io::Write::flush(&mut out);
            }
            term.draw(|f| view::draw(f, &mut app, &mut images))
                .map_err(io_err)?;
            dirty = false;
        }
        // A playing video's pictures come every 67 ms; waiting a whole tick
        // for keys would show them late and unevenly.
        let mut wait = if images.playing() { VIDEO_TICK } else { TICK };
        for _ in 0..EVENTS_PER_FRAME {
            if app.quit {
                break;
            }
            let Some(ev) = input.next(wait).map_err(io_err)? else {
                break;
            };
            match ev {
                TermEvent::Key(k) if k.kind != KeyEventKind::Release => {
                    for job in app.handle_key(k) {
                        worker.send(app.stamp(&job), job);
                    }
                }
                TermEvent::Paste(text) => app.handle_paste(&text),
                _ => {}
            }
            // Before the next key: the worker acts as the account chosen
            // before anything is asked of it.
            for job in account_changes(&mut app, &worker, &accounts) {
                worker.send(app.stamp(&job), job);
            }
            dirty = true;
            // Only what is already waiting; the next frame is not held back.
            wait = Duration::ZERO;
        }
        // The clipboard belongs to the terminal, which the loop owns: the
        // state machine only says what to put in it.
        if let Some(text) = app.take_copy() {
            let mut out = io::stdout();
            let _ = io::Write::write_all(&mut out, clipboard::osc52(&text).as_bytes());
            let _ = io::Write::flush(&mut out);
        }
        // Saved here rather than on the worker, whose jobs wait for the
        // network: a theme applied just before q must not wait behind one.
        if let Some(s) = app.take_settings_save() {
            app.settings_saved(settings.save(&s));
            dirty = true;
        }
        if let Some(on) = app.take_pictures_change() {
            // The pictures on screen go with the Images that drew them:
            // kitty's copies are deleted, and every cell is drawn again.
            if let Some(delete) = images.delete_all() {
                let mut out = io::stdout();
                let _ = io::Write::write_all(&mut out, delete.as_bytes());
                let _ = io::Write::flush(&mut out);
            }
            images = if on {
                make_images(&picker, app.cache_dir())
            } else {
                Images::none()
            };
            if on {
                app.pictures_back(images.shows());
            }
            term.clear().map_err(io_err)?;
            dirty = true;
        }
        while let Some((seq, ev)) = worker.try_recv() {
            for job in app.handle_answer(seq, ev) {
                worker.send(app.stamp(&job), job);
            }
            dirty = true;
        }
        if images.poll() {
            dirty = true;
        }
        if app.expire_status(std::time::Instant::now()) {
            dirty = true;
        }
    }
    Ok(())
}

/// Switch to, or log out, the account the account list asked for: the
/// session is read from its file (a refresh may have rotated its tokens
/// since it was last used), the worker is given it, and then the app asks
/// for the account's lists, which are returned for the loop to send.
fn account_changes(app: &mut App, worker: &Worker, accounts: &AccountStore) -> Vec<worker::Job> {
    let mut jobs = Vec::new();
    if let Some(did) = app.take_account_switch() {
        match accounts.find(&did) {
            Ok(Some(session)) => {
                let _ = accounts.set_current(&session.did);
                worker.use_account(session.clone());
                jobs.extend(app.switched_to(session));
            }
            Ok(None) => app.account_error(format!("{did} is not logged in any more")),
            Err(e) => app.account_error(e.message().to_string()),
        }
    }
    if let Some(did) = app.take_account_logout() {
        let result = accounts.remove(&did).and_then(|_| accounts.current());
        match result {
            Ok(next) => {
                let was_current = app.session.as_ref().is_some_and(|s| s.did == did);
                if was_current {
                    match &next {
                        Some(s) => worker.use_account(s.clone()),
                        None => worker.use_no_account(),
                    }
                }
                jobs.extend(app.logged_out(&did, next));
            }
            Err(e) => app.account_error(e.message().to_string()),
        }
    }
    jobs
}

/// Bluesky's picture server, to connect to at the start, for a service
/// reached over the internet. A local one (a test's stand-in server) gets
/// its pictures elsewhere, and a run against it must not reach out at all.
fn picture_server(service: &str) -> Option<&'static str> {
    let host = service.strip_prefix("https://")?;
    let host = host.split(['/', ':']).next().unwrap_or("");
    let local = host == "localhost" || host.starts_with("127.") || host.starts_with('[');
    (!local && !host.is_empty()).then_some("https://cdn.bsky.app/")
}

#[cfg(test)]
mod tests {
    use super::picture_server;

    #[test]
    fn the_picture_server_is_warmed_only_for_a_service_on_the_internet() {
        assert_eq!(
            picture_server("https://bsky.social"),
            Some("https://cdn.bsky.app/")
        );
        assert_eq!(
            picture_server("https://pds.example.com:2583"),
            Some("https://cdn.bsky.app/")
        );
        assert_eq!(picture_server("http://127.0.0.1:4000"), None);
        assert_eq!(picture_server("https://127.0.0.1:4000"), None);
        assert_eq!(picture_server("https://localhost"), None);
        assert_eq!(picture_server("https://[::1]:8080"), None);
    }
}
