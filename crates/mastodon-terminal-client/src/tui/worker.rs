//! Network calls run on worker threads so the UI never blocks on the
//! server, as bsky's and msky's do.
//!
//! The UI sends a [`Job`] with its number; a worker performs it with the
//! [`Client`] in use when it was sent and answers with an [`Event`] under
//! the same number. The jobs run side by side on a few threads, so a thread
//! opened meanwhile does not wait behind a slow page.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use terminal_kit::inflight::InFlight;

use crate::api::types::{Account, Limits, Media, Notification, Status};
use crate::api::{Client, Next, Page};
use crate::cli::{TimelineKind, Visibility};
use crate::error::{Error, Result};

/// Statuses, or notifications, asked for a page at a time.
pub(crate) const PAGE: usize = 30;

/// Account actions share the relationship endpoint and optimistic state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationAction {
    Follow,
    Mute,
    Block,
}
impl RelationAction {
    fn endpoint(self, on: bool) -> &'static str {
        match (self, on) {
            (Self::Follow, true) => "follow",
            (Self::Follow, false) => "unfollow",
            (Self::Mute, true) => "mute",
            (Self::Mute, false) => "unmute",
            (Self::Block, true) => "block",
            (Self::Block, false) => "unblock",
        }
    }
    pub(crate) fn active(self, r: &crate::api::types::Relationship) -> bool {
        match self {
            Self::Follow => r.following || r.requested,
            Self::Mute => r.muting,
            Self::Block => r.blocking,
        }
    }
}

/// Work for the worker threads.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Job {
    Identity,
    Relationship {
        id: String,
    },
    Relate {
        id: String,
        action: RelationAction,
        on: bool,
    },
    Delete {
        id: String,
    },
    Report {
        account: String,
        status: Option<String>,
        comment: String,
    },
    /// A page of a timeline: the first, or the one older than `max_id`.
    Timeline {
        kind: TimelineKind,
        max_id: Option<String>,
    },
    /// A page of the notifications, as a timeline's.
    Notifications {
        max_id: Option<String>,
    },
    /// The thread of status `id`: what it answers, it, and its replies.
    Thread {
        id: String,
    },
    /// Favourite status `id` (`on`), or take the favourite back.
    Favourite {
        id: String,
        on: bool,
    },
    /// Boost status `id` (`on`), or take the boost back.
    Boost {
        id: String,
        on: bool,
    },
    /// Post a status.
    Post(Box<Draft>),
    /// Open an address in the web browser.
    OpenLink {
        url: String,
    },
    /// A search page; hashtags use max_id, other searches an offset.
    Search {
        query: String,
        accounts: bool,
        next: Option<String>,
    },
    /// An account and its posts; none means the account in use.
    Profile {
        id: Option<String>,
        max_id: Option<String>,
    },
    /// Save the original attachments using the command's naming policy.
    Download {
        id: String,
        files: Vec<Media>,
    },
    /// The limits of a post on the server.
    Limits,
}

/// A status to post, as the composer made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    pub files: Vec<(std::path::PathBuf, String)>,
    pub text: String,
    /// The content warning; none when empty.
    pub cw: String,
    pub visibility: Visibility,
    /// The status it replies to.
    pub reply: Option<String>,
}

impl Job {
    /// Whether it changes something on the server, which quitting right
    /// after sending it must not lose: a status, a favourite, a boost.
    pub(crate) fn changes_server(&self) -> bool {
        matches!(
            self,
            Job::Favourite { .. }
                | Job::Boost { .. }
                | Job::Post(_)
                | Job::Relate { .. }
                | Job::Delete { .. }
                | Job::Report { .. }
        )
    }
}

/// A status's thread.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Thread {
    /// What it answers, the first status of the conversation first.
    pub ancestors: Vec<Status>,
    pub status: Status,
    /// The replies under it, each reply after the status it answers.
    pub descendants: Vec<Status>,
}

/// A finished job.
#[derive(Debug)]
pub(crate) enum Event {
    Identity(Result<Account>),
    Relationship {
        id: String,
        result: Result<crate::api::types::Relationship>,
    },
    Related {
        id: String,
        result: Result<crate::api::types::Relationship>,
    },
    Deleted {
        id: String,
        result: Result<()>,
    },
    Reported(Result<()>),
    Timeline {
        kind: TimelineKind,
        max_id: Option<String>,
        result: Result<Page<Status>>,
    },
    Notifications {
        max_id: Option<String>,
        result: Result<Page<Notification>>,
    },
    Thread {
        id: String,
        result: Result<Box<Thread>>,
    },
    Favourited {
        id: String,
        on: bool,
        result: Result<()>,
    },
    Boosted {
        id: String,
        on: bool,
        result: Result<()>,
    },
    /// The status posted.
    Posted(Result<Box<Status>>),
    /// Whether a browser was started.
    Opened(bool),
    Limits(Limits),
    Search {
        next: Option<String>,
        result: Result<SearchPage>,
    },
    Profile {
        more: bool,
        result: Result<ProfilePage>,
    },
    Downloaded(Result<usize>),
}

