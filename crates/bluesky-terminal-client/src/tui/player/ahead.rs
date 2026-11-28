//! Bounded HLS startup bytes for the selected post and its two neighbours.
//! No decoder or frame is created until the user selects a video.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use crate::hls;

const KEPT: usize = 3;
const PLAYLIST_BYTES: usize = 32 * 1024;
const SEGMENT_BYTES: usize = 1024 * 1024;
const URL_BYTES: usize = 8 * 1024;
const FRESH: Duration = Duration::from_secs(300);

pub(super) struct Startup {
    pub(super) media_url: String,
    pub(super) media: String,
    pub(super) first: Option<Vec<u8>>,
}

/// An in-flight startup ceased to be wanted. Playback must not turn its
/// eviction into a fresh, uncancellable download during a selection change.
#[derive(Debug)]
pub(super) struct Cancelled;

struct Job {
    url: String,
    // A separate connection pool lets eviction interrupt this job's socket
    // without shutting down either playback or another video's read-ahead.
    agent: ureq::Agent,
    cancelled: AtomicBool,
}

impl Job {
    fn cancel(&self) {
        if !self.cancelled.swap(true, Ordering::Relaxed) {
            self.agent.clone().shutdown();
        }
    }

    fn bytes(&self, url: &str, limit: usize) -> Option<Vec<u8>> {
        if self.cancelled.load(Ordering::Relaxed) || url.len() > URL_BYTES {
            return None;
        }
        let mut response = self.agent.get(url).call().ok()?;
        if !response.status().is_success()
            || response
                .body()
                .content_length()
                .is_some_and(|n| n > limit as u64)
        {
            return None;
        }
        let mut reader = response.body_mut().as_reader();
        // Reserve the cap once: read_to_end's geometric growth can allocate
        // twice the byte limit, even when the body is eventually rejected.
        let mut bytes = Vec::with_capacity(limit);
        let mut chunk = [0; 8192];
        loop {
            if self.cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let len = reader.read(&mut chunk).ok()?;
            if len == 0 {
                return Some(bytes);
            }
            if bytes.len() + len > limit {
                return None;
            }
            bytes.extend_from_slice(&chunk[..len]);
        }
    }

    fn read(&self) -> Option<Startup> {
        let master = String::from_utf8(self.bytes(&self.url, PLAYLIST_BYTES)?).ok()?;
        let (media_url, media) = match hls::pick_variant(&master, &self.url) {
            Some(url) => {
                let media = String::from_utf8(self.bytes(&url, PLAYLIST_BYTES)?).ok()?;
                (url, media)
            }
            None => (self.url.clone(), master),
        };
        let first_url = hls::first_segment(&media, &media_url)?;
        // A large segment still gets its playlists cached; playback streams
        // that segment normally, instead of allocating it all here.
        let first = self.bytes(&first_url, SEGMENT_BYTES);
        (!self.cancelled.load(Ordering::Relaxed)).then_some(Startup {
            media_url,
            media,
            first,
        })
    }
}

enum Reading {
    Now,
    Done(Option<Arc<Startup>>, Instant),
}

struct Entry {
    job: Arc<Job>,
    reading: Reading,
}

