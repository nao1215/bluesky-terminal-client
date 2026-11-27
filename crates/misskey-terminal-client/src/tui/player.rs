//! Playing a note's video in the terminal, without sound, as bsky plays
//! Bluesky's.
//!
//! A thread downloads the file, takes the H.264 pictures out of its MP4
//! ([`crate::tui::mp4`]), decodes them with OpenH264 (built into msky, as
//! into bsky, so nothing else is installed), and turns each into the
//! terminal's image protocol at the size of the box it is shown in, paced by
//! the video's own clock; this pacing and the kitty image ids are bsky's.
//! The UI draws the newest picture it has been handed. A video msky cannot
//! play (WebM, HEVC, AV1) is not an error: the viewer says why, and `d`
//! saves it and `o` opens it.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use image::{DynamicImage, RgbImage};
use openh264::decoder::{DecodedYUV, Decoder};
use openh264::formats::YUVSource;
use ratatui::layout::Size;
use ratatui_image::picker::{Capability, Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::protocol::kitty::Kitty;
use ratatui_image::{FilterType, Resize};

use crate::tui::scale;

/// Most pictures a second sent to the terminal; each is a whole image.
const MAX_FPS: f64 = 15.0;
/// Largest video file played; a larger one is saved with `d` instead.
const MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Where the playback is.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Loading,
    Playing,
    Ended,
    /// It cannot be played; the reason, for the user.
    Warning(String),
}

enum Msg {
    Frame(Box<Protocol>),
    State(State),
}

/// A video being played.
pub struct Player {
    playlist: String,
    /// The replay this player is for.
    pub generation: u32,
    rx: Receiver<Msg>,
    stop: Arc<AtomicBool>,
    /// The box, in cells, pictures are made for.
    size: Arc<Mutex<(u16, u16)>>,
    frame: Option<Box<Protocol>>,
    pub state: State,
}

impl Player {
    /// Start playing `playlist` in boxes of `size` cells.
    pub fn start(picker: Picker, playlist: &str, size: (u16, u16), generation: u32) -> Self {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(size));
        let (url, thread_stop, thread_size) =
            (playlist.to_string(), Arc::clone(&stop), Arc::clone(&shared));
        let started = crate::tui::spawn("msky-player", move || {
            let state = match play(&picker, &url, &thread_stop, &thread_size, &tx) {
                Ok(()) => State::Ended,
                Err(why) => State::Warning(why),
            };
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the player dropped, the viewer closed: nobody is left to tell how it ended"
            )]
            let _: Result<(), _> = tx.send(Msg::State(state));
        });
        let state = match started {
            Ok(_) => State::Loading,
            Err(e) => State::Warning(format!("cannot load the video: {e}")),
        };
        Self {
            playlist: playlist.to_string(),
            generation,
            rx,
            stop,
            size: shared,
            frame: None,
            state,
        }
    }

    /// Whether this plays `playlist` for replay `generation`.
    pub fn is(&self, playlist: &str, generation: u32) -> bool {
        self.playlist == playlist && self.generation == generation
    }

    /// Make the next pictures for a box of `size` cells.
    pub fn resize(&self, size: (u16, u16)) {
        if let Ok(mut s) = self.size.lock() {
            *s = size;
        }
    }

    /// Take what the thread has sent. Returns whether anything changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Frame(p) => {
                    self.frame = Some(p);
                    if self.state == State::Loading {
                        self.state = State::Playing;
                    }
                }
                Msg::State(s) => self.state = s,
            }
            changed = true;
        }
        changed
    }

    /// The newest picture, if one has arrived.
    pub fn frame(&self) -> Option<&Protocol> {
        self.frame.as_deref()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(crate::api::agent)
}

/// The video file at `url`, up to [`MAX_BYTES`].
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("not a web address: {url}"));
    }
    let mut resp = agent()
        .get(url)
        .call()
        .map_err(|e| format!("cannot load the video: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "cannot load the video: HTTP {}",
            resp.status().as_u16()
        ));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|e| format!("cannot load the video: {e}"))
}

