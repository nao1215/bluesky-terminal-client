//! Inline images: download and encode in the background, draw when ready.
//!
//! A picture is fetched the first time a frame asks for it: from the disk
//! cache when it is there, from the network otherwise, or from the user's
//! disk for a `file://` key. Encoding it for the terminal (scaling to its box
//! and, for kitty, compressing) is the other expensive step, and it runs on
//! encoder threads too, once per (picture, box size). Until both are done the
//! box shows a placeholder, so the layout never jumps and scrolling never
//! waits for a picture.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use image::DynamicImage;
use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Image, Resize};

use crate::tui::player::{Player, State};

/// Largest image body bsky downloads.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Parallel downloads. Pictures are small and most of their time is spent
/// waiting on the network, so more at once than there are cores.
const LOADERS: usize = 8;
/// Parallel encoders.
const ENCODERS: usize = 2;

/// A decoded picture whose longest side is above this is shrunk to
/// [`SHRUNK_SIDE`] before it is kept. Shrinking costs about as much as
/// decoding again (35 ms for a 2000 x 1500 photo), and every encode scales
/// the picture to its box anyway, so a picture that is not much larger than
/// any box is kept as it is: Bluesky's full-size pictures are at most 2000
/// pixels, and keeping them unshrunk made a screen of twelve ready in 66 ms
/// instead of 109. A camera photo (4000 x 3000) is still shrunk, because
/// scaling it down once is cheaper than scaling from it for every box.
const SHRINK_ABOVE: u32 = 2048;
/// The longest side a picture larger than [`SHRINK_ABOVE`] is kept at. The
/// largest box bsky draws (the viewer across the whole screen) is under this
/// on common screens.
const SHRUNK_SIDE: u32 = 1600;

/// Decoded pictures kept in memory; past this, the ones not drawn in the
/// last frames are dropped, least recently drawn first.
const MAX_DECODED_BYTES: usize = 256 * 1024 * 1024;

/// The key of a picture on the user's disk. Only [`Images::draw_file`] reads
/// the file; a URL from the server that looks like this is never opened.
fn file_key(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// Where a picture comes from.
#[derive(Debug, Clone)]
enum Source {
    Remote(String),
    Local(PathBuf),
    /// Open a connection to this server and keep it for the pictures to
    /// come; nothing is loaded.
    Connect(String),
}

/// A work queue whose workers take the newest job first: when the user
/// scrolls past pictures, the ones on screen now are loaded before the ones
/// already gone by. Jobs for what is on screen come before the background
/// ones (pictures downloaded ahead), however many of those are waiting.
struct Queue<T> {
    jobs: Mutex<Jobs<T>>,
    ready: Condvar,
}

struct Jobs<T> {
    urgent: Vec<T>,
    background: Vec<T>,
    /// No more jobs are coming: the workers stop.
    closed: bool,
}

impl<T> Queue<T> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            jobs: Mutex::new(Jobs {
                urgent: Vec::new(),
                background: Vec::new(),
                closed: false,
            }),
            ready: Condvar::new(),
        })
    }

    /// Let every worker waiting on the queue return.
    fn close(&self) {
        self.jobs.lock().expect("queue").closed = true;
        self.ready.notify_all();
    }

    fn push(&self, job: T) {
        self.jobs.lock().expect("queue").urgent.push(job);
        self.ready.notify_one();
    }

    fn push_background(&self, job: T) {
        self.jobs.lock().expect("queue").background.push(job);
        self.ready.notify_one();
    }

    /// The next job; `None` once the queue is closed.
    fn pop(&self) -> Option<T> {
        let mut jobs = self.jobs.lock().expect("queue");
        loop {
            if jobs.closed {
                return None;
            }
            if let Some(job) = jobs.urgent.pop().or_else(|| jobs.background.pop()) {
                return Some(job);
            }
            jobs = self.ready.wait(jobs).expect("queue");
        }
    }
}

enum Slot {
    /// Asked for; `urgent` once it is wanted on screen, not only ahead.
    Loading {
        urgent: bool,
    },
    Ready(Arc<DynamicImage>),
    /// The download or decode failed at this time; it is tried again after
    /// [`RETRY_AFTER`], so a network blip does not leave a mark for good.
    Failed(Instant),
}

/// A picture encoded for one box size.
enum Encoded {
    Pending,
    Ready(Box<Protocol>),
    Failed,
}

/// How long a failed image waits before it is fetched again.
const RETRY_AFTER: Duration = Duration::from_secs(30);

/// Images kept decoded; past this, the ones not drawn recently are dropped.
const MAX_SLOTS: usize = 256;

type Loaded = (String, Result<DynamicImage, String>);
type Key = (String, u16, u16);

