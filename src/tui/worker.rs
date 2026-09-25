//! Network calls run on a worker thread so the UI never blocks on the server.
//!
//! The UI sends a [`Job`]; the worker performs it with the [`Client`] it
//! holds and answers with an [`Event`]. Writes, and whatever else changes the
//! account, run one at a time in order on one thread, which keeps a like
//! followed by an unlike from racing each other. Reads run on a few threads
//! of their own, so a thread or a profile opens without waiting behind a
//! slow page, an upload, or each other.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use crate::api::types::{
    ChatMessage, Convo, FeedInfo, Media, Notification, Post, Profile, Record, ReplyRef, StrongRef,
    ThreadNode,
};
use crate::api::{self, Client, MAX_AVATAR_BYTES, ProfileEdit, newest};
use crate::config::{AccountStore, Session};
use crate::error::{Error, Result};
use crate::media;
use crate::timeline;

/// Work for the worker thread.
#[derive(Debug, Clone)]
pub enum Job {
    Login {
        service: String,
        identifier: String,
        password: String,
    },
    Timeline,
    /// The custom feeds the account pinned, with their names.
    PinnedFeeds,
    SearchPosts(String),
    SearchActors(String),
    /// Load an actor's profile and recent posts.
    OpenProfile(String),
    /// A post's thread, by the post's URI.
    Thread(String),
    /// The first page of notifications.
    Notifications,
    /// Notifications up to this time have been seen.
    UpdateSeen(String),
    /// The page of `feed` that starts at `cursor`.
    More {
        feed: Feed,
        cursor: String,
    },
    /// The conversations: the first page, or the one at `cursor`.
    Convos {
        cursor: Option<String>,
    },
    /// Messages of a conversation, newest first: the latest, or those
    /// before `cursor`.
    Messages {
        convo_id: String,
        cursor: Option<String>,
    },
    /// Send a message.
    SendMessage {
        convo_id: String,
        text: String,
    },
    /// The conversation with `did`, started if there is none.
    ConvoFor {
        did: String,
    },
    /// Mark a conversation read.
    ReadConvo {
        convo_id: String,
    },
    /// A page of the column `id`: its first page (`cursor` none) for the
    /// load numbered `generation`, or the one at `cursor`.
    Column {
        id: u64,
        generation: u64,
        feed: Feed,
        cursor: Option<String>,
    },
    Like {
        subject: StrongRef,
    },
    Unlike {
        post_uri: String,
        like_uri: String,
    },
    Repost {
        subject: StrongRef,
    },
    Unrepost {
        post_uri: String,
        repost_uri: String,
    },
    Follow {
        did: String,
    },
    Unfollow {
        did: String,
        follow_uri: String,
    },
    /// Mute (`on`) or unmute an account.
    Mute {
        did: String,
        on: bool,
    },
    Block {
        did: String,
    },
    Unblock {
        did: String,
        block_uri: String,
    },
    Post {
        text: String,
        reply: Option<ReplyRef>,
        /// The post this one quotes, when it does.
        quote: Option<StrongRef>,
        /// Pictures, or one video.
        media: Vec<Attachment>,
        /// Where a video is uploaded to.
        video_service: String,
    },
    /// Delete one of the account's own posts, by its URI.
    DeletePost {
        uri: String,
    },
    /// Load the fields the profile editor starts from.
    LoadProfileEditor,
    /// Save a picture or video of a post in `dir`, the download folder
    /// (`None` when there is none).
    Download {
        media: Media,
        dir: Option<PathBuf>,
    },
    /// Open a link in the web browser, with `browser` when one is named.
    OpenLink {
        url: String,
        browser: Option<String>,
    },
    /// A field that is `None` is left as the record has it.
    SaveProfile {
        display_name: Option<String>,
        description: Option<String>,
        /// A new avatar picture; `None` keeps the current one.
        avatar: Option<PathBuf>,
    },
}

pub use crate::compose::Attachment;

/// One page of a list and where the next page begins.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub cursor: Option<String>,
}

impl<T> From<Vec<T>> for Page<T> {
    /// A page with nothing after it.
    fn from(items: Vec<T>) -> Self {
        Self {
            items,
            cursor: None,
        }
    }
}

/// A list that continues past its first page, named by what it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    Timeline,
    /// A custom feed, by the URI of its generator.
    Custom(String),
    SearchPosts(String),
    SearchActors(String),
    /// An account's own posts, by DID.
    Author(String),
    Notifications,
}

/// A further page of posts or of accounts.
#[derive(Debug, Clone, PartialEq)]
pub enum MorePage {
    Posts(Page<Post>),
    Actors(Page<Profile>),
    Notifications(Page<NotifItem>),
}

/// A notification with the posts it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct NotifItem {
    pub n: Notification,
    /// For a reply, mention, or quote: that post, which can be answered.
    pub post: Option<Post>,
    /// For a like or repost: the viewer's post it is about.
    pub subject: Option<Post>,
    /// It was unread when it arrived. Kept after it is marked seen, so the
    /// marker stays for as long as the list is on screen.
    pub fresh: bool,
}

/// The reasons whose notification URI is a post of its own.
fn is_post_reason(reason: &str) -> bool {
    matches!(reason, "reply" | "mention" | "quote" | "subscribed-post")
}

/// The fields of the account's own profile record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileFields {
    pub display_name: String,
    pub description: String,
}

