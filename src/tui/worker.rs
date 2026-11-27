//! Network calls run on worker threads so the UI never blocks on the
//! server, as bsky's do.
//!
//! The UI sends a [`Job`] with its number; a worker performs it with the
//! [`Client`] in use when it was sent and answers with an [`Event`] under
//! the same number. Every job of this version reads (or saves a file), so
//! they run side by side on a few threads, and a thread opened meanwhile
//! does not wait behind a slow page.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use crate::api::Client;
use crate::api::types::{DriveFile, Note, Notification};
use crate::cli::{TimelineKind, Visibility};
use crate::config::Session;
use crate::error::{Error, Result};
use crate::servers::Server;
use crate::{download, miauth};
use serde_json::json;

/// Notes, or notifications, asked for a page at a time.
pub const PAGE: usize = 30;

/// Replies asked for under a note, and under each of the first
/// [`NESTED`] of them.
const REPLIES: usize = 30;
const NESTED: usize = 10;

/// Work for the worker threads.
#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    /// A page of a timeline: the first, or the one older than `until`.
    Timeline {
        kind: TimelineKind,
        until: Option<String>,
    },
    /// A page of the notifications, as a timeline's.
    Notifications { until: Option<String> },
    /// The thread of note `id`: what it answers, it, and its replies.
    Thread { id: String },
    /// Save files of notes in the download folder, each with the note it
    /// is attached to and its place there, from 1, as `msky download`
    /// names them.
    Download {
        files: Vec<(String, usize, DriveFile)>,
    },
    /// Open an address in the web browser.
    OpenLink { url: String },
    /// The servers to log in to.
    Servers,
    /// Post a note: its files are uploaded first, each with its alt text.
    Post {
        text: String,
        cw: Option<String>,
        visibility: Visibility,
        reply: Option<String>,
        renote: Option<String>,
        files: Vec<(PathBuf, String)>,
    },
    /// React to note `id` with `reaction`, or take the reaction back
    /// (`None`); `had` is the one given before, taken back first.
    React {
        id: String,
        reaction: Option<String>,
        had: Option<String>,
    },
    /// Renote note `id` (`on`), or take back your renotes of it.
    Renote { id: String, on: bool },
    /// Delete your own note `id`.
    Delete { id: String },
    /// Mark every notification read.
    MarkRead,
    /// Wait for the access asked for with MiAuth `session` on the server at
    /// `origin`, and make the login of it.
    Login { origin: String, session: String },
}

/// A note's thread.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Thread {
    /// What it answers, the first note of the conversation first.
    pub ancestors: Vec<Note>,
    pub note: Note,
    /// Its replies, each with the replies to it.
    pub replies: Vec<(Note, Vec<Note>)>,
}

/// A finished job.
#[derive(Debug)]
pub enum Event {
    Timeline {
        kind: TimelineKind,
        until: Option<String>,
        result: Result<Vec<Note>>,
    },
    Notifications {
        until: Option<String>,
        result: Result<Vec<Notification>>,
    },
    Thread {
        id: String,
        result: Result<Box<Thread>>,
    },
    /// Where the files were saved.
    Downloaded(Result<Vec<PathBuf>>),
    Opened(Result<()>),
    /// The note posted.
    Posted(Result<Box<Note>>),
    Reacted {
        id: String,
        reaction: Option<String>,
        had: Option<String>,
        result: Result<()>,
    },
    Renoted {
        id: String,
        on: bool,
        result: Result<()>,
    },
    Deleted {
        id: String,
        result: Result<()>,
    },
    MarkedRead(Result<()>),
    /// The servers to log in to, and why the Hub's list could not be read
    /// when the built-in one is given instead.
    Servers {
        list: Vec<Server>,
        failed: Option<Error>,
    },
    /// The login made for `session`, not yet saved.
    LoggedIn {
        session: String,
        result: Result<Session>,
    },
}