fn play(
    picker: &Picker,
    url: &str,
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Msg>,
) -> Result<(), String> {
    let file = fetch(url)?;
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    if file.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Err(
            "the video is WebM (VP8, VP9 or AV1), which msky cannot decode: it decodes H.264 only, with OpenH264"
                .into(),
        );
    }
    let track = crate::tui::mp4::video_track(&file)?;
    let mut decoder = decoder()?;
    let mut pacer = Pacer::new(picker, size, tx);
    // Pictures come out of the decoder in the order they are shown, which
    // with B-frames is not the order they go in; each takes the earliest
    // time not yet used.
    let mut pending: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
    let mut first = true;
    for sample in &track.samples {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        pending.push(Reverse(sample.pts));
        let unit = if first {
            first = false;
            [track.config.as_slice(), sample.data.as_slice()].concat()
        } else {
            sample.data.clone()
        };
        let Ok(Some(yuv)) = decoder.decode(&unit) else {
            continue;
        };
        let picture = to_rgb(&yuv);
        let pts = pending.pop().map(|Reverse(p)| p);
        if !pacer.show(picture, pts)? {
            return Ok(());
        }
    }
    let rest: Vec<Option<RgbImage>> = decoder
        .flush_remaining()
        .unwrap_or_default()
        .iter()
        .map(to_rgb)
        .collect();
    for picture in rest {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let pts = pending.pop().map(|Reverse(p)| p);
        if !pacer.show(picture, pts)? {
            return Ok(());
        }
    }
    if pacer.shown == 0 {
        return Err("no picture of the video could be decoded".into());
    }
    Ok(())
}

/// A decoder that holds pictures until their turn. Forcing one out after
/// every frame (the crate's default) breaks streams with B-frames, which
/// Bluesky's are, and phones' MP4s too: most frames then fail to decode.
fn decoder() -> Result<Decoder, String> {
    use openh264::OpenH264API;
    use openh264::decoder::{DecoderConfig, Flush};
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    Decoder::with_api_config(OpenH264API::from_source(), config)
        .map_err(|e| format!("cannot start the video decoder: {e}"))
}

fn to_rgb(yuv: &DecodedYUV<'_>) -> Option<RgbImage> {
    let (w, h) = yuv.dimensions();
    let mut rgb = vec![0u8; w * h * 3];
    yuv.write_rgb8(&mut rgb);
    RgbImage::from_raw(w as u32, h as u32, rgb)
}

/// Shows pictures at their time: waits for early ones, drops late ones.
struct Pacer<'a> {
    picker: &'a Picker,
    size: &'a Mutex<(u16, u16)>,
    tx: &'a Sender<Msg>,
    cell: (u16, u16),
    /// The stream's clock against the wall clock: set by the first picture,
    /// and set again after a wait for the network, so a slow download
    /// pauses the video instead of skipping it.
    clock: Option<(Instant, u64)>,
    /// When the last picture shown was due.
    last_due: Option<Instant>,
    shown: usize,
}

impl<'a> Pacer<'a> {
    fn new(picker: &'a Picker, size: &'a Mutex<(u16, u16)>, tx: &'a Sender<Msg>) -> Self {
        let f = picker.font_size();
        Self {
            picker,
            size,
            tx,
            cell: (f.width.max(1), f.height.max(1)),
            clock: None,
            last_due: None,
            shown: 0,
        }
    }

    /// Show `picture` (presented at `pts`) when it is due. Returns false once
    /// nobody is watching.
    fn show(&mut self, picture: Option<RgbImage>, pts: Option<u64>) -> Result<bool, String> {
        let Some(picture) = picture else {
            return Ok(true);
        };
        let now = Instant::now();
        let due = match (pts, self.clock) {
            (Some(p), Some((at, first))) => {
                let due = at + Duration::from_secs_f64(p.saturating_sub(first) as f64 / 90_000.0);
                // Far behind (a stall) or far ahead (the times jumped, as at
                // a discontinuity): the clock starts again at this picture,
                // rather than skip all that follows or wait for the jump.
                if now > due + Duration::from_secs(1) || due > now + Duration::from_secs(1) {
                    self.clock = Some((now, p));
                    now
                } else {
                    due
                }
            }
            (Some(p), None) => {
                self.clock = Some((now, p));
                now
            }
            (None, _) => now,
        };
        // Late, or too soon after the last one shown (both by when they are
        // due, so the time a picture takes to prepare is not counted against
        // the next): decoded, not shown.
        let too_soon = self.last_due.is_some_and(|l| {
            due.saturating_duration_since(l) < Duration::from_secs_f64(1.0 / MAX_FPS) * 9 / 10
        });
        if (now > due + Duration::from_millis(250) || too_soon) && self.shown > 0 {
            return Ok(true);
        }
        if due > now {
            thread::sleep(due - now);
        }
        #[expect(
            clippy::map_err_ignore,
            reason = "a poisoned lock says only that the UI thread panicked, which the message says; it is translated whole where it is shown"
        )]
        let (cols, rows) = *self
            .size
            .lock()
            .map_err(|_| "the player stopped".to_string())?;
        if cols == 0 || rows == 0 {
            return Ok(true);
        }
        // Scaled here, off the UI thread, to the pixels of its box, so the
        // protocol only has to encode it.
        let area = Size::new(cols, rows);
        let img = scale::to_box(&DynamicImage::ImageRgb8(picture), area, self.cell);
        let Some(p) = encode_picture(self.picker, img, area, self.shown) else {
            return Ok(true);
        };
        if self.tx.send(Msg::Frame(Box::new(p))).is_err() {
            return Ok(false);
        }
        self.last_due = Some(due);
        self.shown += 1;
        Ok(true)
    }
}

