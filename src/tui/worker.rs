//! Network calls run on a worker thread so the UI never blocks on the server.
//!
//! The UI sends a [`Job`]; the worker performs it with the [`Client`] it owns
//! and answers with an [`Event`]. Jobs run one at a time, in order, which
//! keeps a like followed by an unlike from racing each other.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::api::types::{
    FeedInfo, Media, Notification, Post, Profile, Record, ReplyRef, StrongRef, ThreadNode,
};
use crate::api::{self, Client, MAX_AVATAR_BYTES, PostImage, PostMedia, PostVideo, ProfileEdit};
use crate::config::{Session, SessionStore};
use crate::error::{Error, Result};
use crate::media;
use crate::timeline;
use crate::video;

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
        /// Pictures, or one video.
        media: Vec<Attachment>,
    },
    /// Load the fields the profile editor starts from.
    LoadProfileEditor,
    /// Save a picture or video of a post in the download folder.
    Download(Media),
    /// Open a link in the web browser.
    OpenLink(String),
    SaveProfile {
        display_name: String,
        description: String,
        /// A new avatar picture; `None` keeps the current one.
        avatar: Option<PathBuf>,
    },
}

/// A picture or video on the user's disk to attach to a post.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub path: PathBuf,
    /// Text describing it for people who cannot see it.
    pub alt: String,
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
    ProfileEditor(Result<ProfileFields>),
    ProfileSaved(Result<()>),
    /// Where the download was saved.
    Downloaded(Result<PathBuf>),
    Opened {
        url: String,
        result: Result<()>,
    },
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

/// The latest of some RFC 3339 timestamps, as it was written. Ones that do
/// not parse are skipped.
fn newest<'a>(times: impl Iterator<Item = &'a str>) -> Option<String> {
    times
        .filter_map(|t| chrono::DateTime::parse_from_rfc3339(t).ok().map(|d| (d, t)))
        .max_by_key(|(d, _)| *d)
        .map(|(_, t)| t.to_string())
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
            Job::PinnedFeeds => Event::PinnedFeeds(self.client().and_then(Client::pinned_feeds)),
            Job::CustomFeed(uri) => Event::CustomFeed {
                result: self.custom_feed(&uri, None),
                uri,
            },
            Job::SearchPosts(q) => Event::SearchPosts(self.search_posts(&q, None)),
            Job::SearchActors(q) => Event::SearchActors(self.search_actors(&q, None)),
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
            Job::Post { text, reply, media } => Event::Posted {
                reply_to: reply.as_ref().map(|r| r.parent.uri.clone()),
                result: self.post(&text, reply.as_ref(), &media),
            },
            Job::LoadProfileEditor => Event::ProfileEditor(self.profile_fields()),
            Job::Download(media) => Event::Downloaded(download(&media)),
            Job::OpenLink(url) => Event::Opened {
                result: crate::browser::open(&url),
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
    fn post(&mut self, text: &str, reply: Option<&ReplyRef>, media: &[Attachment]) -> Result<()> {
        let videos = media
            .iter()
            .filter(|a| media::inspect(&a.path).kind == media::Kind::Video)
            .count();
        let embed = match (media.len(), videos) {
            (0, _) => PostMedia::None,
            (1, 1) => {
                let a = &media[0];
                let v = video::prepare(&a.path)?;
                let name = a
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "video".into());
                let service = crate::config::video_service();
                let blob = self.client()?.upload_video(
                    &service,
                    &v.bytes,
                    v.mime,
                    &name,
                    std::time::Duration::from_secs(1),
                )?;
                PostMedia::Video(PostVideo {
                    blob,
                    alt: a.alt.trim().to_string(),
                    dims: v.dims,
                })
            }
            (_, 0) => {
                let prepared = media
                    .iter()
                    .map(|a| media::prepare(&a.path))
                    .collect::<Result<Vec<_>>>()?;
                let mut uploaded = Vec::with_capacity(media.len());
                for (a, p) in media.iter().zip(prepared) {
                    let blob = self.client()?.upload_blob(&p.bytes, p.mime)?;
                    uploaded.push(PostImage {
                        blob,
                        alt: a.alt.trim().to_string(),
                        width: p.width,
                        height: p.height,
                    });
                }
                PostMedia::Images(uploaded)
            }
            _ => {
                return Err(Error::new(
                    crate::error::Kind::Usage,
                    "a post can have up to 4 pictures or one video, not both",
                ));
            }
        };
        self.client()?.create_post(text, reply, &embed)?;
        Ok(())
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
/// `.../watch/<did>/<cid>/playlist.m3u8`.
fn download_name(media: &Media) -> String {
    let clean = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect::<String>()
    };
    match media {
        Media::Image { url, .. } => {
            let last = url
                .split('?')
                .next()
                .unwrap_or(url)
                .rsplit('/')
                .next()
                .unwrap_or("");
            let (stem, ext) = match last.split_once('@') {
                Some((stem, ext)) => (stem, ext),
                None => last.rsplit_once('.').unwrap_or((last, "jpg")),
            };
            let ext = if ext == "jpeg" { "jpg" } else { ext };
            let stem = clean(stem);
            let stem = if stem.is_empty() {
                "picture".into()
            } else {
                stem
            };
            format!("{stem}.{}", clean(ext))
        }
        Media::Video { playlist, .. } => {
            let path = playlist.split('?').next().unwrap_or(playlist);
            let stem = path.rsplit('/').nth(1).map(clean).unwrap_or_default();
            let stem = if stem.is_empty() {
                "video".into()
            } else {
                stem
            };
            format!("{stem}.ts")
        }
    }
}

/// A path in `dir` for `name` that is not taken: `name`, then `name (1)`...
fn free_path(dir: &std::path::Path, name: &str) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    (0..)
        .map(|i| {
            let n = match (i, ext.is_empty()) {
                (0, _) => name.to_string(),
                (i, true) => format!("{stem} ({i})"),
                (i, false) => format!("{stem} ({i}).{ext}"),
            };
            dir.join(n)
        })
        .find(|p| !p.exists())
        .expect("some name is free")
}

/// Save a picture (full size) or a video (its best variant, the segments
/// joined into one MPEG transport stream, which players play as it is).
fn download(media: &Media) -> Result<PathBuf> {
    let dir = crate::config::download_dir()
        .ok_or_else(|| Error::io("there is no download folder; set BSKY_DOWNLOAD_DIR"))?;
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
    std::fs::create_dir_all(&dir)
        .map_err(|e| Error::io(format!("cannot create {}: {e}", dir.display())))?;
    let path = free_path(&dir, &download_name(media));
    std::fs::write(&path, bytes)
        .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
    Ok(path)
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

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(free_path(dir.path(), "a.jpg"), dir.path().join("a.jpg"));
        std::fs::write(dir.path().join("a.jpg"), "").unwrap();
        std::fs::write(dir.path().join("a (1).jpg"), "").unwrap();
        assert_eq!(free_path(dir.path(), "a.jpg"), dir.path().join("a (2).jpg"));
    }

    #[test]
    fn missing_avatar_file_is_an_io_error() {
        let err = read_avatar(std::path::Path::new("/definitely/not/here.png")).unwrap_err();
        assert_eq!(err.kind(), crate::error::Kind::Io);
    }
}