/// A finished job.
#[derive(Debug)]
pub enum Event {
    LoggedIn(Result<Session>),
    Timeline(Result<Page<Post>>),
    PinnedFeeds(Result<Vec<FeedInfo>>),
    /// The first page of posts found for `query`.
    SearchPosts {
        query: String,
        result: Result<Page<Post>>,
    },
    /// The first page of accounts found for `query`.
    SearchActors {
        query: String,
        result: Result<Page<Profile>>,
    },
    Profile(Result<(Profile, Page<Post>)>),
    /// A further page of `feed`, requested from `cursor`.
    More {
        feed: Feed,
        cursor: String,
        result: Result<MorePage>,
    },
    Convos {
        cursor: Option<String>,
        result: Result<Page<Convo>>,
    },
    /// Messages of `convo_id`, newest first.
    Messages {
        convo_id: String,
        cursor: Option<String>,
        result: Result<Page<ChatMessage>>,
    },
    MessageSent {
        convo_id: String,
        /// The text sent, as the box held it.
        text: String,
        result: Result<ChatMessage>,
    },
    ConvoFor {
        did: String,
        result: Result<Convo>,
    },
    ConvoRead {
        convo_id: String,
        result: Result<()>,
    },
    /// A page of the column `id`, for the load `generation`; `cursor` is
    /// where it was asked from, none for the first page.
    Column {
        id: u64,
        generation: u64,
        cursor: Option<String>,
        result: Result<MorePage>,
    },
    Thread {
        uri: String,
        result: Result<ThreadNode>,
    },
    /// The first page of notifications; `seen_at` is the newest one's time.
    Notifications {
        seen_at: String,
        result: Result<Page<NotifItem>>,
    },
    Seen(Result<()>),
    Liked {
        post_uri: String,
        result: Result<String>,
    },
    Unliked {
        post_uri: String,
        result: Result<()>,
    },
    Reposted {
        post_uri: String,
        result: Result<String>,
    },
    Unreposted {
        post_uri: String,
        result: Result<()>,
    },
    Followed {
        did: String,
        result: Result<String>,
    },
    Unfollowed {
        did: String,
        result: Result<()>,
    },
    Muted {
        did: String,
        on: bool,
        result: Result<()>,
    },
    Blocked {
        did: String,
        result: Result<String>,
    },
    Unblocked {
        did: String,
        result: Result<()>,
    },
    Posted {
        reply_to: Option<String>,
        result: Result<()>,
    },
    /// The post at this URI was deleted, or the server refused to.
    PostDeleted {
        uri: String,
        result: Result<()>,
    },
    ProfileEditor(Result<ProfileFields>),
    ProfileSaved(Result<()>),
    /// Where the download was saved.
    Downloaded(Result<PathBuf>),
    Opened {
        url: String,
        result: Result<()>,
    },
}

impl Job {
    /// Whether the job only reads, so it may run beside other jobs and out
    /// of order with them. The profile editor's load stays in order: the
    /// save that follows it writes exactly the version it read.
    pub fn reads(&self) -> bool {
        matches!(
            self,
            Job::Timeline
                | Job::PinnedFeeds
                | Job::SearchPosts(_)
                | Job::SearchActors(_)
                | Job::OpenProfile(_)
                | Job::Thread(_)
                | Job::Notifications
                | Job::More { .. }
                | Job::Column { .. }
                | Job::Convos { .. }
                | Job::Messages { .. }
                | Job::ConvoFor { .. }
                | Job::Download { .. }
                | Job::OpenLink { .. }
        )
    }

    /// The answer of this job when it failed with `e` before it could say
    /// anything else, so the screen stops waiting for it.
    fn failed(self, e: Error) -> Event {
        match self {
            Job::Login { .. } => Event::LoggedIn(Err(e)),
            Job::Timeline => Event::Timeline(Err(e)),
            Job::PinnedFeeds => Event::PinnedFeeds(Err(e)),
            Job::SearchPosts(query) => Event::SearchPosts {
                query,
                result: Err(e),
            },
            Job::SearchActors(query) => Event::SearchActors {
                query,
                result: Err(e),
            },
            Job::OpenProfile(_) => Event::Profile(Err(e)),
            Job::Notifications => Event::Notifications {
                seen_at: api::now(),
                result: Err(e),
            },
            Job::UpdateSeen(_) => Event::Seen(Err(e)),
            Job::Thread(uri) => Event::Thread {
                uri,
                result: Err(e),
            },
            Job::More { feed, cursor } => Event::More {
                feed,
                cursor,
                result: Err(e),
            },
            Job::Convos { cursor } => Event::Convos {
                cursor,
                result: Err(e),
            },
            Job::Messages { convo_id, cursor } => Event::Messages {
                convo_id,
                cursor,
                result: Err(e),
            },
            Job::SendMessage { convo_id, text } => Event::MessageSent {
                convo_id,
                text,
                result: Err(e),
            },
            Job::ConvoFor { did } => Event::ConvoFor {
                did,
                result: Err(e),
            },
            Job::ReadConvo { convo_id } => Event::ConvoRead {
                convo_id,
                result: Err(e),
            },
            Job::Column {
                id,
                generation,
                cursor,
                ..
            } => Event::Column {
                id,
                generation,
                cursor,
                result: Err(e),
            },
            Job::Like { subject } => Event::Liked {
                post_uri: subject.uri,
                result: Err(e),
            },
            Job::Unlike { post_uri, .. } => Event::Unliked {
                post_uri,
                result: Err(e),
            },
            Job::Repost { subject } => Event::Reposted {
                post_uri: subject.uri,
                result: Err(e),
            },
            Job::Unrepost { post_uri, .. } => Event::Unreposted {
                post_uri,
                result: Err(e),
            },
            Job::Follow { did } => Event::Followed {
                did,
                result: Err(e),
            },
            Job::Unfollow { did, .. } => Event::Unfollowed {
                did,
                result: Err(e),
            },
            Job::Mute { did, on } => Event::Muted {
                did,
                on,
                result: Err(e),
            },
            Job::Block { did } => Event::Blocked {
                did,
                result: Err(e),
            },
            Job::Unblock { did, .. } => Event::Unblocked {
                did,
                result: Err(e),
            },
            Job::Post { reply, .. } => Event::Posted {
                reply_to: reply.map(|r| r.parent.uri),
                result: Err(e),
            },
            Job::DeletePost { uri } => Event::PostDeleted {
                uri,
                result: Err(e),
            },
            Job::LoadProfileEditor => Event::ProfileEditor(Err(e)),
            Job::Download { .. } => Event::Downloaded(Err(e)),
            Job::OpenLink { url, .. } => Event::Opened {
                url,
                result: Err(e),
            },
            Job::SaveProfile { .. } => Event::ProfileSaved(Err(e)),
        }
    }
}

/// Run `job` with `run`, answering it with its own failure if `run`
/// panics: a thread that died would leave the job unanswered and take the
/// jobs queued behind it along.
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

/// Threads that run reads. The start asks for the timeline and the
/// notifications at once, and for each column shown; the others keep a
/// thread the user opens meanwhile from waiting behind them.
const READERS: usize = 4;

