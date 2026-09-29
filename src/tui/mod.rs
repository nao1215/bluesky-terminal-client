//! The interactive client: terminal setup, the event loop, and teardown.

pub mod app;
pub mod chat;
pub mod clipboard;
pub mod columns;
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

use std::io::{self, Write};
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

/// `thread::spawn` with a name, so a panic message and a profiler say which
/// of bsky's threads it was. Unlike `thread::spawn` it returns the failure
/// of a system out of threads rather than panicking with it: a panic here
/// on the UI thread took the whole client down.
pub fn spawn<T: Send + 'static>(
    name: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> io::Result<std::thread::JoinHandle<T>> {
    std::thread::Builder::new().name(name.to_string()).spawn(f)
}

/// How long the loop waits for a key before checking the worker again.
const TICK: Duration = Duration::from_millis(50);
/// How long the loop waits while a video plays.
const VIDEO_TICK: Duration = Duration::from_millis(5);
/// How long the loop waits for a key while answers or pictures are on
/// their way, which are shown as soon as they come.
const ANSWER_TICK: Duration = Duration::from_millis(5);
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
    // The first lists are asked for before the terminal is: its answer
    // takes a round trip (over ssh, as long as the network's), and the
    // server's answers need nothing from it.
    let worker = Worker::spawn(session.clone(), accounts.clone())
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot start the client: {e}")))?;
    let (mut app, jobs) = App::new(session, service);
    for job in jobs {
        worker.send(app.stamp(&job), job);
    }

    // The panic hook ratatui sets restores the terminal whichever thread
    // panicked, a caught one included (a picture the encoder chokes on):
    // the client would go on drawing outside the screen it set up. Ours
    // restores it, bracketed paste too, only when the thread drawing
    // panics.
    let default_hook = std::panic::take_hook();
    let setup_failed = |e: io::Error| {
        restored(Err(Error::new(
            Kind::Terminal,
            format!("cannot set up the terminal: {e}"),
        )))
    };
    let mut term = match init_terminal() {
        Ok(term) => term,
        // Raw mode may be on already, or the alternate screen.
        Err(e) => return setup_failed(e),
    };
    let drawing = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() == drawing {
            if let Err(why) = restore_terminal() {
                // Before the panic's own message, which follows.
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
        // Another thread's is caught where its work is run, and that work
        // fails with an error the screen shows; written over the screen,
        // it would only break it.
    }));
    let picker = match terminal::detect_graphics() {
        Ok(p) => p,
        Err(e) => return restored(Err(e)),
    };
    if let Err(e) = execute!(io::stdout(), EnableBracketedPaste) {
        return setup_failed(e);
    }
    let result = event_loop(
        &mut term, picker, &mut app, &worker, accounts, settings, depth, service,
    );
    restored(result)
}

/// What to do about a terminal bsky could not put back.
const RESET_HINT: &str = "run `reset` to put the terminal back";

/// `result`, once the terminal is put back ([`restore_terminal`]). A
/// failure to put it back is reported with it: after a clean quit as an
/// error of its own, after an error in the message of that error.
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

/// Put the terminal back as bsky found it: bracketed paste off, raw mode
/// off, the main screen back. See [`restore_steps`].
fn restore_terminal() -> std::result::Result<(), String> {
    restore_steps(&mut [
        ("turning bracketed paste off", &mut || {
            execute!(io::stdout(), DisableBracketedPaste)
        }),
        (
            "leaving raw mode",
            &mut crossterm::terminal::disable_raw_mode,
        ),
        ("leaving the alternate screen", &mut || {
            execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)
        }),
    ])
}

/// Take every step, even after one fails: ratatui's `restore` stopped at a
/// failed step, and one that failed to leave raw mode left the alternate
/// screen up too. The failures, each named by its step, are returned
/// together.
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

/// The terminal bsky draws on: a frame goes out whole.
type Term = ratatui::Terminal<ratatui::backend::CrosstermBackend<FrameWriter>>;

