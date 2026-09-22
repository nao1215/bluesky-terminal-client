//! The interactive client: terminal setup, the event loop, and teardown.

pub mod app;
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
use std::time::Duration;

use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event as TermEvent, KeyEventKind,
};
use crossterm::execute;

use crate::config::{SessionStore, SettingsStore};
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
pub fn run(store: SessionStore, settings: SettingsStore, service: &str) -> Result<()> {
    terminal::ensure_interactive()?;
    // Read the session before touching the terminal so a broken file is
    // reported on a normal screen.
    let session = store.load()?;
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
    let result = event_loop(&mut term, picker, session, store, settings, depth, service);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn event_loop(
    term: &mut ratatui::DefaultTerminal,
    picker: ratatui_image::picker::Picker,
    session: Option<crate::config::Session>,
    store: SessionStore,
    settings: SettingsStore,
    depth: theme::ColorDepth,
    service: &str,
) -> Result<()> {
    let cache =
        crate::config::cache_dir().map(|d| DiskCache::new(d.join("images"), images::CACHE_BYTES));
    let mut images = Images::new(picker, cache);
    let worker = Worker::spawn(session.clone(), store);
    let (mut app, jobs) = App::new(session, service);
    let (loaded, warning) = settings.load();
    app.apply_settings(loaded, depth, warning);
    jobs.into_iter().for_each(|j| worker.send(j));

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
                    app.handle_key(k).into_iter().for_each(|j| worker.send(j));
                }
                TermEvent::Paste(text) => app.handle_paste(&text),
                _ => {}
            }
            dirty = true;
            // Only what is already waiting; the next frame is not held back.
            wait = Duration::ZERO;
        }
        // Saved here rather than on the worker, which runs jobs in order: a
        // theme applied just before q must not wait behind a network call.
        if let Some(s) = app.take_settings_save() {
            app.settings_saved(settings.save(&s));
            dirty = true;
        }
        while let Some(ev) = worker.try_recv() {
            app.handle_event(ev)
                .into_iter()
                .for_each(|j| worker.send(j));
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
