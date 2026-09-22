//! Network calls run on a worker thread so the UI never blocks on the server.
//!
//! The UI sends a [`Job`]; the worker performs it with the [`Client`] it owns
//! and answers with an [`Event`]. Jobs run one at a time, in order, which
//! keeps a like followed by an unlike from racing each other.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use crate::api::types::{Post, Profile, Record, ReplyRef, StrongRef};
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
    Like {
        subject: StrongRef,
    },
    Unlike {
        post_uri: String,
        like_uri: String,
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
    Timeline(Result<Vec<Post>>),
    SearchPosts(Result<Vec<Post>>),
    SearchActors(Result<Vec<Profile>>),
    Profile(Result<(Profile, Vec<Post>)>),
    Liked {
        post_uri: String,
        result: Result<String>,
    },
    Unliked {
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
            Job::Timeline => Event::Timeline(self.timeline()),
            Job::SearchPosts(q) => Event::SearchPosts(
                self.client()
                    .and_then(|c| c.search_posts(&q))
                    .map(|r| r.posts),
            ),
            Job::SearchActors(q) => Event::SearchActors(
                self.client()
                    .and_then(|c| c.search_actors(&q))
                    .map(|r| r.actors),
            ),
            Job::OpenProfile(actor) => Event::Profile(self.open_profile(&actor)),
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

    fn timeline(&mut self) -> Result<Vec<Post>> {
        let client = self.client()?;
        let did = client.session().did.clone();
        Ok(timeline::followed_posts(client.timeline(None)?.feed, &did))
    }

    fn open_profile(&mut self, actor: &str) -> Result<(Profile, Vec<Post>)> {
        let client = self.client()?;
        let profile = client.profile(actor)?;
        let posts = client
            .author_feed(&profile.did)?
            .feed
            .into_iter()
            .filter(|i| i.reason.is_none())
            .map(|i| i.post)
            .collect();
        Ok((profile, posts))
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