/// Raw mode and the alternate screen, as `ratatui::try_init` sets them up,
/// with the frames written through a [`FrameWriter`]. On a failure the
/// caller puts back what was set up.
fn init_terminal() -> io::Result<Term> {
    crossterm::terminal::enable_raw_mode()?;
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(FrameWriter::new(
        io::stdout(),
    )))
}

/// Holds what a frame writes until the frame is flushed, then hands it to
/// the terminal in one write, marked as one synchronized update (mode 2026)
/// so a terminal that knows the mode shows the frame only once it is whole;
/// one that does not ignores the marks. Through stdout's own buffer a
/// frame went out 1 KB at a time, and the terminal could show the screen
/// half drawn: the old posts below the new ones while scrolling.
pub struct FrameWriter<W: io::Write = io::Stdout> {
    out: W,
    /// [`BEGIN_UPDATE`], then the frame written so far.
    frame: Vec<u8>,
}

/// Starts and ends a synchronized update.
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

#[expect(
    clippy::too_many_arguments,
    reason = "the loop owns what the client runs with; a struct for them would only be unpacked here"
)]
fn event_loop(
    term: &mut Term,
    picker: Option<ratatui_image::picker::Picker>,
    app: &mut App,
    worker: &Worker,
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
        let Some(picker) = picker else {
            return Ok(Images::none());
        };
        let cache = cache.map(|d| DiskCache::new(d.join("images"), images::CACHE_BYTES));
        let images = Images::new(picker.clone(), cache)?;
        if let Some(cdn) = picture_server(service) {
            images.connect(cdn);
        }
        Ok::<_, io::Error>(images)
    };
    let (mut images, images_failed) = if off {
        (Images::none(), None)
    } else {
        without_threads(make_images(
            &picker,
            crate::config::cache_dir(&env, &loaded).0,
        ))
    };
    for s in accounts.list().unwrap_or_default() {
        app.accounts.push((&s).into());
    }
    app.env = env;
    app.apply_settings(loaded, depth, warning);
    // The columns kept for the account, if any, are what the Timeline tab
    // shows from the start.
    for job in app.show_timeline() {
        worker.send(app.stamp(&job), job);
    }
    if off {
        app.pictures = false;
    } else if let Some(why) = images_failed {
        app.pictures_failed(&why);
    } else if !images.shows() {
        app.without_pictures();
    }

    let io_err = |e: io::Error| Error::new(Kind::Terminal, format!("terminal I/O failed: {e}"));
    let mut input = events::Input::new().map_err(io_err)?;
    let mut dirty = true;
    while !app.quit {
        if dirty {
            if let Some(delete) = images.viewer_closed(app.viewer_open()) {
                write_now(delete).map_err(io_err)?;
            }
            term.draw(|f| view::draw(f, app, &mut images))
                .map_err(io_err)?;
            dirty = false;
        }
        // A playing video's pictures come every 67 ms; waiting a whole tick
        // for keys would show them late and unevenly. An answer or a picture
        // on its way is shown as it comes, not at the end of the tick.
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
            for job in account_changes(app, worker, &accounts) {
                worker.send(app.stamp(&job), job);
            }
            dirty = true;
            // Only what is already waiting; the next frame is not held back.
            wait = Duration::ZERO;
        }
        // The clipboard belongs to the terminal, which the loop owns: the
        // state machine only says what to put in it.
        if let Some(text) = app.take_copy() {
            write_now(&clipboard::osc52(&text)).map_err(io_err)?;
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
                write_now(delete).map_err(io_err)?;
            }
            images = Images::none();
            if on {
                let (made, failed) = without_threads(make_images(&picker, app.cache_dir()));
                images = made;
                match failed {
                    Some(why) => app.pictures_failed(&why),
                    None => app.pictures_back(images.shows()),
                }
            }
            term.clear().map_err(io_err)?;
            dirty = true;
        }
        while let Some((seq, ev)) = worker.try_recv() {
            // An account logged in is used from the jobs the app sends on
            // taking the answer, not before.
            if let worker::Event::LoggedIn(Ok(session)) = &ev {
                worker.use_account(session.clone());
            }
            for job in app.handle_answer(seq, ev) {
                worker.send(app.stamp(&job), job);
            }
            dirty = true;
        }
        for url in app.take_pictures_ahead() {
            images.warm(&url);
        }
        // Played only where pictures are shown.
        for playlist in app.take_videos_ahead() {
            if images.shows() {
                player::read_ahead(&playlist);
            }
        }
        if images.poll() {
            dirty = true;
        }
        if app.expire_status(std::time::Instant::now()) {
            dirty = true;
        }
        // The thread of the post the selection rests on is read ahead; the
        // screen does not change for it.
        for job in app.poll_read_ahead(std::time::Instant::now()) {
            worker.send(app.stamp(&job), job);
        }
        // The Chat tab reads the server again now and then while it is shown.
        for job in app.poll_chat(std::time::Instant::now()) {
            worker.send(app.stamp(&job), job);
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
                // Used for this run even when the next cannot be told: the
                // user asked for it, and sees why it will not last.
                let kept = accounts.set_current(&session.did);
                worker.use_account(session.clone());
                jobs.extend(app.switched_to(session));
                if let Err(e) = kept {
                    app.account_error(e.message().to_string());
                }
            }
            Ok(None) => app.account_error(crate::i18n::tf("{} is not logged in any more", &[&did])),
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

