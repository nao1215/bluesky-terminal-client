//! The full-screen client `msky` opens without a command, built as bsky's:
//! terminal setup and teardown, and the event loop that feeds keys and
//! answers to the state machine ([`app`]) and draws it ([`view`]).
//!
//! It reads one server at a time: the one `--host` names, else that of the
//! account in use (`accounts.json`), as the commands do. `A` switches to
//! another account logged in, and so to its server.

pub mod app;
pub mod clipboard;
pub mod events;
pub mod files;
pub mod images;
pub mod input;
pub mod keys;
pub mod mp4;
pub mod player;
pub mod scale;
pub mod text;
pub mod theme;
pub mod view;
pub mod worker;

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{Event as TermEvent, KeyEventKind};
use crossterm::execute;

use crate::api::Client;
use crate::config::{AccountStore, Session};
use crate::error::{Error, Kind, Result};
use crate::terminal;
use app::App;
use images::{DiskCache, Images};
use worker::Worker;

/// `thread::spawn` with a name; unlike it, a system out of threads is an
/// error rather than a panic on the UI thread, as in bsky.
pub fn spawn<T: Send + 'static>(
    name: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> io::Result<std::thread::JoinHandle<T>> {
    std::thread::Builder::new().name(name.to_string()).spawn(f)
}

/// How long the loop waits for a key before checking the worker again.
const TICK: Duration = Duration::from_millis(50);
/// How long the loop waits while a video plays, as bsky's.
const VIDEO_TICK: Duration = Duration::from_millis(5);
/// How long it waits while answers or pictures are on their way.
const ANSWER_TICK: Duration = Duration::from_millis(5);
/// Most input events handled before the screen is drawn again: keys that
/// come faster than a frame (a held j) are applied together.
const EVENTS_PER_FRAME: usize = 64;

/// Environment variable naming the folder downloaded pictures are kept
/// in between runs; `off` keeps none.
pub const CACHE_DIR_ENV: &str = "MSKY_CACHE_DIR";

/// The folder pictures are kept in: `MSKY_CACHE_DIR`, else `msky` in the
/// platform cache folder; none when it is `off` or there is none.
fn cache_dir() -> Option<PathBuf> {
    match std::env::var_os(CACHE_DIR_ENV) {
        Some(v) if v == "off" => None,
        Some(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => dirs::cache_dir().map(|d| d.join("msky")),
    }
}

/// A client of `origin`, as `session` when there is one.
fn client_for(origin: &str, session: Option<&Session>) -> Client {
    Client::new(origin).with_token(session.map(|s| s.token.clone()))
}

/// Run the client until the user quits, on the server at `origin`, as
/// `session` when there is one. `accounts` holds the accounts logged in,
/// when there is a config directory to keep them.
pub fn run(accounts: Option<AccountStore>, session: Option<Session>, origin: &str) -> Result<()> {
    terminal::ensure_interactive()?;
    // The first lists are asked for before the terminal is: its answer
    // takes a round trip, and the server's answers need nothing from it.
    let worker = Worker::spawn(client_for(origin, session.as_ref()))
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot start the client: {e}")))?;
    let (mut app, jobs) = App::new(session, origin);
    for (seq, job) in jobs {
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
    let picker = match terminal::detect_graphics() {
        Ok(p) => p,
        Err(e) => return restored(Err(e)),
    };
    let result = event_loop(&mut term, picker, &mut app, &worker, accounts.as_ref());
    restored(result)
}

const RESET_HINT: &str = "run `reset` to put the terminal back";

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

/// Put the terminal back as msky found it, every step taken even after
/// one fails, as bsky does.
fn restore_terminal() -> std::result::Result<(), String> {
    restore_steps(&mut [
        (
            "leaving raw mode",
            &mut crossterm::terminal::disable_raw_mode,
        ),
        ("leaving the alternate screen", &mut || {
            execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)
        }),
    ])
}

fn restore_steps(
    steps: &mut [(&str, &mut dyn FnMut() -> io::Result<()>)],
) -> std::result::Result<(), String> {
    let failed: Vec<String> = steps
        .iter_mut()
        .filter_map(|(what, step)| step().err().map(|e| format!("{what}: {e}")))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

/// Write `sequence` to the terminal now, outside the frame being drawn.
fn write_now(sequence: &str) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(sequence.as_bytes())?;
    out.flush()
}

type Term = ratatui::Terminal<ratatui::backend::CrosstermBackend<FrameWriter>>;

fn init_terminal() -> io::Result<Term> {
    crossterm::terminal::enable_raw_mode()?;
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(FrameWriter::new(
        io::stdout(),
    )))
}