impl Job {
    /// The answer of this job when it failed with `e` before it could say
    /// anything else, so the screen stops waiting for it.
    fn failed(self, e: Error) -> Event {
        match self {
            Job::Timeline { kind, until } => Event::Timeline {
                kind,
                until,
                result: Err(e),
            },
            Job::Notifications { until } => Event::Notifications {
                until,
                result: Err(e),
            },
            Job::Thread { id } => Event::Thread { id, result: Err(e) },
            Job::Download { .. } => Event::Downloaded(Err(e)),
            Job::OpenLink { .. } => Event::Opened(Err(e)),
            Job::Servers => Event::Servers {
                list: crate::servers::builtin(),
                failed: Some(e),
            },
            Job::Post { .. } => Event::Posted(Err(e)),
            Job::React { id, reaction, had } => Event::Reacted {
                id,
                reaction,
                had,
                result: Err(e),
            },
            Job::Renote { id, on } => Event::Renoted {
                id,
                on,
                result: Err(e),
            },
            Job::Delete { id } => Event::Deleted { id, result: Err(e) },
            Job::MarkRead => Event::MarkedRead(Err(e)),
            Job::Login { session, .. } => Event::LoggedIn {
                session,
                result: Err(e),
            },
        }
    }
}

/// Do `job` as `client`.
pub fn run(job: Job, client: &Client) -> Event {
    match job {
        Job::Timeline { kind, until } => Event::Timeline {
            kind,
            result: client.timeline(kind.endpoint(), true, PAGE, until.as_deref()),
            until,
        },
        Job::Notifications { until } => Event::Notifications {
            result: client.notifications(PAGE, until.as_deref()),
            until,
        },
        Job::Thread { id } => Event::Thread {
            result: thread(client, &id).map(Box::new),
            id,
        },
        Job::Download { files } => Event::Downloaded(save(&files)),
        Job::Post {
            text,
            cw,
            visibility,
            reply,
            renote,
            files,
        } => Event::Posted(
            post(
                client,
                &Draft {
                    text,
                    cw,
                    visibility,
                    reply,
                    renote,
                    files,
                },
            )
            .map(Box::new),
        ),
        Job::React { id, reaction, had } => Event::Reacted {
            result: react(client, &id, reaction.as_deref(), had.as_deref()),
            id,
            reaction,
            had,
        },
        Job::Renote { id, on } => Event::Renoted {
            result: if on {
                client
                    .create_note(&json!({"renoteId": id, "visibility": "public"}))
                    .map(drop)
            } else {
                client.unrenote(&id)
            },
            id,
            on,
        },
        Job::Delete { id } => Event::Deleted {
            result: client.delete_note(&id),
            id,
        },
        Job::MarkRead => Event::MarkedRead(client.mark_all_read()),
        Job::Servers => {
            let (list, failed) = crate::servers::list();
            Event::Servers { list, failed }
        }
        Job::Login { origin, session } => Event::LoggedIn {
            result: login(&origin, &session),
            session,
        },
        Job::OpenLink { url } => Event::Opened(if crate::browser::open(&url) {
            Ok(())
        } else {
            Err(
                Error::io(format!("cannot open {url} in the web browser")).with_hint(format!(
                    "set {} to the program to open it with",
                    crate::browser::BROWSER_ENV
                )),
            )
        }),
    }
}

/// The thread of note `id`: the note, what it answers, its replies, and
/// the replies to the first of them that have some.
fn thread(client: &Client, id: &str) -> Result<Thread> {
    let note = client.note(id)?;
    let mut ancestors = if note.reply_id.is_some() {
        client.conversation(id, REPLIES)?
    } else {
        Vec::new()
    };
    ancestors.reverse();
    let mut replies = Vec::new();
    if note.replies_count > 0 || note.renote_count > 0 {
        for (i, r) in client.children(id, REPLIES)?.into_iter().enumerate() {
            let under = if i < NESTED && r.replies_count > 0 {
                client.children(&r.id, REPLIES)?
            } else {
                Vec::new()
            };
            replies.push((r, under));
        }
    }
    Ok(Thread {
        ancestors,
        note,
        replies,
    })
}

/// A note to post, as the composer made it.
struct Draft {
    text: String,
    cw: Option<String>,
    visibility: Visibility,
    reply: Option<String>,
    renote: Option<String>,
    files: Vec<(PathBuf, String)>,
}