/// Cache and renderer for every image on screen.
pub struct Images {
    protocol_type: ratatui_image::picker::ProtocolType,
    cell: (u16, u16),
    /// Each image with the frame number it was last drawn in.
    slots: HashMap<String, (Slot, u64)>,
    protocols: HashMap<Key, Encoded>,
    frame: u64,
    frame_size: Size,
    /// Style of the "…" / "×" drawn where an image is not ready.
    placeholder: Style,
    fetch: Arc<Queue<(String, Source)>>,
    fetch_rx: Receiver<Loaded>,
    encode: Arc<Queue<(Key, Arc<DynamicImage>)>>,
    encode_rx: Receiver<(Key, Option<Protocol>)>,
    /// The pictures on the user's disk asked for, by key.
    local: HashMap<String, PathBuf>,
    /// For the video player, which encodes its own pictures.
    picker: Picker,
    /// The video playing in the viewer.
    video: Option<Player>,
    /// Whether the full-screen viewer was open in the last frame.
    viewer_open: bool,
    /// Pictures are sent inside tmux's passthrough, which a delete would
    /// need too.
    tmux: bool,
    /// Whether the terminal can show pictures. When it cannot, nothing is
    /// downloaded, decoded, or drawn.
    shows: bool,
}

impl Images {
    /// Start the loader and encoder threads. Downloads are kept in `cache`
    /// when there is one.
    pub fn new(picker: Picker, cache: Option<DiskCache>) -> Self {
        let cache = cache.map(Arc::new);
        if let Some(c) = &cache {
            let c = Arc::clone(c);
            thread::spawn(move || c.trim());
        }
        let fetch = Queue::new();
        let (img_tx, img_rx) = channel::<Loaded>();
        for _ in 0..LOADERS {
            let fetch = Arc::clone(&fetch);
            let img_tx: Sender<Loaded> = img_tx.clone();
            let cache = cache.clone();
            thread::spawn(move || {
                let agent = crate::api::agent();
                loop {
                    let Some((key, source)) = fetch.pop() else {
                        return;
                    };
                    if let Source::Connect(url) = &source {
                        let _ = agent.head(url).call();
                        continue;
                    }
                    let result = load(&agent, cache.as_deref(), &source);
                    if img_tx.send((key, result)).is_err() {
                        return;
                    }
                }
            });
        }
        let encode = Queue::new();
        let (done_tx, done_rx) = channel::<(Key, Option<Protocol>)>();
        for _ in 0..ENCODERS {
            let encode = Arc::clone(&encode);
            let done_tx = done_tx.clone();
            let picker = picker.clone();
            thread::spawn(move || {
                loop {
                    let Some((key, img)): Option<(Key, Arc<DynamicImage>)> = encode.pop() else {
                        return;
                    };
                    let size = Size::new(key.1, key.2);
                    // Scale rather than Fit: a thumbnail smaller than its box
                    // is enlarged to fill it, keeping its proportions.
                    let resize = Resize::Scale(Some(FilterType::Triangle));
                    // A picture the encoder chokes on is marked failed; it
                    // must not take the thread, and every later picture, with it.
                    // Scaled to its box first, fast and without copying the
                    // whole picture, so the protocol only has to encode it.
                    let p = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let cell = (picker.font_size().width, picker.font_size().height);
                        picker
                            .new_protocol(crate::tui::scale::to_box(&img, size, cell), size, resize)
                            .ok()
                    }))
                    .ok()
                    .flatten();
                    if done_tx.send((key, p)).is_err() {
                        return;
                    }
                }
            });
        }
        let f = picker.font_size();
        let player_picker = picker.clone();
        Self {
            protocol_type: picker.protocol_type(),
            cell: (f.width, f.height),
            slots: HashMap::new(),
            protocols: HashMap::new(),
            frame: 0,
            frame_size: Size::default(),
            placeholder: Style::new(),
            fetch,
            fetch_rx: img_rx,
            encode,
            encode_rx: done_rx,
            local: HashMap::new(),
            tmux: player_picker.tmux_detected(),
            picker: player_picker,
            video: None,
            viewer_open: false,
            shows: true,
        }
    }

    /// For a terminal that cannot show pictures: every picture is left out.
    pub fn none() -> Self {
        let mut images = Self::new(Picker::halfblocks(), None);
        images.shows = false;
        images
    }

    /// Whether pictures and video are shown at all.
    pub fn shows(&self) -> bool {
        self.shows
    }

    /// The protocol images are drawn with.
    pub fn protocol_type(&self) -> ratatui_image::picker::ProtocolType {
        self.protocol_type
    }

    /// Have every loader open its connection to the picture server at
    /// `url` now, while the first list is still loading. Each connection
    /// costs a TCP and a TLS handshake, several round trips that can take
    /// a second or more; paid here, the first pictures only wait for their
    /// own download.
    pub fn connect(&self, url: &str) {
        if !self.shows {
            return;
        }
        for _ in 0..LOADERS {
            self.fetch
                .push_background((String::new(), Source::Connect(url.to_string())));
        }
    }

    /// Pixel size of one terminal cell, as the terminal reported it.
    pub fn cell_size(&self) -> (u16, u16) {
        self.cell
    }

    /// Collect finished downloads, encodes, and video pictures. Returns
    /// whether any arrived.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.video.as_mut().is_some_and(Player::poll);
        while let Ok((url, result)) = self.fetch_rx.try_recv() {
            let slot = match result {
                Ok(img) => Slot::Ready(Arc::new(img)),
                Err(_) => Slot::Failed(Instant::now()),
            };
            let last = self.slots.get(&url).map_or(self.frame, |(_, f)| *f);
            self.slots.insert(url, (slot, last));
            changed = true;
        }
        while let Ok((key, p)) = self.encode_rx.try_recv() {
            // An answer for a size dropped since (the terminal was resized)
            // is not wanted any more.
            if let Some(e) = self.protocols.get_mut(&key) {
                *e = match p {
                    Some(p) => Encoded::Ready(Box::new(p)),
                    None => Encoded::Failed,
                };
                changed = true;
            }
        }
        changed
    }

    /// Whether any requested image is still downloading or encoding.
    pub fn loading(&self) -> bool {
        self.video
            .as_ref()
            .is_some_and(|v| v.state == State::Loading)
            || self
                .slots
                .values()
                .any(|(s, _)| matches!(s, Slot::Loading { .. }))
            || self
                .protocols
                .values()
                .any(|e| matches!(e, Encoded::Pending))
    }

    /// Start a frame of `size` cells. Encoded images are per size, so a
    /// resize drops them all; and the cache is trimmed to what was drawn in
    /// the last frames, so a long session does not keep every image it saw.
    pub fn begin_frame(&mut self, size: Size, placeholder: Style) {
        self.frame += 1;
        self.placeholder = placeholder;
        if size != self.frame_size {
            self.frame_size = size;
            self.protocols.clear();
        }
        let bytes = |slot: &Slot| match slot {
            Slot::Ready(img) => img.as_bytes().len(),
            _ => 0,
        };
        let mut total: usize = self.slots.values().map(|(s, _)| bytes(s)).sum();
        if self.slots.len() > MAX_SLOTS || total > MAX_DECODED_BYTES {
            // Oldest first, and never what the last frames drew.
            let keep_after = self.frame.saturating_sub(2);
            let mut old: Vec<(u64, String)> = self
                .slots
                .iter()
                .filter(|(_, (s, last))| !matches!(s, Slot::Loading { .. }) && *last < keep_after)
                .map(|(k, (_, last))| (*last, k.clone()))
                .collect();
            old.sort();
            let mut count = self.slots.len();
            for (_, key) in old {
                if count <= MAX_SLOTS && total <= MAX_DECODED_BYTES {
                    break;
                }
                if let Some((slot, _)) = self.slots.remove(&key) {
                    total -= bytes(&slot);
                    count -= 1;
                    self.local.remove(&key);
                }
            }
            let slots = &self.slots;
            self.protocols
                .retain(|(url, _, _), _| slots.contains_key(url));
        }
    }

    /// Note whether the viewer is open in the frame about to be drawn. When
    /// it has just closed and pictures go by kitty, every encoded picture is
    /// forgotten so the ones on screen are sent again, and the sequence that
    /// deletes kitty's copies is returned for the caller to write first.
    ///
    /// kitty keeps each picture it is sent once, and the list only redraws
    /// its place afterwards; but kitty drops the oldest pictures once its
    /// store is full, and the full-size pictures and the video of the viewer
    /// can fill it. The list then drew only empty places: the avatars and
    /// photos were gone after the viewer closed.
    pub fn viewer_closed(&mut self, open: bool) -> Option<&'static str> {
        let closed = self.viewer_open && !open;
        self.viewer_open = open;
        if !closed {
            return None;
        }
        self.delete_all()
    }

    /// Forget every encoded picture, and return the sequence that deletes
    /// kitty's copies of them, for the caller to write. Other protocols
    /// draw into the cells, which the next full redraw overwrites.
    pub fn delete_all(&mut self) -> Option<&'static str> {
        if self.protocol_type != ProtocolType::Kitty {
            return None;
        }
        self.protocols.clear();
        // Every image of this window and its data; quietly (q=2).
        (!self.tmux).then_some("\x1b_Ga=d,d=A,q=2\x1b\\")
    }

    /// Play the video at `playlist` in `area`, starting it (again, for a new
    /// `generation`) when needed. Draws nothing until its first picture;
    /// returns where playback is.
    pub fn draw_video(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        playlist: &str,
        generation: u32,
    ) -> State {
        if !self.shows {
            return State::Warning("this terminal cannot show video".into());
        }
        let area = area.intersection(frame.area());
        let size = (area.width, area.height);
        match &self.video {
            Some(v) if v.is(playlist, generation) => v.resize(size),
            _ => {
                self.video = Some(Player::start(
                    self.picker.clone(),
                    playlist,
                    size,
                    generation,
                ))
            }
        }
        let v = self.video.as_ref().expect("started above");
        if let Some(p) = v.frame() {
            frame.render_widget(Image::new(p), area);
        }
        v.state.clone()
    }

    /// Stop the video, when the viewer no longer shows it.
    pub fn stop_video(&mut self) {
        self.video = None;
    }

    /// Whether a video is playing, so the screen should follow it closely.
    pub fn playing(&self) -> bool {
        self.video
            .as_ref()
            .is_some_and(|v| matches!(v.state, State::Loading | State::Playing))
    }

    /// Whether a video picture is on screen.
    pub fn video_shown(&self) -> bool {
        self.video.as_ref().is_some_and(|v| v.frame().is_some())
    }

    /// Draw the first of `urls` that is ready in `area`, asking for all of
    /// them: a full-size picture over its thumbnail as soon as it arrives.
    pub fn draw_first(&mut self, frame: &mut Frame, area: Rect, urls: &[&str]) {
        let area = area.intersection(frame.area());
        if area.width == 0 || area.height == 0 {
            return;
        }
        let urls: Vec<&str> = urls.iter().copied().filter(|u| !u.is_empty()).collect();
        let mut mark = "…";
        for (i, url) in urls.iter().enumerate() {
            match self.request(url, area.width, area.height) {
                Ok(_) => {
                    // Everything after this one is still asked for, so it is
                    // ready when it is the best there is.
                    for rest in &urls[i + 1..] {
                        let _ = self.request(rest, area.width, area.height);
                    }
                    let p = self.request(url, area.width, area.height).expect("ready");
                    frame.render_widget(Image::new(p), area);
                    return;
                }
                Err(m) if i == 0 => mark = m,
                Err(_) => {}
            }
        }
        let style = self.placeholder;
        frame.render_widget(Paragraph::new(mark).style(style), area);
    }

    /// Draw the picture at `path` on the user's disk fitted inside `area`.
    pub fn draw_file(&mut self, frame: &mut Frame, area: Rect, path: &Path) {
        let key = file_key(path);
        self.local.insert(key.clone(), path.to_path_buf());
        self.draw(frame, area, &key);
    }

    /// Draw `url` fitted inside `area`, starting its download or encode when
    /// needed.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, url: &str) {
        let area = area.intersection(frame.area());
        if area.width == 0 || area.height == 0 || url.is_empty() {
            return;
        }
        let style = self.placeholder;
        match self.request(url, area.width, area.height) {
            Ok(p) => frame.render_widget(Image::new(p), area),
            Err(mark) => frame.render_widget(Paragraph::new(mark).style(style), area),
        }
    }

    /// Get `url` ready for a box of `width` x `height` without drawing it,
    /// so it is there when it scrolls into view.
    pub fn prefetch(&mut self, url: &str, width: u16, height: u16) {
        if width > 0 && height > 0 && !url.is_empty() {
            let _ = self.request(url, width, height);
        }
    }

    /// Pixel size of `url` once it is decoded (after shrinking).
    pub fn dims(&self, url: &str) -> Option<(u32, u32)> {
        match self.slots.get(url) {
            Some((Slot::Ready(img), _)) => Some((img.width(), img.height())),
            _ => None,
        }
    }

    /// Start downloading `url` without encoding it, for pictures further
    /// ahead than [`Images::prefetch`] prepares: when the reader gets there
    /// only the (fast) encode is left.
    pub fn warm(&mut self, url: &str) {
        if !url.is_empty() {
            let _ = self.decoded(url, false);
        }
    }

    /// The decoded picture, starting its download when needed; or the mark
    /// to show until it is there.
    fn decoded(&mut self, url: &str, urgent: bool) -> Result<Arc<DynamicImage>, &'static str> {
        if !self.shows {
            return Err("");
        }
        let frame_no = self.frame;
        let source = match self.local.get(url) {
            Some(path) => Source::Local(path.clone()),
            None => Source::Remote(url.to_string()),
        };
        let fetch = &self.fetch;
        let job = (url.to_string(), source);
        let (slot, last) = self.slots.entry(url.to_string()).or_insert_with(|| {
            if urgent {
                fetch.push(job.clone());
            } else {
                fetch.push_background(job.clone());
            }
            (Slot::Loading { urgent }, frame_no)
        });
        *last = frame_no;
        match slot {
            Slot::Failed(at) if at.elapsed() >= RETRY_AFTER => {
                fetch.push(job);
                *slot = Slot::Loading { urgent: true };
            }
            // Downloaded ahead and not there yet, and now it is on screen:
            // ahead of the queue it goes.
            Slot::Loading { urgent: false } if urgent => {
                fetch.push(job);
                *slot = Slot::Loading { urgent: true };
            }
            _ => {}
        }
        match slot {
            Slot::Ready(img) => Ok(Arc::clone(img)),
            Slot::Loading { .. } => Err("…"),
            Slot::Failed(_) => Err("×"),
        }
    }

    /// The encoded picture, or the mark to show until it is ready.
    fn request(&mut self, url: &str, width: u16, height: u16) -> Result<&Protocol, &'static str> {
        let img = self.decoded(url, true)?;
        let key = (url.to_string(), width, height);
        let encode = &self.encode;
        let encoded = self.protocols.entry(key.clone()).or_insert_with(|| {
            encode.push((key, img));
            Encoded::Pending
        });
        match encoded {
            Encoded::Ready(p) => Ok(p),
            Encoded::Pending => Err("…"),
            Encoded::Failed => Err("×"),
        }
    }
}