impl Job {
    /// The answer of this job when it failed with `e` before it could say
    /// anything else, so the screen stops waiting for it.
    fn failed(self, e: Error) -> Event {
        match self {
            Job::Identity => Event::Identity(Err(e)),
            Job::Relationship { id } => Event::Relationship { id, result: Err(e) },
            Job::Relate { id, .. } => Event::Related { id, result: Err(e) },
            Job::Delete { id } => Event::Deleted { id, result: Err(e) },
            Job::Report { .. } => Event::Reported(Err(e)),
            Job::Timeline { kind, max_id } => Event::Timeline {
                kind,
                max_id,
                result: Err(e),
            },
            Job::Notifications { max_id } => Event::Notifications {
                max_id,
                result: Err(e),
            },
            Job::Thread { id } => Event::Thread { id, result: Err(e) },
            Job::Favourite { id, on } => Event::Favourited {
                id,
                on,
                result: Err(e),
            },
            Job::Boost { id, on } => Event::Boosted {
                id,
                on,
                result: Err(e),
            },
            Job::Post(_) => Event::Posted(Err(e)),
            Job::OpenLink { .. } => Event::Opened(false),
            Job::Limits => Event::Limits(Limits::default()),
            Job::Search { next, .. } => Event::Search {
                next,
                result: Err(e),
            },
            Job::Profile { max_id, .. } => Event::Profile {
                more: max_id.is_some(),
                result: Err(e),
            },
            Job::Download { .. } => Event::Downloaded(Err(e)),
        }
    }
}

/// Do `job` as `client`.
pub(crate) fn run(job: Job, client: &Client) -> Event {
    match job {
        Job::Identity => Event::Identity(client.me()),
        Job::Relationship { id } => Event::Relationship {
            result: client.relationship(&id),
            id,
        },
        Job::Relate { id, action, on } => Event::Related {
            result: client.relate(&id, action.endpoint(on)),
            id,
        },
        Job::Delete { id } => Event::Deleted {
            result: client.delete_status(&id),
            id,
        },
        Job::Report {
            account,
            status,
            comment,
        } => Event::Reported(client.report(&account, status.as_deref(), &comment)),
        Job::Timeline { kind, max_id } => {
            let (path, extra) = kind.path();
            Event::Timeline {
                kind,
                result: client.timeline(path, extra, PAGE, max_id.as_deref()),
                max_id,
            }
        }
        Job::Notifications { max_id } => Event::Notifications {
            result: client.notifications(PAGE, max_id.as_deref()),
            max_id,
        },
        Job::Thread { id } => Event::Thread {
            result: thread(client, &id).map(Box::new),
            id,
        },
        Job::Favourite { id, on } => Event::Favourited {
            result: client.act_on(&id, if on { "favourite" } else { "unfavourite" }),
            id,
            on,
        },
        Job::Boost { id, on } => Event::Boosted {
            result: client.act_on(&id, if on { "reblog" } else { "unreblog" }),
            id,
            on,
        },
        Job::Post(d) => Event::Posted(post(client, &d).map(Box::new)),
        Job::OpenLink { url } => Event::Opened(crate::browser::open(&url)),
        Job::Limits => Event::Limits(client.limits()),
        Job::Search {
            query,
            accounts,
            next,
        } => Event::Search {
            result: search(client, &query, accounts, next.as_deref()),
            next,
        },
        Job::Profile { id, max_id } => Event::Profile {
            more: max_id.is_some(),
            result: profile(client, id.as_deref(), max_id.as_deref()),
        },
        Job::Download { id, files } => Event::Downloaded(download(&id, &files)),
    }
}

/// The thread of status `id`: the status and its context, asked side by
/// side.
fn thread(client: &Client, id: &str) -> Result<Thread> {
    let (status, context) = std::thread::scope(|s| {
        let context = s.spawn(|| client.context(id));
        let status = client.status(id);
        let context = context
            .join()
            .unwrap_or_else(|_| Err(Error::api("the thread could not be read")));
        (status, context)
    });
    let context = context?;
    Ok(Thread {
        ancestors: context.ancestors,
        status: status?,
        descendants: context.descendants,
    })
}

/// Post `d`, sent once with an `Idempotency-Key`, as `mstdn post` posts.
fn post(client: &Client, d: &Draft) -> Result<Status> {
    let mut args = crate::cli::post::PostArgs {
        text: d.text.clone(),
        images: Vec::new(),
        alts: Vec::new(),
        video: None,
        reply: d.reply.clone(),
        cw: (!d.cw.trim().is_empty()).then(|| d.cw.clone()),
        sensitive: false,
        visibility: d.visibility,
        dry_run: false,
    };
    for (path, alt) in &d.files {
        if terminal_kit::files::is_video_name(&path.to_string_lossy()) {
            if args.video.is_some() {
                return Err(Error::usage("a post can have only one video"));
            }
            args.video = Some(path.clone());
        } else {
            args.images.push(path.clone());
        }
        args.alts.push(alt.clone());
    }
    crate::cli::post::composed(client, &args)
}