/// The largest file the composer uploads, as `msky download` saves.
const MAX_UPLOAD_BYTES: u64 = crate::download::MAX_DOWNLOAD_BYTES;

/// Post `d`: the text checked against the server's own limit (its
/// `meta`), the files uploaded, then the note made. A write is sent once.
fn post(client: &Client, d: &Draft) -> Result<Note> {
    let max = client.max_note_chars();
    let n = d.text.chars().count();
    if n > max {
        return Err(Error::usage(format!(
            "the note is {n} characters; {} takes {max}",
            client.host()
        )));
    }
    let mut ids = Vec::new();
    for (path, alt) in &d.files {
        let len = std::fs::metadata(path)
            .map_err(|e| Error::io(format!("cannot read {}: {e}", path.display())))?
            .len();
        if len > MAX_UPLOAD_BYTES {
            return Err(Error::usage(format!(
                "{} is larger than {} MB",
                path.display(),
                MAX_UPLOAD_BYTES / 1024 / 1024
            )));
        }
        let data = std::fs::read(path)
            .map_err(|e| Error::io(format!("cannot read {}: {e}", path.display())))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".into());
        let alt = (!alt.trim().is_empty()).then_some(alt.as_str());
        ids.push(client.upload(&name, &data, alt)?.id);
    }
    let mut body = json!({"visibility": d.visibility.name()});
    if !d.text.is_empty() {
        body["text"] = json!(d.text);
    }
    if let Some(cw) = &d.cw {
        body["cw"] = json!(cw);
    }
    if let Some(r) = &d.reply {
        body["replyId"] = json!(r);
    }
    if let Some(r) = &d.renote {
        body["renoteId"] = json!(r);
    }
    if !ids.is_empty() {
        body["fileIds"] = json!(ids);
    }
    client.create_note(&body)
}

/// Take back the reaction `had`, then give `reaction`: Misskey takes one
/// reaction from an account per note.
fn react(client: &Client, id: &str, reaction: Option<&str>, had: Option<&str>) -> Result<()> {
    if had.is_some() {
        client.unreact(id)?;
    }
    match reaction {
        Some(r) => client.react(id, r),
        None => Ok(()),
    }
}

/// Wait for the access asked for with `session` on `origin`, as `msky
/// login` waits for it.
fn login(origin: &str, session: &str) -> Result<Session> {
    let client = Client::new(origin);
    let access = miauth::wait_for_access(
        &client,
        session,
        miauth::DEFAULT_TIMEOUT,
        miauth::INTERVAL,
        &mut miauth::WallClock::start(),
    )?;
    miauth::session_of(&client, access)
}

/// Save `files` in the download folder, as `msky download` saves them.
fn save(files: &[(String, usize, DriveFile)]) -> Result<Vec<PathBuf>> {
    let dir = download::download_dir(None)?;
    let agent = crate::api::agent();
    files
        .iter()
        .map(|(note, i, f)| download::save(&agent, f, &dir, &download::file_name(note, *i, f)))
        .collect()
}

/// Run `job` with `run`, answering it with its own failure if `run`
/// panics: a thread that died would leave the job unanswered.
fn guarded(job: Job, run: impl FnOnce(Job) -> Event) -> Event {
    let kept = job.clone();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(job))).unwrap_or_else(|panic| {
        let why = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        kept.failed(Error::api(format!("internal error: {why}")))
    })
}

/// Threads that run jobs.
const WORKERS: usize = 4;

type Queued = (u64, Job, Client);

/// Handle to the running workers.
pub struct Worker {
    /// The account the jobs sent from now on act as.
    client: Mutex<Client>,
    jobs: Sender<Queued>,
    answers: Receiver<(u64, Event)>,
}

impl Worker {
    /// Start the threads, acting as `client`. Fails when the system cannot
    /// start a thread.
    pub fn spawn(client: Client) -> std::io::Result<Self> {
        let (jobs, queue) = channel::<Queued>();
        let queue = Arc::new(Mutex::new(queue));
        let (tx, answers) = channel();
        for _ in 0..WORKERS {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            crate::tui::spawn("msky-worker", move || {
                loop {
                    let next = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok((seq, job, client)) = next else {
                        return;
                    };
                    let event = guarded(job, |job| run(job, &client));
                    if tx.send((seq, event)).is_err() {
                        return;
                    }
                }
            })?;
        }
        Ok(Self {
            client: Mutex::new(client),
            jobs,
            answers,
        })
    }

