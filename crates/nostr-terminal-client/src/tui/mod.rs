//! Full-screen Nostr client; protocol jobs never run on the drawing thread.
mod app;
mod extras;
mod keys;
mod login;
mod note;
mod profile;
mod search;
mod settings;
mod state;
mod view;
mod worker;

use crate::{
    config::{Config, config_dir},
    error::{Error, Result},
};
use app::{App, Overlay};
use crossterm::{
    event::{EnableBracketedPaste, Event, KeyEventKind},
    execute,
};
use std::{io, time::Duration};
use terminal_kit::{events::Input, media::images::Images, screen, terminal, theme};
use worker::{Answer, Job, Worker};

fn terminal_error(message: impl ToString) -> Error {
    Error {
        error: message.to_string(),
        hint: Some(
            "run nstr directly in a terminal; run `reset` if the terminal needs restoring".into(),
        ),
        status: 2,
    }
}

pub(crate) fn run(account: Option<&str>) -> Result<()> {
    if !terminal::is_interactive() {
        return Err(terminal_error("nstr needs an interactive terminal"));
    }
    if !crate::accounts::path(account)?.exists() && !login::configure(account)? {
        return Ok(());
    }
    let config = Config::load_account(account)?;
    let path = config_dir()?.join("settings.json");
    let preferences = settings::Settings::load(&path);
    let mut app = App::new(
        config.public.clone(),
        config.relays.len(),
        preferences.as_ref().cloned().unwrap_or_default(),
        theme::color_depth(|key| std::env::var(key).ok()),
    );
    if let Err(e) = preferences {
        app.writable_settings = false;
        app.message = e.error;
    }
    app.extra.accounts = crate::accounts::list()?;
    state::load(&mut app)?;
    let mut worker = Worker::new(config);
    dispatch(&mut app, &worker, true);
    // Catch drawing panics long enough to restore raw mode and the alternate screen.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        let mut term = screen::init_terminal().map_err(terminal_error)?;
        execute!(io::stdout(), EnableBracketedPaste).map_err(terminal_error)?;
        let mut images = match terminal::detect_graphics(
            std::env::var("NSTR_GRAPHICS")
                .ok()
                .as_deref()
                .and_then(terminal::parse_protocol),
        ) {
            Some(picker) => Images::new(
                picker,
                Some(terminal_kit::media::images::DiskCache::new(
                    config_dir()?.join("picture-cache"),
                    256 * 1024 * 1024,
                )),
            )
            .map_err(terminal_error)?,
            None => Images::none(),
        };
        let mut input = Input::new().map_err(terminal_error)?;
        let mut dirty = true;
        while !app.quit {
            while let Ok(answer) = worker.rx.try_recv() {
                app.answer(answer);
                dirty = true;
            }
            dirty |= images.poll();
            let jobs = app.background();
            send_jobs(&mut app, &worker, jobs);
            let jobs = app.prefetch();
            send_jobs(&mut app, &worker, jobs);
            if let Some(name) = app.extra.switch.take() {
                match Config::load_account(Some(&name)) {
                    Ok(config) => {
                        state::save(&app)?;
                        app = App::new(
                            config.public.clone(),
                            config.relays.len(),
                            app.settings.clone(),
                            app.depth,
                        );
                        app.extra.accounts = crate::accounts::list()?;
                        state::load(&mut app)?;
                        worker = Worker::new(config);
                        dispatch(&mut app, &worker, true);
                        dirty = true;
                    }
                    Err(e) => {
                        app.message = e.error;
                        dirty = true;
                    }
                }
            }
            if std::mem::take(&mut app.extra.persist) {
                state::save(&app)?;
            }
            if dirty {
                if let Some(delete) =
                    images.viewer_closed(matches!(app.overlay, Some(Overlay::Pictures { .. })))
                {
                    screen::write_now(delete).map_err(terminal_error)?;
                    term.clear().map_err(terminal_error)?;
                }
                term.draw(|frame| view::draw(frame, &app, &mut images))
                    .map_err(terminal_error)?;
                dirty = false;
            }
            if let Some(event) = input
                .next(Duration::from_millis(30))
                .map_err(terminal_error)?
            {
                let jobs = match event {
                    Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                    Event::Paste(text) => {
                        app.paste(&text);
                        vec![]
                    }
                    _ => vec![],
                };
                send_jobs(&mut app, &worker, jobs);
                dirty = true;
            }
            if let Some(text) = app.copy.take() {
                screen::write_now(&terminal_kit::clipboard::osc52(&text))
                    .map_err(terminal_error)?;
            }
            if std::mem::take(&mut app.save)
                && let Err(e) = app.settings.save(&path)
            {
                app.message = e.error;
                dirty = true;
            }
        }
        state::save(&app)?;
        worker.writes.wait(Duration::from_secs(3));
        Ok(())
    }));
    let restored = screen::restore_terminal();
    match result {
        Ok(result) => match (result, restored) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), Ok(())) => Err(e),
            (result, Err(why)) => Err(terminal_error(format!(
                "{}cannot restore terminal: {why}",
                result.err().map(|e| format!("{e}; ")).unwrap_or_default()
            ))),
        },
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn dispatch(app: &mut App, worker: &Worker, refresh: bool) {
    let jobs = app.load(refresh);
    send_jobs(app, worker, jobs);
}

fn send_jobs(app: &mut App, worker: &Worker, jobs: Vec<Job>) {
    for job in jobs {
        let (seq, writing) = match &job {
            Job::Load { seq, .. } => (*seq, false),
            Job::Write { seq, .. } => (*seq, true),
            Job::Task { seq, .. } | Job::Prefetch { seq, .. } => (*seq, true),
        };
        let task = matches!(job, Job::Task { .. });
        let prefetch = matches!(job, Job::Prefetch { .. });
        if let Err(e) = worker.send(job) {
            if task {
                app.answer(Answer::Task(seq, Err(e)));
                continue;
            }
            if prefetch {
                app.answer(Answer::Prefetched(seq, Err(e)));
                continue;
            }
            app.answer(if writing {
                Answer::Written(seq, Err(e))
            } else {
                Answer::Loaded(seq, Err(e))
            });
        }
    }
}