/// Where the page after `page` starts: what its `Link` header says, else
/// older than its last item; none when it was the last.
pub(crate) fn next_of<T>(page: &Page<T>, last_id: impl Fn(&T) -> &str) -> Option<String> {
    match &page.next {
        Next::End => None,
        Next::MaxId(m) => Some(m.clone()),
        Next::Unsaid => page.items.last().map(|i| last_id(i).to_string()),
    }
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

/// A job, its number, the account it acts as, and whether it changes the
/// server.
type Queued = (u64, Job, Client, bool);

/// Handle to the running workers.
pub(crate) struct Worker {
    client: Client,
    jobs: Sender<Queued>,
    answers: Receiver<(u64, Event)>,
    /// The jobs that change the server, sent and not done yet.
    writes: InFlight,
}

impl Worker {
    /// Start the threads, acting as `client`. Fails when the system cannot
    /// start a thread.
    pub(crate) fn spawn(client: Client) -> std::io::Result<Self> {
        let (jobs, queue) = channel::<Queued>();
        let queue = Arc::new(Mutex::new(queue));
        let (tx, answers) = channel();
        let writes = InFlight::default();
        for _ in 0..WORKERS {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let writes = writes.clone();
            std::thread::Builder::new()
                .name("mstdn-worker".into())
                .spawn(move || {
                    loop {
                        let next = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
                        let Ok((seq, job, client, write)) = next else {
                            return;
                        };
                        let event = guarded(job, |job| run(job, &client));
                        if write {
                            writes.done();
                        }
                        if tx.send((seq, event)).is_err() {
                            return;
                        }
                    }
                })?;
        }
        Ok(Self {
            client,
            jobs,
            answers,
            writes,
        })
    }

    /// Use the new account for jobs queued from now on; queued jobs keep theirs.
    pub(crate) fn set_client(&mut self, client: Client) {
        self.client = client;
    }

    /// Send `job`, numbered `seq`.
    pub(crate) fn send(&self, seq: u64, job: Job) {
        let write = job.changes_server();
        if write {
            self.writes.start();
        }
        if self
            .jobs
            .send((seq, job, self.client.clone(), write))
            .is_err()
            && write
        {
            // No thread is left to take it, so none will say it is done.
            self.writes.done();
        }
    }

    /// Wait, for at most `limit`, until every job sent that changes the
    /// server is done: a favourite given just before q still reaches the
    /// server. Returns at once when none is on its way.
    pub(crate) fn finish_writes(&self, limit: Duration) {
        self.writes.wait(limit);
    }

    /// An answer, if one has come.
    pub(crate) fn try_recv(&self) -> Option<(u64, Event)> {
        self.answers.try_recv().ok()
    }
}

/// The results of one search page.
#[derive(Debug)]
pub(crate) struct SearchPage {
    pub posts: Vec<Status>,
    pub accounts: Vec<Account>,
    pub next: Option<String>,
}
/// An account's header and one page of its posts.
#[derive(Debug)]
pub(crate) struct ProfilePage {
    pub account: Account,
    pub posts: Page<Status>,
}
fn search(client: &Client, query: &str, accounts: bool, next: Option<&str>) -> Result<SearchPage> {
    if !accounts && let Some(tag) = crate::cli::hashtag(query) {
        let p = client.timeline(
            &format!("v1/timelines/tag/{}", crate::api::percent_encode(tag)),
            &[],
            PAGE,
            next,
        )?;
        let next = next_of(&p, |s: &Status| &s.id);
        return Ok(SearchPage {
            posts: p.items,
            accounts: Vec::new(),
            next,
        });
    }
    if !client.logged_in() {
        return Err(Error::usage("log in to search accounts or posts"));
    }
    let offset = next.and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let p = client.search(
        query,
        if accounts { "accounts" } else { "statuses" },
        false,
        PAGE,
        offset,
    )?;
    let count = if accounts {
        p.accounts.len()
    } else {
        p.statuses.len()
    };
    Ok(SearchPage {
        posts: p.statuses,
        accounts: p.accounts,
        next: (count == PAGE).then(|| (offset + count).to_string()),
    })
}
fn profile(client: &Client, id: Option<&str>, max_id: Option<&str>) -> Result<ProfilePage> {
    let account = match id {
        Some(id) => client.account(id)?,
        None => client.me()?,
    };
    let posts = client.account_statuses(
        &account.id,
        &crate::api::StatusFilter::default(),
        PAGE,
        max_id,
    )?;
    Ok(ProfilePage { account, posts })
}
fn download(id: &str, files: &[Media]) -> Result<usize> {
    let dir = crate::download::download_dir(None)?;
    let agent = crate::api::agent();
    for (i, file) in files.iter().enumerate() {
        crate::download::save(&agent, file, &dir, &crate::download::file_stem(id, i + 1))?;
    }
    Ok(files.len())
}

#[cfg(test)]
mod tests;
