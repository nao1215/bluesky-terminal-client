//! Network calls run on a worker thread so the UI never blocks on the server.
//!
//! The UI sends a [`Job`]; the worker performs it with the [`Client`] it owns
//! and answers with an [`Event`]. Jobs run one at a time, in order, which
//! keeps a like followed by an unlike from racing each other.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::api::types::{Notification, Post, Profile, Record, ReplyRef, StrongRef, ThreadNode};
use crate::api::{self, Client, MAX_AVATAR_BYTES, ProfileEdit};
use crate::config::{Session, SessionStore};
use crate::error::{Error, Result};
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
    },
    /// Load the fields the profile editor starts from.
    LoadProfileEditor,
    SaveProfile {
        display_name: String,
        description: String,
        /// Path to a new avatar image; empty keeps the current one.
        avatar_path: String,
    },
}

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
    SearchPosts(Result<Page<Post>>),
    SearchActors(Result<Page<Profile>>),
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
    /// The first page of notifications, fetched at `seen_at`.
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
    ProfileEditor(Result<ProfileFields>),
    ProfileSaved(Result<()>),
}

/// Handle to the running worker.
pub struct Worker {
    tx: Sender<Job>,
    rx: Receiver<Event>,
}

impl Worker {
    /// Start the worker. `session` is the saved login, if any.
    pub fn spawn(session: Option<Session>, store: SessionStore) -> Self {
        let (job_tx, job_rx) = channel::<Job>();
        let (ev_tx, ev_rx) = channel::<Event>();
        thread::spawn(move || {
            let mut state = State {
                client: session.map(|s| Client::new(s, Some(store.clone()))),
                store,
                editor_base: None,
            };
            for job in job_rx {
                let event = state.run(job);
                if ev_tx.send(event).is_err() {
                    break;
                }
            }
        });
        Self {
            tx: job_tx,
            rx: ev_rx,
        }
    }

    /// Queue a job.
    pub fn send(&self, job: Job) {
        // The worker only stops when the UI drops it, so a send cannot fail
        // while the UI is still running.
        let _ = self.tx.send(job);
    }

    /// A finished job, if any.
    pub fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

struct State {
    client: Option<Client>,
    store: SessionStore,
    /// The profile record the open editor was filled from (`Some(None)` when
    /// the account has none yet); saving edits exactly this version.
    editor_base: Option<Option<Record>>,
}

impl State {
    fn client(&mut self) -> Result<&mut Client> {
        self.client
            .as_mut()
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
            Job::SearchPosts(q) => Event::SearchPosts(self.search_posts(&q, None)),
            Job::SearchActors(q) => Event::SearchActors(self.search_actors(&q, None)),
            Job::OpenProfile(actor) => Event::Profile(self.open_profile(&actor)),
            Job::Notifications => {
                // Everything up to the moment of the request is what the user
                // is about to see; later notifications stay unread.
                let seen_at = api::now();
                Event::Notifications {
                    result: self.notifications(None),
                    seen_at,
                }
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
            Job::Post { text, reply } => Event::Posted {
                reply_to: reply.as_ref().map(|r| r.parent.uri.clone()),
                result: self
                    .client()
                    .and_then(|c| c.create_post(&text, reply.as_ref()))
                    .map(|_| ()),
            },
            Job::LoadProfileEditor => Event::ProfileEditor(self.profile_fields()),
            Job::SaveProfile {
                display_name,
                description,
                avatar_path,
            } => Event::ProfileSaved(self.save_profile(display_name, description, &avatar_path)),
        }
    }

    fn login(&mut self, service: &str, identifier: &str, password: &str) -> Result<Session> {
        let session = api::login(service, identifier, password)?;
        self.store.save(&session)?;
        self.client = Some(Client::new(session.clone(), Some(self.store.clone())));
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
        let did = client.session().did.clone();
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
        let r = self.client()?.author_feed(did, cursor)?;
        Ok(Page {
            items: r
                .feed
                .into_iter()
                .filter(|i| i.reason.is_none())
                .map(|i| i.post)
                .collect(),
            cursor: r.cursor,
        })
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

    fn open_profile(&mut self, actor: &str) -> Result<(Profile, Page<Post>)> {
        let profile = self.client()?.profile(actor)?;
        let posts = self.author_feed(&profile.did, None)?;
        Ok((profile, posts))
    }

    fn more(&mut self, feed: &Feed, cursor: &str) -> Result<MorePage> {
        let c = Some(cursor);
        Ok(match feed {
            Feed::Timeline => MorePage::Posts(self.timeline(c)?),
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

    fn save_profile(
        &mut self,
        display_name: String,
        description: String,
        avatar_path: &str,
    ) -> Result<()> {
        let avatar = read_avatar(avatar_path)?;
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

/// Read and check a new avatar file; an empty path means "keep the current one".
fn read_avatar(path: &str) -> Result<Option<(Vec<u8>, String)>> {
    let path = path.trim();
    if path.is_empty() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| Error::io(format!("cannot read {path}: {e}")))?;
    let mime = api::sniff_image_mime(&bytes)
        .filter(|m| *m == "image/png" || *m == "image/jpeg")
        .ok_or_else(|| Error::io(format!("{path} is not a PNG or JPEG image")))?;
    if bytes.len() > MAX_AVATAR_BYTES {
        return Err(Error::io(format!(
            "{path} is {} bytes; avatars must be at most {MAX_AVATAR_BYTES} bytes",
            bytes.len()
        )));
    }
    Ok(Some((bytes, mime.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_avatar_path_keeps_the_avatar() {
        assert_eq!(read_avatar("  ").unwrap(), None);
    }

    #[test]
    fn avatar_must_be_png_or_jpeg() {
        let dir = tempfile::tempdir().unwrap();
        let gif = dir.path().join("a.gif");
        std::fs::write(&gif, b"GIF89a....").unwrap();
        let err = read_avatar(gif.to_str().unwrap()).unwrap_err();
        assert!(err.message().contains("PNG or JPEG"), "{err}");

        let png = dir.path().join("a.png");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        let (bytes, mime) = read_avatar(png.to_str().unwrap()).unwrap().unwrap();
        assert_eq!((bytes.len(), mime.as_str()), (8, "image/png"));
    }

    #[test]
    fn oversized_avatar_is_refused_before_upload() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.png");
        let mut data = b"\x89PNG\r\n\x1a\n".to_vec();
        data.resize(MAX_AVATAR_BYTES + 1, 0);
        std::fs::write(&big, data).unwrap();
        let err = read_avatar(big.to_str().unwrap()).unwrap_err();
        assert!(err.message().contains("at most"), "{err}");
    }

    #[test]
    fn missing_avatar_file_is_an_io_error() {
        let err = read_avatar("/definitely/not/here.png").unwrap_err();
        assert_eq!(err.kind(), crate::error::Kind::Io);
    }
}