#[derive(Default)]
struct State {
    wanted: Vec<String>,
    entries: Vec<Entry>,
    closed: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn next(&self) -> Option<Arc<Job>> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            if state.entries.len() < KEPT
                && let Some(url) = state.wanted.iter().find(|url| {
                    !state
                        .entries
                        .iter()
                        .any(|e| e.job.url == **url && !e.job.cancelled.load(Ordering::Relaxed))
                })
            {
                let agent = ureq::Agent::config_builder()
                    .http_status_as_error(false)
                    .timeout_global(Some(Duration::from_secs(5)))
                    .timeout_resolve(Some(Duration::from_secs(2)))
                    .timeout_connect(Some(Duration::from_secs(2)))
                    .build()
                    .into();
                let job = Arc::new(Job {
                    url: url.clone(),
                    agent,
                    cancelled: AtomicBool::new(false),
                });
                state.entries.push(Entry {
                    job: Arc::clone(&job),
                    reading: Reading::Now,
                });
                return Some(job);
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn work(&self) {
        while let Some(job) = self.next() {
            let result = job.read().map(Arc::new);
            let mut state = self.lock();
            if let Some(i) = state.entries.iter().position(|e| Arc::ptr_eq(&e.job, &job)) {
                if job.cancelled.load(Ordering::Relaxed) {
                    drop(result);
                    state.entries.remove(i);
                } else {
                    state.entries[i].reading = Reading::Done(result, Instant::now());
                }
            }
            self.changed.notify_all();
        }
    }
}

struct Cache(Arc<Shared>);

impl Cache {
    fn new() -> Self {
        let cache = Self(Arc::new(Shared::default()));
        let mut workers = 0;
        for _ in 0..KEPT {
            let shared = Arc::clone(&cache.0);
            if crate::tui::spawn("bsky-video-ahead", move || shared.work()).is_ok() {
                workers += 1;
            }
        }
        cache.0.lock().closed = workers == 0;
        cache
    }

    fn update(&self, urls: &[String]) {
        let mut state = self.0.lock();
        let mut wanted = Vec::new();
        for url in urls {
            if url.len() <= URL_BYTES && !wanted.contains(url) {
                wanted.push(url.clone());
            }
            if wanted.len() == KEPT {
                break;
            }
        }
        state.entries.retain(|entry| {
            let fresh = match entry.reading {
                Reading::Now => true,
                Reading::Done(_, at) => at.elapsed() < FRESH,
            };
            if wanted.contains(&entry.job.url) && fresh {
                return true;
            }
            entry.job.cancel();
            // Keep a cancelled worker's slot until it releases its buffers:
            // rapid scrolling cannot accumulate detached megabyte downloads.
            matches!(entry.reading, Reading::Now)
        });
        state.wanted = wanted;
        self.0.changed.notify_all();
    }

    fn get(&self, url: &str, stop: &AtomicBool) -> Result<Option<Arc<Startup>>, Cancelled> {
        let mut state = self.0.lock();
        if state.closed || !state.wanted.iter().any(|u| u == url) {
            return Ok(None);
        }
        loop {
            if stop.load(Ordering::Relaxed)
                || state.closed
                || !state.wanted.iter().any(|u| u == url)
            {
                return Err(Cancelled);
            }
            if let Some(entry) = state
                .entries
                .iter()
                .find(|e| e.job.url == url && !e.job.cancelled.load(Ordering::Relaxed))
                && let Reading::Done(result, _) = &entry.reading
            {
                return Ok(result.clone());
            }
            // Playback may be stopped while it waits for an in-flight
            // read-ahead. Stop does not notify this cache's condition variable.
            state = self
                .0
                .changed
                .wait_timeout(state, Duration::from_millis(20))
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.closed = true;
        for entry in &state.entries {
            entry.job.cancel();
        }
        state.entries.clear();
        self.0.changed.notify_all();
    }
}

static CACHE: OnceLock<Cache> = OnceLock::new();

/// At most three startups, each with two 32 KiB playlists and a 1 MiB
/// segment. Cancelled jobs count against the same three slots until done.
/// Playback borrows the bytes through Arc rather than copying the cache.
pub(crate) fn read_ahead(urls: &[String]) {
    let cache = if urls.is_empty() {
        CACHE.get()
    } else {
        Some(CACHE.get_or_init(Cache::new))
    };
    if let Some(cache) = cache {
        cache.update(urls);
    }
}

pub(super) fn cached(url: &str, stop: &AtomicBool) -> Result<Option<Arc<Startup>>, Cancelled> {
    match CACHE.get() {
        Some(cache) => cache.get(url, stop),
        None => Ok(None),
    }
}

/// Cancel background work on every way out of the TUI, including I/O errors.
pub(crate) struct AheadSession;

impl Drop for AheadSession {
    fn drop(&mut self) {
        read_ahead(&[]);
    }
}

#[cfg(test)]
mod tests;
