//! The interactive client: terminal setup, the event loop, and teardown.

pub(crate) mod app;
pub(crate) mod chat;
pub(crate) mod columns;
pub(crate) mod files;
pub(crate) mod images;
pub(crate) mod keys;
pub(crate) mod player;
pub(crate) mod theme;
pub(crate) mod thread;
pub(crate) mod view;
pub(crate) mod worker;

// The drawing helpers that know nothing of Bluesky, shared with other
// terminal clients; at these paths so the rest of bsky reads them as before.
pub(crate) use terminal_kit::{clipboard, events, input, text};

use terminal_kit::screen::{Term, init_terminal, restore_terminal, write_now};

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{EnableBracketedPaste, Event as TermEvent, KeyEventKind};
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
pub(crate) fn spawn<T: Send + 'static>(
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
/// Longest q waits for writes still on their way: a server that never
/// answers does not keep bsky open.
const QUIT_WAIT: Duration = Duration::from_secs(3);

/// Run the client until the user quits.
/// `session` is the account to start as, if any; `accounts` holds every
/// logged-in one.
pub(crate) fn run(
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
    let (mut images, images_failed) = if off {
        (Images::none(), None)
    } else {
        without_threads(make_images(
            &picker,
            crate::config::cache_dir(&env, &loaded).0,
            picture_server(service, app.session.as_ref()),
        ))
    };
    for s in accounts.list().unwrap_or_default() {
        app.accounts.push((&s).into());
    }
    app.kitty_graphics = picker
        .as_ref()
        .is_some_and(|p| p.protocol_type() == ratatui_image::picker::ProtocolType::Kitty);
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
    let video_ahead = player::AheadSession;
    let mut dirty = true;
    while !app.quit {
        // Update before drawing: a player started by this frame adopts the
        // same in-flight read instead of issuing three duplicate requests.
        let videos = app.videos_near_selection(app.inline_video() && images.shows());
        player::read_ahead(&videos);
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
                let (made, failed) = without_threads(make_images(
                    &picker,
                    app.cache_dir(),
                    picture_server(service, app.session.as_ref()),
                ));
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
    drop(video_ahead);
    // A write sent just before q, such as a like or the notifications
    // marked seen, reaches the server before bsky ends.
    worker.finish_writes(QUIT_WAIT);
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

fn make_images(
    picker: &Option<ratatui_image::picker::Picker>,
    cache: Option<PathBuf>,
    cdn: Option<&str>,
) -> io::Result<Images> {
    let Some(picker) = picker else {
        return Ok(Images::none());
    };
    let cache = cache.map(|d| DiskCache::new(d.join("images"), images::CACHE_BYTES));
    let mut images = Images::with_loader(picker.clone(), cache, |p| {
        crate::media::load(p)
            .map(|(image, _)| image)
            .map_err(|e| e.message().to_string())
    })?;
    images.set_player(player::start);
    if let Some(cdn) = cdn {
        images.connect(cdn);
    }
    Ok(images)
}

/// Bluesky's picture server, to connect to at the start, for a service
/// reached over the internet. A local one (a test's stand-in server) gets
/// its pictures elsewhere, and a run against it must not reach out at all.
fn picture_server(service: &str, session: Option<&Session>) -> Option<&'static str> {
    // The CLI service is only the login default. Warming its CDN while a
    // saved account uses another service can occupy every picture loader,
    // leaving even that account's local thumbnails waiting for the CDN.
    let service = session.map_or(service, |s| s.service.as_str());
    let host = service.strip_prefix("https://")?;
    let host = host.split(['/', ':']).next().unwrap_or("");
    let local = host == "localhost" || host.starts_with("127.") || host.starts_with('[');
    (!local && !host.is_empty()).then_some("https://cdn.bsky.app/")
}

#[cfg(test)]
mod tests {
    use super::{account_changes, with_restore, without_threads};
    use super::{make_images, picture_server};
    use crate::config::{AccountStore, Session};
    use crate::error::{Error, Kind};
    use crate::tui::app::App;
    use crate::tui::worker::Worker;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::io;

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

    #[test]
    fn a_saved_local_service_does_not_let_cdn_warmups_starve_thumbnails() {
        use std::sync::mpsc;
        use std::time::Duration;
        use terminal_kit::test_support::http::{Response, reply, serve_http};

        let (heads_tx, heads_rx) = mpsc::channel();
        let (picture_tx, picture_rx) = mpsc::channel();
        let server = serve_http(move |request, _| {
            if request.method == "HEAD" {
                // Hold every warm-up until the test ends, including on
                // assertion failure. No sleep decides which request wins.
                let (release, wait) = mpsc::channel::<()>();
                heads_tx.send(release).unwrap();
                let _released = wait.recv();
                return Some(reply(200, ""));
            }
            assert_eq!(request.target, "/thumbnail.png");
            picture_tx.send(()).unwrap();
            Some(Response {
                status: 200,
                headers: vec![("Content-Type", "image/png".into())],
                body: include_bytes!("../../e2e/atago/testdata/avatar-alice.png").to_vec(),
            })
        })
        .unwrap();
        let mut saved = session("did:plc:me", "me.test");
        saved.service = server.url().into();
        let default_service = "https://bsky.social";
        let (app, _) = App::new(Some(saved), default_service);
        let cdn = picture_server(default_service, app.session.as_ref());
        // Redirect any wrongly selected public CDN to our blocked mock:
        // the regression must never contact a real server.
        let mut images = make_images(
            &Some(ratatui_image::picker::Picker::halfblocks()),
            None,
            cdn.map(|_| server.url()),
        )
        .unwrap();
        let _held: Vec<_> = if cdn.is_some() {
            // Images::connect warms all eight loaders. Force that ordering
            // before asking for the poster, as a slow CDN did on CI.
            (0..8)
                .map(|_| heads_rx.recv_timeout(Duration::from_secs(5)).unwrap())
                .collect()
        } else {
            Vec::new()
        };
        images.prefetch(&format!("{}/thumbnail.png", server.url()), 36, 11);
        picture_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("the saved account's thumbnail must not wait for unrelated CDN warm-ups");
        assert!(server.calls().iter().all(|r| r.method == "GET"));
    }

    #[test]
    fn the_picture_server_is_warmed_only_for_a_service_on_the_internet() {
        assert_eq!(
            picture_server("https://bsky.social", None),
            Some("https://cdn.bsky.app/")
        );
        assert_eq!(
            picture_server("https://pds.example.com:2583", None),
            Some("https://cdn.bsky.app/")
        );
        assert_eq!(picture_server("http://127.0.0.1:4000", None), None);
        assert_eq!(picture_server("https://127.0.0.1:4000", None), None);
        assert_eq!(picture_server("https://localhost", None), None);
        assert_eq!(picture_server("https://[::1]:8080", None), None);
        let mut saved = session("did:plc:me", "me.test");
        for service in [
            "http://127.0.0.1:4000",
            "https://localhost",
            "https://[::1]:8080",
        ] {
            saved.service = service.into();
            assert_eq!(picture_server("https://bsky.social", Some(&saved)), None);
        }
        saved.service = "https://bsky.social".into();
        assert_eq!(
            picture_server("http://127.0.0.1:4000", Some(&saved)),
            Some("https://cdn.bsky.app/")
        );
    }
}