/// The pictures made, or, when their threads could not be started, none
/// and why, for the status line: the client goes on without them.
fn without_threads(made: io::Result<Images>) -> (Images, Option<String>) {
    match made {
        Ok(images) => (images, None),
        Err(e) => (Images::none(), Some(e.to_string())),
    }
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
    use super::{FrameWriter, picture_server};
    use super::{account_changes, restore_steps, with_restore, without_threads};
    use crate::config::{AccountStore, Session};
    use crate::error::{Error, Kind};
    use crate::tui::app::App;
    use crate::tui::worker::Worker;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::io::{self, Write};

    // ratatui's restore stopped at the first step that failed: raw mode
    // that could not be left kept the alternate screen up too, and the
    // failure went unreported. Every step is taken, and each failure named.
    #[test]
    fn every_restore_step_runs_after_one_fails_and_each_failure_is_named() {
        let mut ran = Vec::new();
        let result = {
            let ran = std::cell::RefCell::new(&mut ran);
            restore_steps(&mut [
                ("turning bracketed paste off", &mut || {
                    ran.borrow_mut().push("paste");
                    Err(io::Error::other("stdout is closed"))
                }),
                ("leaving raw mode", &mut || {
                    ran.borrow_mut().push("raw");
                    Ok(())
                }),
                ("leaving the alternate screen", &mut || {
                    ran.borrow_mut().push("screen");
                    Err(io::Error::other("stdout is closed"))
                }),
            ])
        };
        assert_eq!(ran, ["paste", "raw", "screen"]);
        assert_eq!(
            result.unwrap_err(),
            "turning bracketed paste off: stdout is closed; leaving the alternate screen: stdout is closed"
        );
        assert_eq!(restore_steps(&mut [("x", &mut || Ok(()))]), Ok(()));
    }

    #[test]
    fn a_terminal_not_put_back_is_an_error_even_after_a_clean_quit() {
        assert_eq!(with_restore(Ok(()), Ok(())), Ok(()));
        let err = with_restore(Ok(()), Err("leaving raw mode: gone".into())).unwrap_err();
        assert_eq!(err.kind(), Kind::Terminal);
        assert_eq!(
            err.to_string(),
            "error: cannot restore the terminal: leaving raw mode: gone\nhint: run `reset` to put the terminal back"
        );
        // An error the client stopped with keeps its class and hint.
        let first = Error::new(Kind::Io, "cannot read x").with_hint("fix x");
        let err = with_restore(Err(first), Err("leaving raw mode: gone".into())).unwrap_err();
        assert_eq!(err.kind(), Kind::Io);
        assert_eq!(
            err.to_string(),
            "error: cannot read x; cannot restore the terminal: leaving raw mode: gone\nhint: fix x"
        );
    }

    fn session(did: &str, handle: &str) -> Session {
        Session {
            service: "http://127.0.0.1:9".into(),
            did: did.into(),
            handle: handle.into(),
            access_jwt: "a".into(),
            refresh_jwt: "r".into(),
        }
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    // The account chosen is kept in accounts.json for the next start; one
    // that could not be written was dropped silently, and the next start
    // was as the other account.
    #[test]
    fn a_switch_that_cannot_be_kept_for_the_next_start_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = AccountStore::open(dir.path());
        let (home, work) = (
            session("did:plc:home", "home.test"),
            session("did:plc:work", "work.test"),
        );
        accounts.save(&work).unwrap();
        accounts.save(&home).unwrap();
        let (mut app, _) = App::new(Some(home.clone()), &home.service);
        for s in accounts.list().unwrap() {
            app.accounts.push((&s).into());
        }
        let kept = dir.path().join("accounts.json");
        std::fs::remove_file(&kept).unwrap();
        std::fs::create_dir(&kept).unwrap();
        press(&mut app, KeyCode::Char('A'));
        let at = |did: &str| app.accounts.iter().position(|a| a.did == did).unwrap();
        let (from, to) = (at("did:plc:home"), at("did:plc:work"));
        let step = if to > from { 'j' } else { 'k' };
        for _ in 0..from.abs_diff(to) {
            press(&mut app, KeyCode::Char(step));
        }
        press(&mut app, KeyCode::Enter);
        let worker = Worker::spawn(Some(home), accounts.clone()).unwrap();
        account_changes(&mut app, &worker, &accounts);
        assert_eq!(app.session.as_ref().unwrap().did, "did:plc:work");
        let status = app.status.as_ref().expect("a status");
        assert!(status.error, "{}", status.text);
        assert!(status.text.contains("accounts.json"), "{}", status.text);
    }

    // With no thread left for the picture loaders, starting them panicked
    // on the UI thread; the client now runs without pictures and says why.
    #[test]
    fn pictures_whose_threads_cannot_start_leave_the_client_running_without_them() {
        let (images, why) =
            without_threads(Err(io::Error::other("Resource temporarily unavailable")));
        assert!(!images.shows());
        let why = why.unwrap();
        let (mut app, _) = App::new(None, "https://bsky.social");
        app.pictures_failed(&why);
        assert!(!app.pictures);
        let status = app.status.as_ref().unwrap();
        assert!(status.error);
        assert_eq!(
            status.text,
            "cannot show pictures: Resource temporarily unavailable"
        );
    }

    /// Each write it is given, as the terminal would receive it.
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
        for part in ["\x1b[1;1H", "家族👨‍👩‍👧", &"x".repeat(5000)] {
            w.write_all(part.as_bytes()).unwrap();
        }
        assert!(
            writes.0.borrow().is_empty(),
            "nothing goes out before the frame ends"
        );
        w.flush().unwrap();
        let want = format!("\x1b[?2026h\x1b[1;1H家族👨‍👩‍👧{}\x1b[?2026l", "x".repeat(5000));
        assert_eq!(*writes.0.borrow(), [want.into_bytes()]);
        // The next frame starts empty; a flush with nothing drawn sends nothing.
        w.flush().unwrap();
        w.write_all(b"b").unwrap();
        w.flush().unwrap();
        assert_eq!(writes.0.borrow().len(), 2);
        assert_eq!(writes.0.borrow()[1], b"\x1b[?2026hb\x1b[?2026l");
    }

    /// Draws a screen of posts through ratatui and counts the writes the
    /// terminal gets.
    #[test]
    fn a_drawn_screen_is_one_write() {
        use ratatui::backend::CrosstermBackend;
        use ratatui::widgets::Paragraph;
        let writes = Writes::default();
        let mut term = ratatui::Terminal::with_options(
            CrosstermBackend::new(FrameWriter::new(writes.clone())),
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 120, 50)),
            },
        )
        .unwrap();
        let text = "今日は山に登りました 🏔️ the view from the top 👨‍👩‍👧\n".repeat(50);
        term.draw(|f| f.render_widget(Paragraph::new(text.as_str()), f.area()))
            .unwrap();
        let writes = writes.0.borrow();
        assert_eq!(writes.len(), 1);
        assert!(writes[0].len() > 4096, "{} bytes", writes[0].len());
    }

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