/// The two kitty image ids the pictures of a video are sent under, in turn.
/// ratatui-image gives each picture a new random id, and kitty keeps every
/// image it is sent until its store is full, then drops the oldest: a video
/// of a few hundred pictures pushed the timeline's avatars and photos out,
/// and they came back blank after the viewer closed. A picture sent under an
/// id replaces the image kitty had under it, so two ids keep at most two
/// pictures there. Not one: the cells that show a kitty picture name its id,
/// so under a single id they never change, the screen library leaves them
/// alone, and kitty redraws only the row that carries the new picture; the
/// video stayed black but for a strip at its top.
const VIDEO_IMAGE_IDS: [u32; 2] = [0x00B5_4B59, 0x00B5_4B5A];

/// The protocol for the `nth` picture shown of the video, already scaled
/// to its box.
fn encode_picture(picker: &Picker, img: DynamicImage, area: Size, nth: usize) -> Option<Protocol> {
    if picker.protocol_type() == ProtocolType::Kitty {
        // What new_protocol would choose for a picture that fits its box.
        let size = Resize::natural_size(&img, picker.font_size());
        let compress = picker
            .capabilities()
            .contains(&Capability::KittyCompression);
        let id = VIDEO_IMAGE_IDS[nth % 2];
        return Kitty::new(img, size, id, picker.tmux_detected(), compress)
            .ok()
            .map(Protocol::Kitty);
    }
    let f = picker.font_size();
    let img = scale::pad_to_cells(img, area, (f.width, f.height));
    picker
        .new_protocol(img, area, Resize::Scale(Some(FilterType::Triangle)))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker() -> Picker {
        #[allow(
            deprecated,
            reason = "a picker of a known cell size, which a test has no terminal to ask for"
        )]
        let mut p = Picker::from_fontsize((10, 20).into());
        p.set_protocol_type(ProtocolType::Kitty);
        p
    }

    // The fixture is served, downloaded, decoded and paced: pictures come,
    // and the player says it ended.
    #[test]
    fn an_mp4_from_the_server_plays_to_its_end() {
        use crate::api::stub_tests::{Reply, serve};
        let clip = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e2e/atago/testdata/clip.mp4"
        ))
        .unwrap();
        let (url, _) = serve(Arc::new(move |_, _, _| Reply {
            status: 200,
            headers: Vec::new(),
            body: clip.clone(),
        }));
        let mut p = Player::start(picker(), &format!("{url}/clip.mp4"), (16, 9), 0);
        let start = Instant::now();
        while p.state != State::Ended && start.elapsed() < Duration::from_secs(10) {
            p.poll();
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(p.state, State::Ended);
        assert!(p.frame().is_some(), "a picture was shown");
    }

    // A WebM is said to be one, with why it cannot be played.
    #[test]
    fn a_webm_says_why_it_cannot_be_played() {
        use crate::api::stub_tests::{Reply, serve};
        let (url, _) = serve(Arc::new(|_, _, _| Reply {
            status: 200,
            headers: Vec::new(),
            body: b"\x1a\x45\xdf\xa3 webm".to_vec(),
        }));
        let mut p = Player::start(picker(), &format!("{url}/v.webm"), (16, 9), 0);
        let start = Instant::now();
        while matches!(p.state, State::Loading) && start.elapsed() < Duration::from_secs(10) {
            p.poll();
            thread::sleep(Duration::from_millis(10));
        }
        let State::Warning(why) = &p.state else {
            panic!("{:?}", p.state)
        };
        assert!(why.starts_with("the video is WebM"), "{why}");
    }
}
