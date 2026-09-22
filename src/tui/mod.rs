//! The interactive client: terminal setup, the event loop, and teardown.

pub mod app;
pub mod images;
pub mod input;
pub mod text;
pub mod view;
pub mod worker;

use std::io;
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event as TermEvent, KeyEventKind,
};
use crossterm::execute;

use crate::config::SessionStore;
use crate::error::{Error, Kind, Result};
use crate::terminal;
use app::App;
use images::Images;
use worker::Worker;

/// How long the loop waits for a key before checking the worker again.
const TICK: Duration = Duration::from_millis(50);

/// Run the client until the user quits.
pub fn run(store: SessionStore, service: &str) -> Result<()> {
    terminal::ensure_interactive()?;
    // Read the session before touching the terminal so a broken file is
    // reported on a normal screen.
    let session = store.load()?;

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
    let result = event_loop(&mut term, picker, session, store, service);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn event_loop(
    term: &mut ratatui::DefaultTerminal,
    picker: ratatui_image::picker::Picker,
    session: Option<crate::config::Session>,
    store: SessionStore,
    service: &str,
) -> Result<()> {
    let mut images = Images::new(picker);
    let worker = Worker::spawn(session.clone(), store);
    let (mut app, jobs) = App::new(session, service);
    jobs.into_iter().for_each(|j| worker.send(j));

    let io_err = |e: io::Error| Error::new(Kind::Terminal, format!("terminal I/O failed: {e}"));
    let mut dirty = true;
    while !app.quit {
        if dirty {
            term.draw(|f| view::draw(f, &mut app, &mut images))
                .map_err(io_err)?;
            dirty = false;
        }
        if event::poll(TICK).map_err(io_err)? {
            match event::read().map_err(io_err)? {
                TermEvent::Key(k) if k.kind != KeyEventKind::Release => {
                    app.handle_key(k).into_iter().for_each(|j| worker.send(j));
                }
                TermEvent::Paste(text) => app.handle_paste(&text),
                _ => {}
            }
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
    }
    Ok(())
}