impl Drop for Images {
    /// The loader and encoder threads end with the images they serve.
    fn drop(&mut self) {
        self.fetch.close();
        self.encode.close();
    }
}

/// Read a picture: a local one from the user's disk, a downloaded one from
/// the cache or the network. Only a body that decodes is cached, and a cached
/// one that no longer decodes is removed, so a bad answer (an error page
/// served as 200) is not kept. A picture much larger than any box is shrunk
/// ([`shrink`]).
fn load(
    agent: &ureq::Agent,
    cache: Option<&DiskCache>,
    source: &Source,
) -> Result<DynamicImage, String> {
    let img = match source {
        Source::Local(path) => crate::media::load(path)
            .map(|(img, _)| img)
            .map_err(|e| e.message().to_string())?,
        Source::Connect(url) => return Err(format!("{url} is a connection, not a picture")),
        Source::Remote(url) => {
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return Err(format!("not a web address: {url}"));
            }
            let cached = cache.and_then(|c| c.get(url).map(|b| (c, b)));
            match cached {
                Some((c, bytes)) => match image::load_from_memory(&bytes) {
                    Ok(img) => img,
                    Err(_) => {
                        c.remove(url);
                        download(agent, cache, url)?
                    }
                },
                None => download(agent, cache, url)?,
            }
        }
    };
    Ok(shrink(img))
}