/// Handle to the running worker.
pub struct Worker {
    /// The account the jobs sent from now on act as; each job takes it when
    /// it is sent.
    client: Arc<Mutex<Option<Client>>>,
    accounts: AccountStore,
    writes: Sender<(u64, Job, Option<Client>)>,
    reads: Sender<(u64, Job, Option<Client>)>,
    rx: Receiver<(u64, Event)>,
}

impl Worker {
    /// Start the worker. `session` is the account to act as, if any; its
    /// refreshed tokens are saved to its file in `accounts`.
    pub fn spawn(session: Option<Session>, accounts: AccountStore) -> Self {
        let client = Arc::new(Mutex::new(session.map(|s| {
            let store = accounts.store_for(&s.did);
            Client::new(s, Some(store))
        })));
        let (ev_tx, ev_rx) = channel::<(u64, Event)>();
        let (writes, write_rx) = channel::<(u64, Job, Option<Client>)>();
        let mut state = State {
            acting: None,
            accounts: accounts.clone(),
            editor_base: None,
        };
        let events = ev_tx.clone();
        crate::tui::spawn("bsky-write", move || {
            for (seq, job, acting) in write_rx {
                state.acting = acting;
                if events
                    .send((seq, guarded(job, |job| state.run(job))))
                    .is_err()
                {
                    break;
                }
            }
        });
        let (reads, read_rx) = channel::<(u64, Job, Option<Client>)>();
        let read_rx = Arc::new(Mutex::new(read_rx));
        for _ in 0..READERS {
            let mut state = State {
                acting: None,
                accounts: accounts.clone(),
                editor_base: None,
            };
            let jobs = Arc::clone(&read_rx);
            let events = ev_tx.clone();
            crate::tui::spawn("bsky-read", move || {
                loop {
                    // Held only while waiting for a job, not while running it.
                    let job = jobs.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok((seq, job, acting)) = job else { break };
                    state.acting = acting;
                    if events
                        .send((seq, guarded(job, |job| state.run(job))))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        Self {
            client,
            accounts,
            writes,
            reads,
            rx: ev_rx,
        }
    }

    /// Act as `session` from the next job on. Done here, before any job for
    /// the account is sent, rather than as a job: reads run beside each
    /// other, and one could start before a job that switched.
    pub fn use_account(&self, session: Session) {
        let store = self.accounts.store_for(&session.did);
        *self.client.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(Client::new(session, Some(store)));
    }

    /// Act as nobody: every job fails until an account is used or logged in.
    pub fn use_no_account(&self) {
        *self.client.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// Queue a job; its answer comes back with the same `seq`. It acts as
    /// the account in use now, even when it waits behind a slow write
    /// while another account is switched to.
    pub fn send(&self, seq: u64, job: Job) {
        let acting = self
            .client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        // The worker only stops when the UI drops it, and a job that panics
        // is answered like one that failed, so a send cannot fail while the
        // UI is still running.
        let lane = if job.reads() {
            &self.reads
        } else {
            &self.writes
        };
        let _ = lane.send((seq, job, acting));
    }

    /// A finished job and the `seq` it was sent with, if any.
    pub fn try_recv(&self) -> Option<(u64, Event)> {
        self.rx.try_recv().ok()
    }
}

struct State {
    /// The account in use when the running job was sent, which it acts as.
    acting: Option<Client>,
    accounts: AccountStore,
    /// The profile record the open editor was filled from (`Some(None)` when
    /// the account has none yet); saving edits exactly this version.
    editor_base: Option<Option<Record>>,
}

impl State {
    fn client(&self) -> Result<Client> {
        self.acting
            .clone()
            .ok_or_else(|| Error::api(crate::i18n::t("not logged in")))
    }

    fn run(&mut self, job: Job) -> Event {
        match job {
            Job::Login {
                service,
                identifier,
                password,
            } => Event::LoggedIn(self.login(&service, &identifier, &password)),
            Job::Timeline => Event::Timeline(self.timeline(None)),
            Job::PinnedFeeds => Event::PinnedFeeds(self.client().and_then(|c| c.pinned_feeds())),
            Job::SearchPosts(query) => Event::SearchPosts {
                result: self.search_posts(&query, None),
                query,
            },
            Job::SearchActors(query) => Event::SearchActors {
                result: self.search_actors(&query, None),
                query,
            },
            Job::OpenProfile(actor) => Event::Profile(self.open_profile(&actor)),
            Job::Notifications => {
                let result = self.notifications(None);
                // Seen up to the newest notification shown, in the server's
                // own time: a local clock running fast would otherwise mark
                // notifications that arrive later as seen too.
                let seen_at = result
                    .as_ref()
                    .ok()
                    .and_then(|p| newest(p.items.iter().map(|i| i.n.indexed_at.as_str())))
                    .unwrap_or_else(api::now);
                Event::Notifications { result, seen_at }
            }
            Job::UpdateSeen(at) => Event::Seen(self.client().and_then(|c| c.update_seen(&at))),
            Job::Thread(uri) => Event::Thread {
                result: self.client().and_then(|c| c.post_thread(&uri)),
                uri,
            },
            Job::More { feed, cursor } => Event::More {
                result: self.page(&feed, Some(&cursor)),
                feed,
                cursor,
            },
            Job::Convos { cursor } => Event::Convos {
                result: self.client().and_then(|c| {
                    let out = c.convos(cursor.as_deref())?;
                    Ok(Page {
                        items: out.convos,
                        cursor: out.cursor,
                    })
                }),
                cursor,
            },
            Job::Messages { convo_id, cursor } => Event::Messages {
                result: self.client().and_then(|c| {
                    let out = c.messages(&convo_id, cursor.as_deref())?;
                    Ok(Page {
                        items: out.messages,
                        cursor: out.cursor,
                    })
                }),
                convo_id,
                cursor,
            },
            Job::SendMessage { convo_id, text } => Event::MessageSent {
                result: self.client().and_then(|c| c.send_message(&convo_id, &text)),
                convo_id,
                text,
            },
            Job::ConvoFor { did } => Event::ConvoFor {
                result: self.client().and_then(|c| c.convo_for(&did)),
                did,
            },
            Job::ReadConvo { convo_id } => Event::ConvoRead {
                result: self.client().and_then(|c| c.update_read(&convo_id, None)),
                convo_id,
            },
            Job::Column {
                id,
                generation,
                feed,
                cursor,
            } => Event::Column {
                result: self.page(&feed, cursor.as_deref()),
                id,
                generation,
                cursor,
            },
            Job::Repost { subject } => Event::Reposted {
                post_uri: subject.uri.clone(),
                result: self.client().and_then(|c| c.repost(&subject)),
            },
            Job::Unrepost {
                post_uri,
                repost_uri,
            } => Event::Unreposted {
                post_uri,
                result: self.client().and_then(|c| c.unrepost(&repost_uri)),
            },
            Job::Like { subject } => Event::Liked {
                post_uri: subject.uri.clone(),
                result: self.client().and_then(|c| c.like(&subject)),
            },
            Job::Unlike { post_uri, like_uri } => Event::Unliked {
                post_uri,
                result: self.client().and_then(|c| c.unlike(&like_uri)),
            },
            Job::Follow { did } => Event::Followed {
                result: self.client().and_then(|c| c.follow(&did)),
                did,
            },
            Job::Unfollow { did, follow_uri } => Event::Unfollowed {
                did,
                result: self.client().and_then(|c| c.unfollow(&follow_uri)),
            },
            Job::Mute { did, on } => Event::Muted {
                result: self
                    .client()
                    .and_then(|c| if on { c.mute(&did) } else { c.unmute(&did) }),
                did,
                on,
            },
            Job::Block { did } => Event::Blocked {
                result: self.client().and_then(|c| c.block(&did)),
                did,
            },
            Job::Unblock { did, block_uri } => Event::Unblocked {
                did,
                result: self.client().and_then(|c| c.unblock(&block_uri)),
            },
            Job::Post {
                text,
                reply,
                quote,
                media,
                video_service,
            } => Event::Posted {
                reply_to: reply.as_ref().map(|r| r.parent.uri.clone()),
                result: self.post(
                    &text,
                    reply.as_ref(),
                    quote.as_ref(),
                    &media,
                    &video_service,
                ),
            },
            Job::DeletePost { uri } => Event::PostDeleted {
                result: self.client().and_then(|c| c.delete_post(&uri)),
                uri,
            },
            Job::LoadProfileEditor => Event::ProfileEditor(self.profile_fields()),
            Job::Download { media, dir } => Event::Downloaded(download(&media, dir.as_deref())),
            Job::OpenLink { url, browser } => Event::Opened {
                result: crate::browser::open(&url, browser.as_deref()),
                url,
            },
            Job::SaveProfile {
                display_name,
                description,
                avatar,
            } => {
                Event::ProfileSaved(self.save_profile(display_name, description, avatar.as_deref()))
            }
        }
    }

    /// Log in and keep the account. Jobs go on acting as the account in use
    /// until the UI takes the answer and switches (`Worker::use_account`):
    /// one it sends before that is for what it still shows.
    fn login(&self, service: &str, identifier: &str, password: &str) -> Result<Session> {
        let session = api::login(service, identifier, password)?;
        self.accounts.save(&session)?;
        Ok(session)
    }

    /// A page of the timeline, filtered to posts by followed accounts.
    ///
    /// A raw page can be all reposts and own posts, which filters down to
    /// nothing; then the next raw pages are read (a few at most) so that the
    /// user gets posts, not an empty page that ends the scrolling.
    fn timeline(&self, cursor: Option<&str>) -> Result<Page<Post>> {
        const RAW_PAGES: usize = 3;
        let client = self.client()?;
        let did = client.did().to_string();
        let mut cursor = cursor.map(str::to_string);
        let mut items = Vec::new();
        for _ in 0..RAW_PAGES {
            let raw = client.timeline(cursor.as_deref())?;
            items.extend(timeline::followed_posts(raw.feed, &did));
            let advanced = raw.cursor.is_some() && raw.cursor != cursor;
            cursor = raw.cursor;
            if !items.is_empty() || !advanced {
                break;
            }
        }
        Ok(Page { items, cursor })
    }

    /// A page of a custom feed, as the feed chose it: reposts and posts by
    /// accounts you do not follow included, replies with their context.
    fn custom_feed(&self, uri: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        let raw = self.client()?.feed(uri, cursor)?;
        Ok(Page {
            items: timeline::feed_posts(raw.feed),
            cursor: raw.cursor,
        })
    }

    fn search_posts(&self, q: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        let r = self.client()?.search_posts(q, cursor)?;
        Ok(Page {
            items: r.posts,
            cursor: r.cursor,
        })
    }

    fn search_actors(&self, q: &str, cursor: Option<&str>) -> Result<Page<Profile>> {
        let r = self.client()?.search_actors(q, cursor)?;
        Ok(Page {
            items: r.actors,
            cursor: r.cursor,
        })
    }

    /// An account's own posts: reposts are left out, as on the timeline.
    fn author_feed(&self, did: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        Ok(author_page(self.client()?.author_feed(did, cursor)?))
    }

    /// A page of notifications, joined with the posts they are about.
    fn notifications(&self, cursor: Option<&str>) -> Result<Page<NotifItem>> {
        let client = self.client()?;
        let page = client.notifications(cursor)?;
        // One getPosts for the whole page. A like or repost of a repost
        // names the repost, which getPosts does not return, so only post
        // URIs are asked for.
        let mut uris: Vec<String> = Vec::new();
        for n in &page.notifications {
            let uri = if is_post_reason(&n.reason) {
                Some(n.uri.as_str())
            } else {
                n.reason_subject.as_deref()
            };
            if let Some(u) = uri.filter(|u| u.contains("/app.bsky.feed.post/"))
                && !uris.iter().any(|x| x == u)
            {
                uris.push(u.to_string());
            }
        }
        // The list is useful without the post texts; a failed join shows it
        // without them rather than not at all.
        let posts = if uris.is_empty() {
            Vec::new()
        } else {
            client.posts(&uris).unwrap_or_default()
        };
        let find = |uri: Option<&str>| uri.and_then(|u| posts.iter().find(|p| p.uri == u)).cloned();
        let items = page
            .notifications
            .into_iter()
            .map(|n| {
                let (post, subject) = if is_post_reason(&n.reason) {
                    (find(Some(&n.uri)), None)
                } else {
                    (None, find(n.reason_subject.as_deref()))
                };
                NotifItem {
                    fresh: !n.is_read,
                    n,
                    post,
                    subject,
                }
            })
            .collect();
        Ok(Page {
            items,
            cursor: page.cursor,
        })
    }

    /// The profile and its posts, asked for at once: getAuthorFeed takes a
    /// handle as well as a DID, so it need not wait for the profile.
    fn open_profile(&self, actor: &str) -> Result<(Profile, Page<Post>)> {
        let client = self.client()?;
        let (profile, posts) = thread::scope(|s| {
            let posts = s.spawn(|| client.author_feed(actor, None));
            let profile = client.profile(actor);
            let posts = posts.join().unwrap_or_else(|_| {
                Err(Error::api(
                    "app.bsky.feed.getAuthorFeed: the request thread failed",
                ))
            });
            (profile, posts)
        });
        Ok((profile?, author_page(posts?)))
    }

    /// The page of `feed` at `cursor`, or its first page.
    fn page(&mut self, feed: &Feed, cursor: Option<&str>) -> Result<MorePage> {
        let c = cursor;
        Ok(match feed {
            Feed::Timeline => MorePage::Posts(self.timeline(c)?),
            Feed::Custom(uri) => MorePage::Posts(self.custom_feed(uri, c)?),
            Feed::SearchPosts(q) => MorePage::Posts(self.search_posts(q, c)?),
            Feed::SearchActors(q) => MorePage::Actors(self.search_actors(q, c)?),
            Feed::Author(did) => MorePage::Posts(self.author_feed(did, c)?),
            Feed::Notifications => MorePage::Notifications(self.notifications(c)?),
        })
    }

    fn profile_fields(&mut self) -> Result<ProfileFields> {
        let record = self.client()?.own_profile_record()?;
        let value = record.as_ref().map(|r| r.value.clone()).unwrap_or_default();
        self.editor_base = Some(record);
        let field = |k: &str| {
            value
                .get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        Ok(ProfileFields {
            display_name: field("displayName"),
            description: field("description"),
        })
    }

    /// Prepare every picture (or the video), upload them, then publish the
    /// post with them. Everything is prepared before anything is uploaded,
    /// so a file that cannot be read stops the post before anything reaches
    /// the server.
    fn post(
        &self,
        text: &str,
        reply: Option<&ReplyRef>,
        quote: Option<&StrongRef>,
        media: &[Attachment],
        video_service: &str,
    ) -> Result<()> {
        let client = self.client()?;
        // The post says it is written in the language chosen for the screens.
        let writer = crate::i18n::current();
        crate::compose::send_post(&client, text, reply, quote, media, video_service, writer)
            .map(|_| ())
    }

    fn save_profile(
        &mut self,
        display_name: Option<String>,
        description: Option<String>,
        avatar: Option<&std::path::Path>,
    ) -> Result<()> {
        let avatar = avatar.map(read_avatar).transpose()?;
        let base = match self.editor_base.clone() {
            Some(base) => base,
            // The UI opens the editor before saving, so this is a fallback.
            None => self.client()?.own_profile_record()?,
        };
        let edit = ProfileEdit {
            display_name,
            description,
            avatar,
        };
        self.client()?.update_profile(base.as_ref(), &edit)?;
        // The next edit starts from what was just written, which the editor
        // reads again when it opens.
        self.editor_base = None;
        Ok(())
    }
}

/// A page of an account's posts without its reposts, as on the timeline.
fn author_page(r: crate::api::types::AuthorFeed) -> Page<Post> {
    Page {
        items: r
            .feed
            .into_iter()
            .filter(|i| i.reason.is_none())
            .map(|i| i.post)
            .collect(),
        cursor: r.cursor,
    }
}

/// Largest file a download saves.
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

fn fetch_bytes(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    let mut resp = agent.get(url).call().map_err(|e| {
        Error::api(crate::i18n::tf(
            "cannot download {}: {}",
            &[url, &e.to_string()],
        ))
    })?;
    if !resp.status().is_success() {
        return Err(Error::api(crate::i18n::tf(
            "cannot download {}: HTTP {}",
            &[url, &(resp.status().as_u16()).to_string()],
        )));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_vec()
        .map_err(|e| {
            Error::api(crate::i18n::tf(
                "cannot download {}: {}",
                &[url, &e.to_string()],
            ))
        })
}

/// The file name a Bluesky media URL suggests: `<cid>.<ext>` for
/// `.../plain/<did>/<cid>@jpeg`, `<cid>.ts` for a video's
/// `.../watch/<did>/<cid>/playlist.m3u8`. Only ASCII letters, digits, `-`,
/// `_`, and inner dots are kept, and a name Windows reserves for a device
/// (`CON`, `NUL`, `COM1`...) becomes the fallback, so the name works on
/// every system and never leaves the download folder.
fn download_name(media: &Media) -> String {
    let (last, fallback) = match media {
        Media::Image { url, .. } => (url_path(url).rsplit('/').next().unwrap_or(""), "picture"),
        Media::Video { playlist, .. } => {
            (url_path(playlist).rsplit('/').nth(1).unwrap_or(""), "video")
        }
    };
    let (stem, ext) = match media {
        Media::Video { .. } => (last, "ts"),
        Media::Image { .. } => match last.split_once('@') {
            Some(parts) => parts,
            None => last.rsplit_once('.').unwrap_or((last, "jpg")),
        },
    };
    let ext = match clean_name(ext).to_ascii_lowercase().as_str() {
        "" => "jpg".to_string(),
        "jpeg" => "jpg".to_string(),
        e => e.to_string(),
    };
    let mut stem = clean_name(stem);
    // Most file systems take 255 bytes a name; the room left leaves space
    // for " (9999)" and the extension. The name is ASCII, so any cut is on
    // a character boundary.
    stem.truncate(MAX_STEM_BYTES);
    let stem = stem.trim_end_matches('.');
    let stem = if stem.is_empty() || is_reserved_name(stem) {
        fallback
    } else {
        stem
    };
    format!("{stem}.{ext}")
}

/// Longest stem of a download's name, in bytes.
const MAX_STEM_BYTES: usize = 100;

/// A URL without its query and fragment.
fn url_path(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

/// `s` with only the characters a file name keeps everywhere, and without
/// dots at either end (Windows drops trailing ones; leading ones hide it).
fn clean_name(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    kept.trim_matches('.').to_string()
}

/// Whether Windows reserves `stem` for a device, whatever follows its first
/// dot: `nul.txt` opens the device as `nul` does.
fn is_reserved_name(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or(stem).to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit())
}

/// The names to try for `name` in `dir`: `name`, then `name (1)`...
fn candidate_path(dir: &std::path::Path, name: &str, i: usize) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    dir.join(match (i, ext.is_empty()) {
        (0, _) => name.to_string(),
        (i, true) => format!("{stem} ({i})"),
        (i, false) => format!("{stem} ({i}).{ext}"),
    })
}

/// Save a picture (full size) or a video (its best variant, the segments
/// joined into one MPEG transport stream, which players play as it is).
fn download(media: &Media, dir: Option<&std::path::Path>) -> Result<PathBuf> {
    let dir = dir.ok_or_else(|| {
        Error::io(crate::i18n::t("there is no download folder")).with_hint(crate::i18n::t(
            "choose one on the settings screen (s on your Profile tab)",
        ))
    })?;
    let agent = api::agent();
    let bytes = match media {
        Media::Image { url, .. } => fetch_bytes(&agent, url)?,
        Media::Video { playlist, .. } => {
            let text = |url: &str| {
                fetch_bytes(&agent, url).map(|b| String::from_utf8_lossy(&b).into_owned())
            };
            let master = text(playlist)?;
            let (media_url, media_text) = match crate::hls::pick_best_variant(&master, playlist) {
                Some(url) => {
                    let t = text(&url)?;
                    (url, t)
                }
                None => (playlist.clone(), master),
            };
            let all = join_segments(
                crate::hls::segments(&media_text, &media_url),
                MAX_DOWNLOAD_BYTES,
                |seg| fetch_bytes(&agent, seg),
            )?;
            if all.is_empty() {
                return Err(Error::api(crate::i18n::t(
                    "the video's playlist lists nothing to download",
                )));
            }
            all
        }
    };
    let name = match media {
        Media::Image { .. } => picture_name(&download_name(media), &bytes)?,
        Media::Video { .. } => download_name(media),
    };
    std::fs::create_dir_all(dir).map_err(|e| {
        Error::io(crate::i18n::tf(
            "cannot create {}: {}",
            &[&(dir.display()).to_string(), &e.to_string()],
        ))
    })?;
    save_new(dir, &name, &bytes)
}

/// The segments of a video fetched with `fetch` and joined, refused once
/// they come to more than `limit` bytes: each is limited on its own, and a
/// playlist may list any number of them.
fn join_segments(
    segments: impl IntoIterator<Item = String>,
    limit: u64,
    mut fetch: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut all = Vec::new();
    for seg in segments {
        all.extend(fetch(&seg)?);
        if all.len() as u64 > limit {
            return Err(Error::api(crate::i18n::tf(
                "the video is larger than {} MB",
                &[&(limit / (1024 * 1024)).to_string()],
            )));
        }
    }
    Ok(all)
}

/// `name` with the extension of the picture `bytes` hold. The server's
/// address names the file, but only the bytes say what it is: a picture at
/// an address ending in `.exe` must not be saved as a program, and what is
/// not a picture at all is not saved.
fn picture_name(name: &str, bytes: &[u8]) -> Result<String> {
    use image::ImageFormat;
    let ext = match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => "png",
        Ok(ImageFormat::Jpeg) => "jpg",
        Ok(ImageFormat::Gif) => "gif",
        Ok(ImageFormat::WebP) => "webp",
        _ => {
            return Err(Error::api(crate::i18n::tf(
                "{} is not a picture bsky can save; nothing was written",
                &[name],
            )));
        }
    };
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    Ok(format!("{stem}.{ext}"))
}

/// Write `bytes` to a new file in `dir` named `name`, or `name (1)`... A
/// name is taken when anything is there, a link to nothing included: the
/// file is created only if it does not exist (`create_new`), so nothing is
/// overwritten or written through a link, even if it appears meanwhile.
fn save_new(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    use std::io::Write;
    save_new_with(dir, name, |f| f.write_all(bytes))
}

/// [`save_new`], with `write` filling the new file.
fn save_new_with(
    dir: &std::path::Path,
    name: &str,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<PathBuf> {
    for i in 0..10_000 {
        let path = candidate_path(dir, name, i);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(Error::io(crate::i18n::tf(
                    "cannot write {}: {}",
                    &[&(path.display()).to_string(), &e.to_string()],
                )));
            }
        };
        if let Err(e) = write(&mut file).and_then(|()| file.sync_all()) {
            // What was written of it is not the picture or the video: a
            // full disk left a cut file that looked like the download, and
            // took its name from the next try.
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(Error::io(crate::i18n::tf(
                "cannot write {}: {}",
                &[&(path.display()).to_string(), &e.to_string()],
            )));
        }
        return Ok(path);
    }
    Err(Error::io(crate::i18n::tf(
        "{} has too many files named like {}",
        &[&(dir.display()).to_string(), name],
    )))
}

/// Read a new avatar and encode it the way post pictures are: upright,
/// scaled, under the size limit, without the camera's metadata.
fn read_avatar(path: &std::path::Path) -> Result<(Vec<u8>, String)> {
    let p = media::prepare_avatar(path, MAX_AVATAR_BYTES)?;
    Ok((p.bytes, p.mime.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // A panic in a job ended its thread: the job never answered, so the
    // screen waited for it for good, and a panic on the write thread left
    // every later like, post and follow unsent.
    #[test]
    fn a_job_that_panics_is_answered_with_its_own_failure() {
        let subject = StrongRef {
            uri: "at://did:plc:a/app.bsky.feed.post/1".into(),
            cid: "c".into(),
        };
        let event = guarded(Job::Like { subject }, |_| panic!("boom"));
        match event {
            Event::Liked {
                post_uri,
                result: Err(e),
            } => {
                assert_eq!(post_uri, "at://did:plc:a/app.bsky.feed.post/1");
                assert!(e.message().contains("boom"), "{}", e.message());
            }
            other => panic!("{other:?}"),
        }
        let event = guarded(Job::Timeline, |_| panic!("{}", String::from("owned")));
        assert!(
            matches!(&event, Event::Timeline(Err(e)) if e.message().contains("owned")),
            "{event:?}"
        );
    }

    #[test]
    fn a_job_that_does_not_panic_is_answered_as_it_ran() {
        let event = guarded(Job::Timeline, |_| Event::Timeline(Ok(Page::from(vec![]))));
        assert!(matches!(event, Event::Timeline(Ok(_))), "{event:?}");
    }

    #[test]
    fn a_write_thread_keeps_running_after_a_job_panics() {
        let (tx, rx) = channel::<Job>();
        let (done_tx, done_rx) = channel::<Event>();
        let handle = thread::spawn(move || {
            for job in rx {
                let event = guarded(job, |job| match job {
                    Job::Follow { .. } => panic!("boom"),
                    _ => Event::Seen(Ok(())),
                });
                done_tx.send(event).unwrap();
            }
        });
        tx.send(Job::Follow {
            did: "did:plc:b".into(),
        })
        .unwrap();
        tx.send(Job::UpdateSeen("now".into())).unwrap();
        drop(tx);
        assert!(matches!(
            done_rx.recv().unwrap(),
            Event::Followed { result: Err(_), .. }
        ));
        assert!(matches!(done_rx.recv().unwrap(), Event::Seen(Ok(()))));
        handle.join().unwrap();
    }

    /// A stand-in server that answers createSession for did:plc:b and
    /// anything else with `{}`, and keeps the path and the token of each
    /// call.
    type Calls = Arc<Mutex<Vec<(String, String)>>>;

    fn serve() -> (String, Calls) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let log: Calls = Arc::default();
        let kept = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut r = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                let path = line.split(' ').nth(1).unwrap_or("").to_string();
                let (mut auth, mut len) = (String::new(), 0);
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    let (name, value) = h.split_once(':').unwrap_or_default();
                    match name.to_ascii_lowercase().as_str() {
                        "authorization" => auth = value.trim().to_string(),
                        "content-length" => len = value.trim().parse().unwrap(),
                        _ => {}
                    }
                }
                r.read_exact(&mut vec![0; len]).unwrap();
                let body = if path.contains("createSession") {
                    r#"{"did":"did:plc:b","handle":"b.test","accessJwt":"B-access","refreshJwt":"B-refresh"}"#
                } else {
                    "{}"
                };
                kept.lock().unwrap().push((path, auth));
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (url, log)
    }

    fn answer(worker: &Worker, seq: u64) -> Event {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some((s, ev)) = worker.try_recv()
                && s == seq
            {
                return ev;
            }
            assert!(Instant::now() < deadline, "no answer to {seq}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    // Adding an account logs it in on the worker, but the account in use
    // changes only when the UI takes the answer: a job the UI sends before
    // that (a read receipt for what it shows) acts as the account shown.
    #[test]
    fn a_login_does_not_change_the_account_jobs_act_as_until_the_ui_takes_it() {
        let (url, log) = serve();
        let dir = tempfile::tempdir().unwrap();
        let accounts = AccountStore::open(dir.path());
        let a = Session {
            service: url.clone(),
            did: "did:plc:a".into(),
            handle: "a.test".into(),
            access_jwt: "A-access".into(),
            refresh_jwt: "A-refresh".into(),
        };
        accounts.save(&a).unwrap();
        let worker = Worker::spawn(Some(a), accounts);
        worker.send(
            1,
            Job::Login {
                service: url,
                identifier: "b.test".into(),
                password: "app-pass".into(),
            },
        );
        let Event::LoggedIn(Ok(b)) = answer(&worker, 1) else {
            panic!("the login failed")
        };
        worker.send(2, Job::UpdateSeen("2026-09-24T00:00:00.000Z".into()));
        answer(&worker, 2);
        worker.use_account(b);
        worker.send(3, Job::UpdateSeen("2026-09-24T00:00:00.000Z".into()));
        answer(&worker, 3);
        let seen: Vec<String> = log
            .lock()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.contains("updateSeen"))
            .map(|(_, auth)| auth.clone())
            .collect();
        assert_eq!(seen, ["Bearer A-access", "Bearer B-access"]);
    }

    #[test]
    fn seen_is_the_newest_time_whatever_the_order_or_offset() {
        let times = [
            "2026-09-22T00:30:00.000Z",
            "2026-09-22T10:00:00+09:00",
            "not a time",
            "2026-09-22T00:59:59.999Z",
        ];
        assert_eq!(
            newest(times.into_iter()).as_deref(),
            Some("2026-09-22T10:00:00+09:00")
        );
        assert_eq!(newest(std::iter::empty()), None);
    }

    #[test]
    fn any_picture_becomes_a_png_or_jpeg_avatar_and_text_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let webp = dir.path().join("me.webp");
        image::RgbImage::from_pixel(20, 10, image::Rgb([1, 2, 3]))
            .save(&webp)
            .unwrap();
        let (bytes, mime) = read_avatar(&webp).unwrap();
        assert!(mime == "image/png" || mime == "image/jpeg", "{mime}");
        assert_eq!(
            image::guess_format(&bytes).map(|f| f.to_mime_type()).ok(),
            Some(mime.as_str())
        );
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "not an image").unwrap();
        let err = read_avatar(&text).unwrap_err();
        assert!(
            err.message().contains("is not a picture bsky can read"),
            "{err}"
        );
    }

    #[test]
    fn downloads_are_named_after_the_media() {
        let img = |url: &str| Media::Image {
            url: url.into(),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(
            download_name(&img(
                "https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:x/bafkreiabc@jpeg"
            )),
            "bafkreiabc.jpg"
        );
        assert_eq!(
            download_name(&img("http://127.0.0.1:1/img/full1.png")),
            "full1.png"
        );
        let video = Media::Video {
            playlist: "https://video.bsky.app/watch/did%3Aplc%3Ax/bafkreivid/playlist.m3u8".into(),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&video), "bafkreivid.ts");
    }

    fn encoded(format: image::ImageFormat) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut out, format)
            .unwrap();
        out.into_inner()
    }

    // A picture is saved with the extension of what it is, not of what its
    // address says: a post's picture at an address ending in .exe was
    // saved as an .exe, with whatever bytes the server sent.
    #[rstest::rstest]
    #[case::exe_that_is_a_png("setup.exe", image::ImageFormat::Png, "setup.png")]
    #[case::jpeg_named_png("photo.png", image::ImageFormat::Jpeg, "photo.jpg")]
    #[case::gif("anim.jpg", image::ImageFormat::Gif, "anim.gif")]
    #[case::webp("bafkreiabc.jpg", image::ImageFormat::WebP, "bafkreiabc.webp")]
    #[case::no_dot("picture", image::ImageFormat::Png, "picture.png")]
    fn a_picture_is_saved_as_what_it_is(
        #[case] name: &str,
        #[case] format: image::ImageFormat,
        #[case] want: &str,
    ) {
        assert_eq!(picture_name(name, &encoded(format)).unwrap(), want);
    }

    #[rstest::rstest]
    #[case::program(b"MZ\x90\x00 a program".as_slice())]
    #[case::script(b"#!/bin/sh\nrm -rf ~\n".as_slice())]
    #[case::html(b"<html>not found</html>".as_slice())]
    #[case::empty(b"".as_slice())]
    fn what_is_not_a_picture_is_not_saved(#[case] bytes: &[u8]) {
        let err = picture_name("setup.exe", bytes).unwrap_err();
        assert!(err.message().contains("is not a picture"), "{err}");
    }

    #[rstest::rstest]
    #[case::reserved_on_windows("https://cdn.test/plain/did:plc:x/CON@jpeg", "picture.jpg")]
    #[case::reserved_with_a_dot("https://cdn.test/x/nul.", "picture.jpg")]
    #[case::no_extension("https://cdn.test/plain/did:plc:x/abc@", "abc.jpg")]
    #[case::only_dots("https://cdn.test/plain/did:plc:x/..@..", "picture.jpg")]
    #[case::fragment("https://cdn.test/plain/did:plc:x/abc@png#a/b", "abc.png")]
    #[case::emoji_and_cjk("https://cdn.test/x/写真👨‍👩‍👧.png", "picture.png")]
    fn a_download_name_is_safe_on_every_system(#[case] url: &str, #[case] want: &str) {
        let media = Media::Image {
            url: url.into(),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&media), want);
    }