    /// Send `job`, numbered `seq`, to be done as the account in use now.
    pub fn send(&self, seq: u64, job: Job) {
        let client = self
            .client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "the threads end only when the worker is dropped, so a send fails only on the way out"
        )]
        let _: std::result::Result<(), _> = self.jobs.send((seq, job, client));
    }

    /// Act as `client` from the next job on.
    pub fn use_client(&self, client: Client) {
        *self.client.lock().unwrap_or_else(PoisonError::into_inner) = client;
    }

    /// An answer, if one has come.
    pub fn try_recv(&self) -> Option<(u64, Event)> {
        self.answers.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::stub_tests::{reply, serve};
    use serde_json::Value;
    use std::time::{Duration, Instant};

    fn wait(w: &Worker) -> (u64, Event) {
        let start = Instant::now();
        loop {
            if let Some(a) = w.try_recv() {
                return a;
            }
            assert!(start.elapsed() < Duration::from_secs(10), "no answer");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    // A thread is its note, what it answers (the first note first) and two
    // levels of replies.
    #[test]
    fn a_thread_is_its_ancestors_the_note_and_two_levels_of_replies() {
        let (url, log) = serve(Arc::new(|path, body, _| {
            let v: Value = serde_json::from_str(body).unwrap();
            match (path, v["noteId"].as_str()) {
                ("/api/notes/show", _) => reply(
                    200,
                    r#"{"id":"n","replyId":"p","text":"本文 👨‍👩‍👧","repliesCount":2}"#,
                ),
                ("/api/notes/conversation", _) => reply(
                    200,
                    r#"[{"id":"p","text":"親"},{"id":"root","text":"最初"}]"#,
                ),
                ("/api/notes/children", Some("n")) => reply(
                    200,
                    r#"[{"id":"r1","repliesCount":1},{"id":"r2","repliesCount":0}]"#,
                ),
                ("/api/notes/children", Some("r1")) => reply(200, r#"[{"id":"r1a"}]"#),
                _ => reply(404, "{}"),
            }
        }));
        let w = Worker::spawn(Client::new(&url)).unwrap();
        w.send(7, Job::Thread { id: "n".into() });
        let (seq, ev) = wait(&w);
        assert_eq!(seq, 7);
        let Event::Thread { id, result } = ev else {
            panic!("{ev:?}")
        };
        assert_eq!(id, "n");
        let t = result.unwrap();
        let ids = |v: &[Note]| v.iter().map(|n| n.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&t.ancestors), ["root", "p"]);
        assert_eq!(t.note.id, "n");
        assert_eq!(t.replies.len(), 2);
        assert_eq!(ids(&t.replies[0].1), ["r1a"]);
        assert!(
            t.replies[1].1.is_empty(),
            "a note without replies is not asked"
        );
        assert_eq!(log.lock().unwrap().len(), 4);
    }

    // A timeline page is asked with its untilId, renotes included, and its
    // number comes back with it.
    #[test]
    fn a_page_is_answered_under_the_number_it_was_sent_with() {
        let (url, log) = serve(Arc::new(|_, _, _| reply(200, r#"[{"id":"b"}]"#)));
        let w = Worker::spawn(Client::new(&url)).unwrap();
        w.send(
            3,
            Job::Timeline {
                kind: TimelineKind::Local,
                until: Some("c".into()),
            },
        );
        let (seq, ev) = wait(&w);
        assert_eq!(seq, 3);
        assert!(matches!(
            ev,
            Event::Timeline { kind: TimelineKind::Local, until: Some(ref u), result: Ok(ref n) }
                if u == "c" && n.len() == 1
        ));
        let log = log.lock().unwrap();
        assert_eq!(log[0].0, "/api/notes/local-timeline");
        let v: Value = serde_json::from_str(&log[0].1).unwrap();
        assert_eq!(v["untilId"], "c");
        assert_eq!(v["withRenotes"], true);
        assert_eq!(v["limit"], PAGE);
    }

    #[test]
    fn a_job_that_panics_is_answered_with_its_failure() {
        let ev = guarded(Job::Thread { id: "x".into() }, |_| panic!("boom"));
        let Event::Thread { result: Err(e), .. } = ev else {
            panic!("{ev:?}")
        };
        assert_eq!(e.message(), "internal error: boom");
    }

    // A post checks the server's own limit, uploads each file with its alt
    // text, then makes the note with them, once.
    #[test]
    fn a_post_uploads_its_files_and_makes_the_note() {
        let (url, log) = serve(Arc::new(|path, _, n| match path {
            "/api/meta" => reply(200, r#"{"maxNoteTextLength": 5}"#),
            "/api/drive/files/create" => reply(200, &format!(r#"{{"id":"f{n}"}}"#)),
            "/api/notes/create" => reply(200, r#"{"createdNote":{"id":"new"}}"#),
            _ => reply(404, "{}"),
        }));
        let dir = tempfile::tempdir().unwrap();
        let pic = dir.path().join("猫🐈.png");
        std::fs::write(&pic, b"\x89PNG").unwrap();
        let client = Client::new(&url).with_token(Some("fake-token-for-tests".into()));
        let d = Draft {
            text: "こんにちは".into(),
            cw: Some("注意".into()),
            visibility: Visibility::Home,
            reply: Some("r1".into()),
            renote: None,
            files: vec![(pic, "黒猫".into())],
        };
        assert_eq!(post(&client, &d).unwrap().id, "new");
        {
            let log = log.lock().unwrap();
            let paths: Vec<&str> = log.iter().map(|(p, _)| p.as_str()).collect();
            assert_eq!(
                paths,
                ["/api/meta", "/api/drive/files/create", "/api/notes/create"]
            );
            assert!(
                log[1].1.contains("黒猫"),
                "the alt text goes as the comment"
            );
            let v: Value = serde_json::from_str(&log[2].1).unwrap();
            assert_eq!(v["text"], "こんにちは");
            assert_eq!(v["cw"], "注意");
            assert_eq!(v["visibility"], "home");
            assert_eq!(v["replyId"], "r1");
            assert_eq!(v["fileIds"], serde_json::json!(["f1"]));
        }
        // Over the server's limit, nothing is uploaded or made.
        let long = Draft {
            text: "あいうえおか".into(),
            ..d
        };
        let e = post(&client, &long).unwrap_err();
        assert_eq!(
            e.message(),
            format!("the note is 6 characters; {} takes 5", client.host())
        );
        assert_eq!(log.lock().unwrap().len(), 4);
    }

    // A reaction replacing another takes the old one back first; a renote
    // taken back is notes/unrenote.
    #[test]
    fn a_reaction_replaces_the_one_given_and_a_renote_is_taken_back() {
        let (url, log) = serve(Arc::new(|_, _, _| reply(204, "")));
        let w = Worker::spawn(Client::new(&url).with_token(Some("fake-token-for-tests".into())))
            .unwrap();
        w.send(
            1,
            Job::React {
                id: "a".into(),
                reaction: Some("👍🏽".into()),
                had: Some("❤".into()),
            },
        );
        assert!(matches!(wait(&w).1, Event::Reacted { result: Ok(()), .. }));
        w.send(
            2,
            Job::Renote {
                id: "a".into(),
                on: false,
            },
        );
        assert!(matches!(
            wait(&w).1,
            Event::Renoted {
                on: false,
                result: Ok(()),
                ..
            }
        ));
        w.send(3, Job::MarkRead);
        assert!(matches!(wait(&w).1, Event::MarkedRead(Ok(()))));
        let log = log.lock().unwrap();
        let paths: Vec<&str> = log.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/api/notes/reactions/delete",
                "/api/notes/reactions/create",
                "/api/notes/unrenote",
                "/api/notifications/mark-all-as-read"
            ]
        );
        let v: Value = serde_json::from_str(&log[1].1).unwrap();
        assert_eq!(v["reaction"], "👍🏽");
        assert_eq!(v["noteId"], "a");
    }
}