/// The picture as it is kept decoded: unchanged up to [`SHRINK_ABOVE`],
/// else scaled to [`SHRUNK_SIDE`].
fn shrink(img: DynamicImage) -> DynamicImage {
    if img.width().max(img.height()) > SHRINK_ABOVE {
        img.thumbnail(SHRUNK_SIDE, SHRUNK_SIDE)
    } else {
        img
    }
}

fn download(
    agent: &ureq::Agent,
    cache: Option<&DiskCache>,
    url: &str,
) -> Result<DynamicImage, String> {
    let bytes = fetch(agent, url)?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    if let Some(c) = cache {
        c.put(url, &bytes);
    }
    Ok(img)
}

fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    // A connection kept from before can be closed by the server just as it
    // is used again; a GET is safe to send once more on a new one.
    let mut resp = match agent.get(url).call() {
        Ok(r) => r,
        Err(_) => agent.get(url).call().map_err(|e| e.to_string())?,
    };
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_IMAGE_BYTES)
        .read_to_vec()
        .map_err(|e| e.to_string())
}

/// Downloaded pictures kept on disk between runs. Bluesky's image URLs name
/// the picture's content, so what is stored under a URL never goes stale.
///
/// Each file holds the URL on its first line and the body after it, so two
/// URLs that hash alike cannot show each other's picture. The least recently
/// used files are removed once the cache outgrows its budget. A cache that
/// cannot be written is simply not used; it never stops a picture showing.
#[derive(Debug)]
pub struct DiskCache {
    dir: PathBuf,
    max_bytes: u64,
    /// Bytes written since the last trim; a long session trims again.
    written: AtomicU64,
}

