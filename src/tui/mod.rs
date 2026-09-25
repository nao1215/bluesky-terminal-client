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

/// `thread::spawn` with a name, so a panic message and a profiler say which
/// of bsky's threads it was.
pub fn spawn<T: Send + 'static>(
    name: &str,
    f: impl FnOnce() -> T + Send + 'static,
) -> std::thread::JoinHandle<T> {
    std::thread::Builder::new()
        .name(name.to_string())
        .spawn(f)
        .expect("a thread starts")
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
    let worker = Worker::spawn(session.clone(), accounts.clone());
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
    let mut term = init_terminal()
        .map_err(|e| Error::new(Kind::Terminal, format!("cannot set up the terminal: {e}")))?;
    let drawing = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() == drawing {
            let _ = execute!(io::stdout(), DisableBracketedPaste);
            ratatui::restore();
            default_hook(info);
        }
        // Another thread's is caught where its work is run, and that work
        // fails with an error the screen shows; written over the screen,
        // it would only break it.
    }));
    let picker = match terminal::detect_graphics() {
        Ok(p) => p,
        Err(e) => {
            ratatui::restore();
            return Err(e);
        }
    };
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let result = event_loop(
        &mut term, picker, &mut app, &worker, accounts, settings, depth, service,
    );
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

/// The terminal bsky draws on: a frame goes out whole.
type Term = ratatui::Terminal<ratatui::backend::CrosstermBackend<FrameWriter>>;

/// Raw mode and the alternate screen, as `ratatui::try_init` sets them up,
/// with the frames written through a [`FrameWriter`].
fn init_terminal() -> io::Result<Term> {
    crossterm::terminal::enable_raw_mode()?;
    if let Err(e) = execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen) {
        let _ = crossterm::terminal::disable_raw_mode();
        return Err(e);
    }
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

#[allow(clippy::too_many_arguments)]
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
    } else if !images.shows() {
        app.without_pictures();
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
                let _ = accounts.set_current(&session.did);
                worker.use_account(session.clone());
                jobs.extend(app.switched_to(session));
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
    use std::io::Write;

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