    #[rstest::rstest]
    #[case::reserved("https://video.test/watch/did/COM1/playlist.m3u8", "video.ts")]
    #[case::dot_dot("https://video.test/watch/did/../playlist.m3u8", "video.ts")]
    #[case::fragment("https://video.test/watch/did/cid/playlist.m3u8#a/b", "cid.ts")]
    fn a_video_download_name_is_safe_on_every_system(#[case] url: &str, #[case] want: &str) {
        let media = Media::Video {
            playlist: url.into(),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&media), want);
    }

    // A name already taken by a link, even one to nothing, is not written
    // through: that would create or replace the file it points to.
    #[cfg(unix)]
    #[test]
    fn a_download_never_writes_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.txt");
        let downloads = dir.path().join("dl");
        std::fs::create_dir(&downloads).unwrap();
        std::os::unix::fs::symlink(&outside, downloads.join("a.jpg")).unwrap();
        let saved = save_new(&downloads, "a.jpg", b"picture").unwrap();
        assert_eq!(saved, downloads.join("a (1).jpg"));
        assert!(!outside.exists(), "wrote through the link");
        assert_eq!(std::fs::read(saved).unwrap(), b"picture");
    }

    // A name longer than a file system allows (255 bytes on most) failed
    // the download with "File name too long" after the whole file had come.
    #[test]
    fn a_long_address_still_names_a_file_that_can_be_written() {
        let stem = "a".repeat(300);
        let media = Media::Image {
            url: format!("https://cdn.test/plain/did:plc:x/{stem}@jpeg"),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        let name = download_name(&media);
        assert!(name.len() <= 128, "{} bytes", name.len());
        assert!(name.ends_with(".jpg") && name.starts_with("aaaa"), "{name}");
        let video = Media::Video {
            playlist: format!("https://video.test/watch/did/{stem}/playlist.m3u8"),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert!(download_name(&video).len() <= 128);
        let dir = tempfile::tempdir().unwrap();
        save_new(dir.path(), &name, b"x").unwrap();
        let again = save_new(dir.path(), &name, b"x").unwrap();
        assert!(again.to_string_lossy().ends_with(" (1).jpg"));
    }

    // The size limit held for each segment but not for the video: a playlist
    // of many segments was kept in memory whole, however large.
    #[test]
    fn a_video_download_stops_once_its_segments_pass_the_limit() {
        let segments = ["a", "b", "c", "d"].map(String::from);
        let mut fetched = Vec::new();
        let got = join_segments(segments.clone(), 15, |url| {
            fetched.push(url.to_string());
            Ok(vec![0; 10])
        });
        let e = got.unwrap_err();
        assert!(e.message().contains("MB"), "{}", e.message());
        assert_eq!(fetched, ["a", "b"], "fetched after the limit was passed");

        let all = join_segments(segments.clone(), 40, |_| Ok(vec![1; 10])).unwrap();
        assert_eq!(all.len(), 40);

        let e = join_segments(segments, 40, |url| {
            if url == "c" {
                Err(Error::api("gone"))
            } else {
                Ok(vec![1; 10])
            }
        })
        .unwrap_err();
        assert_eq!(e.message(), "gone");
    }

    // A write that fails part way (a full disk) left a cut file under the
    // download's name, which looked saved and took the name from a retry.
    #[test]
    fn a_download_that_cannot_be_written_whole_leaves_no_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let err = save_new_with(dir.path(), "a.jpg", |f| {
            f.write_all(b"half of it")?;
            Err(std::io::Error::other("no space left on device"))
        })
        .unwrap_err();
        assert!(err.message().contains("no space left"), "{err}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(
            save_new(dir.path(), "a.jpg", b"whole").unwrap(),
            dir.path().join("a.jpg")
        );
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), "old").unwrap();
        std::fs::write(dir.path().join("a (1).jpg"), "old").unwrap();
        let saved = save_new(dir.path(), "a.jpg", b"new").unwrap();
        assert_eq!(saved, dir.path().join("a (2).jpg"));
        assert_eq!(std::fs::read(dir.path().join("a.jpg")).unwrap(), b"old");
        let first = save_new(dir.path(), "b", b"x").unwrap();
        assert_eq!(first, dir.path().join("b"));
        assert_eq!(
            save_new(dir.path(), "b", b"x").unwrap(),
            dir.path().join("b (1)")
        );
    }

    #[test]
    fn missing_avatar_file_is_an_io_error() {
        let err = read_avatar(std::path::Path::new("/definitely/not/here.png")).unwrap_err();
        assert_eq!(err.kind(), crate::error::Kind::Io);
    }
}