/// Default size budget of the picture cache.
pub const CACHE_BYTES: u64 = 256 * 1024 * 1024;

impl DiskCache {
    pub fn new(dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            dir,
            max_bytes,
            written: AtomicU64::new(0),
        }
    }

    /// Forget what is stored for `url`.
    fn remove(&self, url: &str) {
        let _ = fs::remove_file(self.path(url));
    }

    fn path(&self, url: &str) -> PathBuf {
        // FNV-1a: stable across runs and Rust versions, unlike the std hasher.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in url.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        self.dir.join(format!("{h:016x}"))
    }

    /// The body stored for `url`. Reading it counts as a use.
    pub fn get(&self, url: &str) -> Option<Vec<u8>> {
        let path = self.path(url);
        let data = fs::read(&path).ok()?;
        let body = data.strip_prefix(url.as_bytes())?.strip_prefix(b"\n")?;
        if let Ok(f) = fs::File::options().append(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(body.to_vec())
    }

    /// Store `body` for `url`. The file appears whole or not at all.
    pub fn put(&self, url: &str, body: &[u8]) {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let path = self.path(url);
        let tmp = self.dir.join(format!(
            "tmp.{}.{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        // A new file only: never through a link someone left at that name.
        let written = fs::File::options()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .and_then(|mut f| {
                f.write_all(url.as_bytes())?;
                f.write_all(b"\n")?;
                f.write_all(body)
            });
        if written.is_err() || fs::rename(&tmp, &path).is_err() {
            let _ = fs::remove_file(&tmp);
            return;
        }
        let len = (url.len() + 1 + body.len()) as u64;
        if self.written.fetch_add(len, Ordering::Relaxed) + len > self.max_bytes / 8 {
            self.written.store(0, Ordering::Relaxed);
            self.trim();
        }
    }

    /// Remove the least recently used files until the cache fits its budget,
    /// and temporary files a crashed run left behind.
    pub fn trim(&self) {
        let Ok(dir) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        for entry in dir.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let path = entry.path();
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let stale_tmp = entry.file_name().to_string_lossy().starts_with("tmp.")
                && modified
                    .elapsed()
                    .is_ok_and(|age| age > Duration::from_secs(3600));
            if stale_tmp {
                let _ = fs::remove_file(&path);
                continue;
            }
            files.push((modified, meta.len(), path));
        }
        let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
        files.sort();
        for (_, len, path) in files {
            if total <= self.max_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case(2000, 1500, (2000, 1500))]
    #[case(2048, 1024, (2048, 1024))]
    #[case(4000, 3000, (1600, 1200))]
    #[case(1000, 4000, (400, 1600))]
    fn only_a_picture_much_larger_than_any_box_is_shrunk(
        #[case] w: u32,
        #[case] h: u32,
        #[case] want: (u32, u32),
    ) {
        let img = shrink(DynamicImage::new_rgb8(w, h));
        assert_eq!((img.width(), img.height()), want);
    }

    /// Prints how long a screen of twelve 2000 x 1500 photos takes to be
    /// ready through the real loader and encoder threads, for comparing a
    /// change to the picture pipeline before and after:
    /// `cargo test --release screen_of_photos -- --ignored --nocapture`.
    // Not built for scripts/coverage.sh: it never runs there, so it would
    // count as untested code.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "measurement"]
    fn a_screen_of_photos_until_ready() {
        use ratatui::{Terminal, backend::TestBackend};
        use ratatui_image::picker::ProtocolType;
        let dir = tempfile::tempdir().unwrap();
        let mut seed = 7u32;
        let photo = image::RgbImage::from_fn(2000, 1500, |x, y| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let n = (seed >> 24) as u8 / 8;
            image::Rgb([(x / 8) as u8 ^ n, (y / 6) as u8, ((x + y) % 256) as u8 ^ n])
        });
        let mut jpeg = Vec::new();
        DynamicImage::ImageRgb8(photo)
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        let paths: Vec<PathBuf> = (0..12u8)
            .map(|i| {
                let p = dir.path().join(format!("p{i}.jpg"));
                let mut b = jpeg.clone();
                let n = b.len();
                b[n - 3] ^= i;
                fs::write(&p, b).unwrap();
                p
            })
            .collect();
        for proto in [
            ProtocolType::Kitty,
            ProtocolType::Sixel,
            ProtocolType::Iterm2,
        ] {
            let mut samples = Vec::new();
            for _ in 0..5 {
                #[allow(deprecated)]
                let mut picker = Picker::from_fontsize((10, 20).into());
                picker.set_protocol_type(proto);
                let mut images = Images::new(picker, None);
                let mut term = Terminal::new(TestBackend::new(160, 50)).unwrap();
                let start = Instant::now();
                loop {
                    term.draw(|f| {
                        images.begin_frame(f.area().as_size(), Style::new());
                        for (i, p) in paths.iter().enumerate() {
                            let (x, y) = ((i % 4) as u16 * 40, (i / 4) as u16 * 14);
                            images.draw_file(f, Rect::new(x, y, 36, 12), p);
                        }
                    })
                    .unwrap();
                    images.poll();
                    if !images.loading() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{proto:?}: 12 photos ready in {:.0} ms (median of 5, {:.0} to {:.0})",
                samples[2], samples[0], samples[4]
            );
        }
    }

    #[test]
    fn the_newest_job_is_taken_first() {
        let q = Queue::new();
        q.push(1);
        q.push(2);
        q.push(3);
        assert_eq!([q.pop(), q.pop(), q.pop()], [Some(3), Some(2), Some(1)]);
    }

    #[test]
    fn a_closed_queue_lets_its_workers_go() {
        let q: Arc<Queue<u8>> = Queue::new();
        let worker = {
            let q = Arc::clone(&q);
            thread::spawn(move || q.pop())
        };
        thread::sleep(Duration::from_millis(50));
        q.close();
        assert_eq!(worker.join().unwrap(), None);
    }

    #[test]
    fn closing_the_viewer_sends_kittys_pictures_again() {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ProtocolType::Kitty);
        let mut images = Images::new(picker, None);
        images
            .protocols
            .insert(("https://x/a.png".into(), 4, 2), Encoded::Failed);
        assert_eq!(images.viewer_closed(false), None);
        assert_eq!(images.viewer_closed(true), None);
        assert_eq!(images.protocols.len(), 1, "kept while the viewer is open");
        assert_eq!(images.viewer_closed(false), Some("\x1b_Ga=d,d=A,q=2\x1b\\"));
        assert!(images.protocols.is_empty());
        // Only once per close.
        assert_eq!(images.viewer_closed(false), None);
    }

    #[test]
    fn other_protocols_keep_their_pictures_when_the_viewer_closes() {
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ProtocolType::Sixel);
        let mut images = Images::new(picker, None);
        images
            .protocols
            .insert(("https://x/a.png".into(), 4, 2), Encoded::Failed);
        images.viewer_closed(true);
        assert_eq!(images.viewer_closed(false), None);
        assert_eq!(images.protocols.len(), 1);
    }

    #[test]
    fn dropping_images_ends_their_threads() {
        // Many in a row would run out of threads if each left ten behind.
        for _ in 0..300 {
            drop(Images::new(Picker::halfblocks(), None));
        }
    }

    #[test]
    fn what_is_on_screen_comes_before_what_is_ahead() {
        let q = Queue::new();
        q.push_background(1);
        q.push_background(2);
        q.push(3);
        q.push_background(4);
        q.push(5);
        let order: Vec<_> = (0..5).map(|_| q.pop().unwrap()).collect();
        assert_eq!(order, [5, 3, 4, 2, 1]);
    }

    #[test]
    fn a_cached_body_that_no_longer_decodes_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf(), CACHE_BYTES);
        // An unreachable address: after dropping the bad copy, the download
        // fails too, and nothing is stored in its place.
        let url = "http://127.0.0.1:9/pic.png";
        cache.put(url, b"<html>not a picture</html>");
        let agent = crate::api::agent();
        assert!(load(&agent, Some(&cache), &Source::Remote(url.into())).is_err());
        assert_eq!(cache.get(url), None);
    }

    #[test]
    fn the_cache_returns_what_was_stored_for_that_url_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().join("images"), CACHE_BYTES);
        assert_eq!(cache.get("https://cdn.test/a"), None);
        cache.put("https://cdn.test/a", b"body a");
        assert_eq!(
            cache.get("https://cdn.test/a").as_deref(),
            Some(&b"body a"[..])
        );
        assert_eq!(cache.get("https://cdn.test/b"), None);
        // A file whose first line names another URL is not its picture.
        fs::write(cache.path("https://cdn.test/b"), b"https://other\nx").unwrap();
        assert_eq!(cache.get("https://cdn.test/b"), None);
        let names: Vec<_> = fs::read_dir(dir.path().join("images"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.iter().all(|n| !n.starts_with("tmp.")), "{names:?}");
    }

    #[test]
    fn trimming_removes_the_least_recently_used_first() {
        let dir = tempfile::tempdir().unwrap();
        // Each file is its URL line and 20 bytes, about 32; two fit.
        let cache = DiskCache::new(dir.path().to_path_buf(), 80);
        for (i, url) in ["https://old", "https://mid", "https://new"]
            .iter()
            .enumerate()
        {
            cache.put(url, &[b'x'; 20]);
            let f = fs::File::options()
                .append(true)
                .open(cache.path(url))
                .unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + i as u64))
                .unwrap();
        }
        cache.trim();
        assert_eq!(cache.get("https://old"), None);
        assert!(cache.get("https://mid").is_some());
        assert!(cache.get("https://new").is_some());
    }

    #[test]
    fn an_unwritable_cache_is_skipped_quietly() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        fs::write(&file, "").unwrap();
        let cache = DiskCache::new(file.join("images"), CACHE_BYTES);
        cache.put("https://a", b"x");
        assert_eq!(cache.get("https://a"), None);
        cache.trim();
    }

    /// A long session draws far more pictures than it keeps: past
    /// MAX_SLOTS decoded pictures the least recently drawn are dropped, and
    /// never the ones on screen now.
    #[test]
    fn decoded_pictures_are_bounded_and_the_ones_on_screen_stay() {
        use ratatui::{Terminal, backend::TestBackend};
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..MAX_SLOTS + 40)
            .map(|i| {
                let p = dir.path().join(format!("p{i}.png"));
                image::RgbImage::from_pixel(2, 2, image::Rgb([i as u8, 0, 0]))
                    .save(&p)
                    .unwrap();
                p
            })
            .collect();
        let mut images = Images::new(Picker::halfblocks(), None);
        let mut term = Terminal::new(TestBackend::new(40, 10)).unwrap();
        // Ten new pictures a frame, each frame waiting for its pictures, as
        // a reader scrolling down a long list.
        for chunk in paths.chunks(10) {
            let start = Instant::now();
            loop {
                term.draw(|f| {
                    images.begin_frame(f.area().as_size(), Style::new());
                    for (i, p) in chunk.iter().enumerate() {
                        images.draw_file(f, Rect::new(i as u16 * 4, 0, 4, 2), p);
                    }
                })
                .unwrap();
                images.poll();
                if !images.loading() || start.elapsed() > Duration::from_secs(10) {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
            }
        }
        // One more frame drawing only the last ten: the trim runs.
        let last: Vec<&PathBuf> = paths.iter().rev().take(10).collect();
        term.draw(|f| {
            images.begin_frame(f.area().as_size(), Style::new());
            for (i, p) in last.iter().enumerate() {
                images.draw_file(f, Rect::new(i as u16 * 4, 0, 4, 2), p);
            }
        })
        .unwrap();
        assert!(
            images.slots.len() <= MAX_SLOTS,
            "{} decoded pictures kept",
            images.slots.len()
        );
        for p in last {
            assert!(
                images.slots.contains_key(&file_key(p)),
                "{} was dropped",
                p.display()
            );
        }
        // An encoded picture is kept only for a picture still kept.
        assert!(
            images
                .protocols
                .keys()
                .all(|(url, _, _)| images.slots.contains_key(url))
        );
    }

    /// Temporary files a crashed run left in the cache are removed once they
    /// are an hour old; one being written now is not.
    #[test]
    fn the_cache_trim_removes_stale_temporary_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_path_buf(), 1 << 20);
        let stale = dir.path().join("tmp.1.0");
        let fresh = dir.path().join("tmp.1.1");
        fs::write(&stale, b"half written").unwrap();
        fs::write(&fresh, b"being written").unwrap();
        fs::File::options()
            .write(true)
            .open(&stale)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(2 * 3600))
            .unwrap();
        cache.put("https://a.test/p.png", b"body");
        cache.trim();
        assert!(!stale.exists(), "a stale temporary file is left");
        assert!(fresh.exists(), "a fresh temporary file was removed");
        assert_eq!(
            cache.get("https://a.test/p.png").as_deref(),
            Some(&b"body"[..])
        );
    }

    #[test]
    fn a_local_picture_is_loaded_by_its_file_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.png");
        image::RgbImage::from_pixel(3200, 100, image::Rgb([1, 2, 3]))
            .save(&path)
            .unwrap();
        let agent = crate::api::agent();
        let img = load(&agent, None, &Source::Local(path.clone())).unwrap();
        assert_eq!((img.width(), img.height()), (SHRUNK_SIDE, 50));
        let missing = load(&agent, None, &Source::Local(dir.path().join("gone.png")));
        assert!(missing.unwrap_err().contains("gone.png"));
        // A server's URL is never read from disk, whatever it looks like.
        let remote = load(&agent, None, &Source::Remote(file_key(&path)));
        assert!(remote.unwrap_err().starts_with("not a web address"));
    }
}