/// Holds what a frame writes until the frame is flushed, then hands it to
/// the terminal in one write, marked as one synchronized update (mode
/// 2026), as bsky's: the terminal never shows a frame half drawn.
pub struct FrameWriter<W: io::Write = io::Stdout> {
    out: W,
    frame: Vec<u8>,
}

const BEGIN_UPDATE: &[u8] = b"\x1b[?2026h";
const END_UPDATE: &[u8] = b"\x1b[?2026l";

impl<W: io::Write> FrameWriter<W> {
    pub fn new(out: W) -> Self {
        let mut frame = Vec::with_capacity(64 * 1024);
        frame.extend_from_slice(BEGIN_UPDATE);
        Self { out, frame }
    }
}

impl<W: io::Write> io::Write for FrameWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.frame.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.frame.len() > BEGIN_UPDATE.len() {
            self.frame.extend_from_slice(END_UPDATE);
            let written = self.out.write_all(&self.frame);
            self.frame.truncate(BEGIN_UPDATE.len());
            written?;
        }
        self.out.flush()
    }
}

fn event_loop(
    term: &mut Term,
    picker: Option<ratatui_image::picker::Picker>,
    app: &mut App,
    worker: &Worker,
    accounts: Option<&AccountStore>,
) -> Result<()> {
    let mut images = match &picker {
        None => Images::none(),
        Some(p) => {
            let cache = cache_dir().map(|d| DiskCache::new(d.join("images"), images::CACHE_BYTES));
            match Images::new(p.clone(), cache) {
                Ok(i) => i,
                Err(e) => {
                    app.account_error(format!("cannot show pictures: {e}"));
                    Images::none()
                }
            }
        }
    };
    app.pictures = images.shows();
    app.theme = theme::for_env(|k| std::env::var(k).ok());
    if let Some(store) = accounts {
        app.accounts = store
            .list()
            .unwrap_or_default()
            .iter()
            .map(app::Account::from)
            .collect();
        // The tabs the settings name, in their order.
        let (settings, broken) = crate::config::load_settings(store.dir());
        let (tabs, unknown) = app::tabs_of(&settings.tabs.unwrap_or_default());
        for (seq, job) in app.set_tabs(tabs) {
            worker.send(seq, job);
        }
        if let Some(why) = broken.or(unknown) {
            app.account_error(why);
        }
    }
    // Without an account at all the client opens on the login form, as
    // bsky's does; esc reads the server as a visitor.
    if app.session.is_none() && app.accounts.is_empty() && accounts.is_some() {
        for (seq, job) in app.open_login() {
            worker.send(seq, job);
        }
    }
    let io_err = |e: io::Error| Error::new(Kind::Terminal, format!("terminal I/O failed: {e}"));
    let mut dirty = true;
    while !app.quit {
        if dirty {
            if let Some(delete) = images.viewer_closed(app.viewer_open()) {
                write_now(delete).map_err(io_err)?;
                // The cells kitty's pictures were placed in no longer say
                // what ratatui last wrote there: drawn on the diff alone,
                // the next frames left text of the screen before the
                // viewer beside the pictures (a line's end, an emoji gone
                // from it). The whole screen is drawn again instead.
                term.clear().map_err(io_err)?;
            }
            term.draw(|f| view::draw(f, app, &mut images))
                .map_err(io_err)?;
            dirty = false;
        }
        let mut wait = if images.playing() {
            VIDEO_TICK
        } else if app.pending > 0 || images.loading() {
            ANSWER_TICK
        } else {
            TICK
        };
        for _ in 0..EVENTS_PER_FRAME {
            if app.quit {
                break;
            }
            let Some(ev) = events::next(wait).map_err(io_err)? else {
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
            if let Some(store) = accounts {
                for (seq, job) in account_changes(app, worker, store) {
                    worker.send(seq, job);
                }
            }
            dirty = true;
            wait = Duration::ZERO;
        }
        if let Some(text) = app.take_copy() {
            write_now(&clipboard::osc52(&text)).map_err(io_err)?;
        }
        while let Some((seq, ev)) = worker.try_recv() {
            for (seq, job) in app.handle_answer(seq, ev) {
                worker.send(seq, job);
            }
            if let Some(store) = accounts {
                for (seq, job) in logged_in(app, worker, store) {
                    worker.send(seq, job);
                }
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

/// Save the login the form just made, and use it.
fn logged_in(app: &mut App, worker: &Worker, accounts: &AccountStore) -> app::Jobs {
    let Some(session) = app.take_logged_in() else {
        return Vec::new();
    };
    // Used for this run even when it cannot be saved: the user allowed it,
    // and sees why it will not last.
    let saved = accounts.save(&session);
    app.accounts = accounts
        .list()
        .unwrap_or_default()
        .iter()
        .map(app::Account::from)
        .collect();
    worker.use_client(client_for(&session.service, Some(&session)));
    let origin = session.service.clone();
    let jobs = app.switched_to(Some(session), &origin);
    if let Err(e) = saved {
        app.account_error(e.message().to_string());
    }
    jobs
}

/// Switch to, or log out, the account the account list asked for: its
/// file is read again, the worker is given its token, and then the app
/// asks for its lists, which are returned for the loop to send.
fn account_changes(app: &mut App, worker: &Worker, accounts: &AccountStore) -> app::Jobs {
    let mut jobs = Vec::new();
    if let Some(host) = app.take_account_switch() {
        match accounts.find(&host) {
            Ok(Some(session)) => {
                let kept = accounts.set_current(&host);
                worker.use_client(client_for(&session.service, Some(&session)));
                let origin = session.service.clone();
                jobs.extend(app.switched_to(Some(session), &origin));
                if let Err(e) = kept {
                    app.account_error(e.message().to_string());
                }
            }
            Ok(None) => app.account_error(format!("{host} is not logged in any more")),
            Err(e) => app.account_error(e.message().to_string()),
        }
    }
    if let Some(host) = app.take_account_logout() {
        match accounts.remove(&host).and_then(|_| accounts.current()) {
            Ok(next) => {
                let was_current = app.session.as_ref().is_some_and(|s| s.host() == host);
                if was_current {
                    let origin = next
                        .as_ref()
                        .map_or_else(|| app.origin.clone(), |s| s.service.clone());
                    worker.use_client(client_for(&origin, next.as_ref()));
                }
                jobs.extend(app.logged_out(&host, next));
            }
            Err(e) => app.account_error(e.message().to_string()),
        }
    }
    jobs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn every_restore_step_runs_after_one_fails_and_each_failure_is_named() {
        let mut ran = Vec::new();
        let result = {
            let ran = std::cell::RefCell::new(&mut ran);
            restore_steps(&mut [
                ("leaving raw mode", &mut || {
                    ran.borrow_mut().push("raw");
                    Err(io::Error::other("stdout is closed"))
                }),
                ("leaving the alternate screen", &mut || {
                    ran.borrow_mut().push("screen");
                    Err(io::Error::other("stdout is closed"))
                }),
            ])
        };
        assert_eq!(ran, ["raw", "screen"]);
        assert_eq!(
            result.unwrap_err(),
            "leaving raw mode: stdout is closed; leaving the alternate screen: stdout is closed"
        );
    }

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

    #[derive(Default, Clone)]
    struct Writes(std::rc::Rc<std::cell::RefCell<Vec<Vec<u8>>>>);

    impl Write for Writes {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().push(buf.to_vec());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_frame_reaches_the_terminal_in_one_write_as_one_update() {
        let writes = Writes::default();
        let mut w = FrameWriter::new(writes.clone());
        for part in ["\x1b[1;1H", "家族👨‍👩‍👧", "x"] {
            w.write_all(part.as_bytes()).unwrap();
        }
        assert!(writes.0.borrow().is_empty());
        w.flush().unwrap();
        assert_eq!(
            *writes.0.borrow(),
            [b"\x1b[?2026h\x1b[1;1H\xe5\xae\xb6\xe6\x97\x8f\xf0\x9f\x91\xa8\xe2\x80\x8d\xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x91\xa7x\x1b[?2026l".to_vec()]
        );
        w.flush().unwrap();
        assert_eq!(writes.0.borrow().len(), 1, "an empty frame sends nothing");
    }

    fn session(service: &str, name: &str) -> Session {
        Session {
            service: service.into(),
            user_id: format!("id-{name}"),
            username: name.into(),
            token: format!("fake-token-for-tests-{name}"),
            permissions: Vec::new(),
        }
    }

    fn press(app: &mut App, code: KeyCode) -> app::Jobs {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    // Enter on another account in the list switches to it and to its
    // server, keeps it as the one in use for the next start, and loads
    // its home timeline; x and y log one out and move on to the next.
    #[test]
    fn the_account_list_switches_and_logs_out_through_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        let (a, b) = (
            session("http://127.0.0.1:9", "alice"),
            session("http://127.0.0.1:8", "bob"),
        );
        store.save(&b).unwrap();
        store.save(&a).unwrap();
        let (mut app, _) = App::new(Some(a.clone()), &a.service);
        app.accounts = store
            .list()
            .unwrap()
            .iter()
            .map(app::Account::from)
            .collect();
        let worker = Worker::spawn(Client::new(&a.service)).unwrap();
        press(&mut app, KeyCode::Char('A'));
        let at = |app: &App, h: &str| app.accounts.iter().position(|x| x.host == h).unwrap();
        let (from, to) = (at(&app, "127.0.0.1:9"), at(&app, "127.0.0.1:8"));
        let step = if to > from { 'j' } else { 'k' };
        for _ in 0..from.abs_diff(to) {
            press(&mut app, KeyCode::Char(step));
        }
        press(&mut app, KeyCode::Enter);
        let jobs = account_changes(&mut app, &worker, &store);
        assert_eq!(app.session.as_ref().unwrap().username, "bob");
        assert_eq!(app.origin, "http://127.0.0.1:8");
        assert_eq!(store.current_host().as_deref(), Some("127.0.0.1:8"));
        assert!(matches!(
            jobs[0].1,
            worker::Job::Timeline {
                kind: crate::cli::TimelineKind::Home,
                until: None
            }
        ));
        // x on bob, the one in use, then y: alice is used again.
        press(&mut app, KeyCode::Char('A'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Char('y'));
        account_changes(&mut app, &worker, &store);
        assert_eq!(app.session.as_ref().unwrap().username, "alice");
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(app.accounts.len(), 1);
    }

    // A switch that cannot be kept for the next start says so, and is used
    // for this run all the same.
    #[test]
    fn a_switch_that_cannot_be_kept_for_the_next_start_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        let (a, b) = (
            session("http://127.0.0.1:9", "alice"),
            session("http://127.0.0.1:8", "bob"),
        );
        store.save(&b).unwrap();
        store.save(&a).unwrap();
        let kept = dir.path().join("accounts.json");
        std::fs::remove_file(&kept).unwrap();
        std::fs::create_dir(&kept).unwrap();
        let (mut app, _) = App::new(Some(a.clone()), &a.service);
        app.accounts = store
            .list()
            .unwrap()
            .iter()
            .map(app::Account::from)
            .collect();
        let worker = Worker::spawn(Client::new(&a.service)).unwrap();
        let bob = app
            .accounts
            .iter()
            .position(|x| x.username == "bob")
            .unwrap();
        app.overlay = Some(app::Overlay::Accounts { selected: bob });
        press(&mut app, KeyCode::Enter);
        account_changes(&mut app, &worker, &store);
        assert_eq!(app.session.as_ref().unwrap().username, "bob");
        let status = app.status.as_ref().unwrap();
        assert!(status.error, "{}", status.text);
        assert!(status.text.contains("accounts.json"), "{}", status.text);
    }
}
