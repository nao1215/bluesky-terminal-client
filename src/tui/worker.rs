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
    FeedInfo, Media, Notification, Post, Profile, Record, ReplyRef, StrongRef, ThreadNode,
};
use crate::api::{self, Client, MAX_AVATAR_BYTES, ProfileEdit};
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
    /// The first page of the custom feed at this URI.
    CustomFeed(String),
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
    SaveProfile {
        display_name: String,
        description: String,
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
    /// The first page of the custom feed `uri`.
    CustomFeed {
        uri: String,
        result: Result<Page<Post>>,
    },
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
                | Job::CustomFeed(_)
                | Job::SearchPosts(_)
                | Job::SearchActors(_)
                | Job::OpenProfile(_)
                | Job::Thread(_)
                | Job::Notifications
                | Job::More { .. }
                | Job::Download { .. }
                | Job::OpenLink { .. }
        )
    }
}

/// Threads that run reads. The start asks for three things at once (the
/// timeline, the notifications, the pinned feeds); one more keeps a thread
/// the user opens meanwhile from waiting behind them.
const READERS: usize = 4;

/// Handle to the running worker.
pub struct Worker {
    /// The account every job acts as, shared by every thread.
    client: Arc<Mutex<Option<Client>>>,
    accounts: AccountStore,
    writes: Sender<(u64, Job)>,
    reads: Sender<(u64, Job)>,
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
        let (writes, write_rx) = channel::<(u64, Job)>();
        let mut state = State {
            client: Arc::clone(&client),
            accounts: accounts.clone(),
            editor_base: None,
        };
        let events = ev_tx.clone();
        thread::spawn(move || {
            for (seq, job) in write_rx {
                if events.send((seq, state.run(job))).is_err() {
                    break;
                }
            }
        });
        let (reads, read_rx) = channel::<(u64, Job)>();
        let read_rx = Arc::new(Mutex::new(read_rx));
        for _ in 0..READERS {
            let mut state = State {
                client: Arc::clone(&client),
                accounts: accounts.clone(),
                editor_base: None,
            };
            let jobs = Arc::clone(&read_rx);
            let events = ev_tx.clone();
            thread::spawn(move || {
                loop {
                    // Held only while waiting for a job, not while running it.
                    let job = jobs.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok((seq, job)) = job else { break };
                    if events.send((seq, state.run(job))).is_err() {
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

    /// Queue a job; its answer comes back with the same `seq`.
    pub fn send(&self, seq: u64, job: Job) {
        // The worker only stops when the UI drops it, so a send cannot fail
        // while the UI is still running.
        let lane = if job.reads() {
            &self.reads
        } else {
            &self.writes
        };
        let _ = lane.send((seq, job));
    }

    /// A finished job and the `seq` it was sent with, if any.
    pub fn try_recv(&self) -> Option<(u64, Event)> {
        self.rx.try_recv().ok()
    }
}

/// The latest of some RFC 3339 timestamps, as it was written. Ones that do
/// not parse are skipped.
fn newest<'a>(times: impl Iterator<Item = &'a str>) -> Option<String> {
    times
        .filter_map(|t| chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| (d, t)))
        .max_by_key(|(d, _)| *d)
        .map(|(_, t)| t.to_string())
}

struct State {
    /// Shared by every thread; a login replaces it for all of them.
    client: Arc<Mutex<Option<Client>>>,
    accounts: AccountStore,
    /// The profile record the open editor was filled from (`Some(None)` when
    /// the account has none yet); saving edits exactly this version.
    editor_base: Option<Option<Record>>,
}

impl State {
    fn client(&self) -> Result<Client> {
        self.client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| Error::api("not logged in"))
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
            Job::CustomFeed(uri) => Event::CustomFeed {
                result: self.custom_feed(&uri, None),
                uri,
            },
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
                result: self.more(&feed, &cursor),
                feed,
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

    fn login(&mut self, service: &str, identifier: &str, password: &str) -> Result<Session> {
        let session = api::login(service, identifier, password)?;
        self.accounts.save(&session)?;
        *self.client.lock().unwrap_or_else(PoisonError::into_inner) = Some(Client::new(
            session.clone(),
            Some(self.accounts.store_for(&session.did)),
        ));
        Ok(session)
    }

    /// A page of the timeline, filtered to posts by followed accounts.
    ///
    /// A raw page can be all reposts and own posts, which filters down to
    /// nothing; then the next raw pages are read (a few at most) so that the
    /// user gets posts, not an empty page that ends the scrolling.
    fn timeline(&mut self, cursor: Option<&str>) -> Result<Page<Post>> {
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
    fn custom_feed(&mut self, uri: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        let raw = self.client()?.feed(uri, cursor)?;
        Ok(Page {
            items: timeline::feed_posts(raw.feed),
            cursor: raw.cursor,
        })
    }

    fn search_posts(&mut self, q: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        let r = self.client()?.search_posts(q, cursor)?;
        Ok(Page {
            items: r.posts,
            cursor: r.cursor,
        })
    }

    fn search_actors(&mut self, q: &str, cursor: Option<&str>) -> Result<Page<Profile>> {
        let r = self.client()?.search_actors(q, cursor)?;
        Ok(Page {
            items: r.actors,
            cursor: r.cursor,
        })
    }

    /// An account's own posts: reposts are left out, as on the timeline.
    fn author_feed(&mut self, did: &str, cursor: Option<&str>) -> Result<Page<Post>> {
        Ok(author_page(self.client()?.author_feed(did, cursor)?))
    }

    /// A page of notifications, joined with the posts they are about.
    fn notifications(&mut self, cursor: Option<&str>) -> Result<Page<NotifItem>> {
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
    fn open_profile(&mut self, actor: &str) -> Result<(Profile, Page<Post>)> {
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

    fn more(&mut self, feed: &Feed, cursor: &str) -> Result<MorePage> {
        let c = Some(cursor);
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
        &mut self,
        text: &str,
        reply: Option<&ReplyRef>,
        quote: Option<&StrongRef>,
        media: &[Attachment],
        video_service: &str,
    ) -> Result<()> {
        let client = self.client()?;
        crate::compose::send_post(&client, text, reply, quote, media, video_service).map(|_| ())
    }

    fn save_profile(
        &mut self,
        display_name: String,
        description: String,
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
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| Error::api(format!("cannot download {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::api(format!(
            "cannot download {url}: HTTP {}",
            resp.status().as_u16()
        )));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_vec()
        .map_err(|e| Error::api(format!("cannot download {url}: {e}")))
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
    let stem = clean_name(stem);
    let stem = if stem.is_empty() || is_reserved_name(&stem) {
        fallback.to_string()
    } else {
        stem
    };
    format!("{stem}.{ext}")
}

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
        Error::io("there is no download folder")
            .with_hint("choose one on the settings screen (s on your Profile tab)")
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
            let mut all = Vec::new();
            for seg in crate::hls::segments(&media_text, &media_url) {
                all.extend(fetch_bytes(&agent, &seg)?);
            }
            if all.is_empty() {
                return Err(Error::api("the video's playlist lists nothing to download"));
            }
            all
        }
    };
    let name = match media {
        Media::Image { .. } => picture_name(&download_name(media), &bytes)?,
        Media::Video { .. } => download_name(media),
    };
    std::fs::create_dir_all(dir)
        .map_err(|e| Error::io(format!("cannot create {}: {e}", dir.display())))?;
    save_new(dir, &name, &bytes)
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
            return Err(Error::api(format!(
                "{name} is not a picture bsky can save; nothing was written"
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
    for i in 0..10_000 {
        let path = candidate_path(dir, name, i);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::io(format!("cannot write {}: {e}", path.display()))),
        };
        file.write_all(bytes)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
        return Ok(path);
    }
    Err(Error::io(format!(
        "{} has too many files named like {name}",
        dir.display()
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
        assert_eq!(api::sniff_image_mime(&bytes), Some(mime.as_str()));
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