#[cfg(test)]
mod latency {
    use super::*;

    /// How long the pictures of a first screen take after a list that took
    /// 1.5 s to load, with and without the connections opened meanwhile:
    /// `BSKY_PICTURES=... cargo test --release first_screen -- --ignored --nocapture`.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "measurement"]
    fn first_screen() {
        let Ok(list) = std::env::var("BSKY_PICTURES") else {
            return;
        };
        let urls: Vec<&str> = list.lines().filter(|l| !l.is_empty()).collect();
        for warm in [false, true, false, true] {
            let mut images = Images::new(Picker::halfblocks(), None);
            if warm {
                images.connect("https://cdn.bsky.app/");
            }
            thread::sleep(Duration::from_millis(1500));
            let start = Instant::now();
            while !urls.iter().all(|u| {
                matches!(
                    images.slots.get(*u),
                    Some((Slot::Ready(_) | Slot::Failed(_), _))
                )
            }) {
                for u in &urls {
                    let _ = images.decoded(u, true);
                }
                images.poll();
                thread::sleep(Duration::from_millis(5));
            }
            println!(
                "warm {warm}: {} pictures in {:?}",
                urls.len(),
                start.elapsed()
            );
        }
    }

    /// Where the time to a picture on screen goes, for the URLs in
    /// $BSKY_PICTURES (one per line), one agent as a loader thread has:
    /// `BSKY_PICTURES=... cargo test --release picture_stages -- --ignored --nocapture`.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "measurement"]
    fn picture_stages() {
        let Ok(list) = std::env::var("BSKY_PICTURES") else {
            return;
        };
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
        let agent = crate::api::agent();
        for url in list.lines().filter(|l| !l.is_empty()) {
            let t0 = Instant::now();
            let bytes = fetch(&agent, url).unwrap();
            let t_fetch = t0.elapsed();
            let img = image::load_from_memory(&bytes).unwrap();
            let t_decode = t0.elapsed() - t_fetch;
            let (w, h) = (img.width(), img.height());
            let area = Size::new(40, 12);
            let t1 = Instant::now();
            let cell = (10, 20);
            let _ = picker
                .new_protocol(
                    crate::tui::scale::to_box(&img, area, cell),
                    area,
                    Resize::Scale(Some(FilterType::Triangle)),
                )
                .unwrap();
            let t_encode = t1.elapsed();
            println!(
                "{:>5} KB {w}x{h}: fetch {t_fetch:?} decode {t_decode:?} encode {t_encode:?}",
                bytes.len() / 1024
            );
        }
    }
}
