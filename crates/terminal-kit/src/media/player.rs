//! Silent MP4/H.264 playback and shared video frame scheduling.
//! Transport-specific clients supply a job to `start_with`; the UI polls its newest frame.

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

use crate::scale;

/// Most pictures a second sent to the terminal; each is a whole image.
const MAX_FPS: f64 = 15.0;
/// Largest video file played; a larger one is saved with `d` instead.
const MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Where the playback is.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Waiting for the first frame.
    Loading,
    /// Frames are arriving.
    Playing,
    /// The last frame was reached.
    Ended,
    /// It cannot be played; the reason, for the user.
    Warning(String),
}

/// Playback operations used by the shared renderer. Dropping a backend stops it.
pub trait Video {
    /// Whether this is the requested URL and replay.
    fn is(&self, url: &str, generation: u32) -> bool;
    /// Change the target size in terminal cells.
    fn resize(&self, size: (u16, u16));
    /// Collect new frames and report whether the screen changed.
    fn poll(&mut self) -> bool;
    /// The most recent encoded frame.
    fn frame(&self) -> Option<&Protocol>;
    /// Current playback state.
    fn state(&self) -> &State;
}

/// A transport and decoder job. Frames go to the sender; `stop` ends work and
/// `size` tracks the current terminal box. The renderer owns both controls.
pub type VideoJob = fn(
    &Picker,
    &str,
    &AtomicBool,
    &Mutex<(u16, u16)>,
    &Sender<Box<Protocol>>,
) -> Result<(), String>;

/// Run a transport's video job with the shared lifetime, resize and frame queue.
pub fn start_with(
    picker: Picker,
    url: &str,
    size: (u16, u16),
    generation: u32,
    run: VideoJob,
) -> Box<dyn Video> {
    Box::new(Player::start(picker, url, size, generation, run))
}

pub(super) fn start(
    picker: Picker,
    url: &str,
    size: (u16, u16),
    generation: u32,
) -> Box<dyn Video> {
    start_with(picker, url, size, generation, play)
}

/// A video being played.
pub(crate) struct Player {
    playlist: String,
    /// The replay this player is for.
    generation: u32,
    frames: Receiver<Box<Protocol>>,
    states: Receiver<State>,
    stop: Arc<AtomicBool>,
    /// The box, in cells, pictures are made for.
    size: Arc<Mutex<(u16, u16)>>,
    frame: Option<Box<Protocol>>,
    state: State,
}

impl Player {
    /// Start playing `playlist` in boxes of `size` cells.
    fn start(
        picker: Picker,
        playlist: &str,
        size: (u16, u16),
        generation: u32,
        run: VideoJob,
    ) -> Self {
        let (tx, frames) = channel();
        let (state_tx, states) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(size));
        let (url, thread_stop, thread_size) =
            (playlist.to_string(), Arc::clone(&stop), Arc::clone(&shared));
        let started = super::spawn("terminal-video", move || {
            let state = match run(&picker, &url, &thread_stop, &thread_size, &tx) {
                Ok(()) => State::Ended,
                Err(why) => State::Warning(why),
            };
            #[expect(
                clippy::let_underscore_must_use,
                reason = "the player dropped, the viewer closed: nobody is left to tell how it ended"
            )]
            let _: Result<(), _> = state_tx.send(state);
        });
        let state = match started {
            Ok(_) => State::Loading,
            Err(e) => State::Warning(format!("cannot load the video: {e}")),
        };
        Self {
            playlist: playlist.to_string(),
            generation,
            frames,
            states,
            stop,
            size: shared,
            frame: None,
            state,
        }
    }
}

impl Video for Player {
    /// Whether this plays `playlist` for replay `generation`.
    fn is(&self, playlist: &str, generation: u32) -> bool {
        self.playlist == playlist && self.generation == generation
    }

    /// Make the next pictures for a box of `size` cells.
    fn resize(&self, size: (u16, u16)) {
        if let Ok(mut s) = self.size.lock() {
            *s = size;
        }
    }

    /// Take what the thread has sent. Returns whether anything changed.
    fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(frame) = self.frames.try_recv() {
            self.frame = Some(frame);
            if self.state == State::Loading {
                self.state = State::Playing;
            }
            changed = true;
        }
        while let Ok(state) = self.states.try_recv() {
            self.state = state;
            changed = true;
        }
        changed
    }

    /// The newest picture, if one has arrived.
    fn frame(&self) -> Option<&Protocol> {
        self.frame.as_deref()
    }
    fn state(&self) -> &State {
        &self.state
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(super::agent)
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
    tx: &Sender<Box<Protocol>>,
) -> Result<(), String> {
    let file = fetch(url)?;
    play_file(picker, &file, stop, size, tx)
}

fn play_file(
    picker: &Picker,
    file: &[u8],
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Box<Protocol>>,
) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    if file.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Err(
            "the video is WebM (VP8, VP9 or AV1), which this player cannot decode: it decodes H.264 only, with OpenH264"
                .into(),
        );
    }
    let track = super::mp4::video_track(file)?;
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
    pacer.finish(&mut decoder, &mut pending, stop)?;
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

/// Convert a decoded H.264 frame to RGB for the terminal encoder.
pub fn to_rgb(yuv: &DecodedYUV<'_>) -> Option<RgbImage> {
    let (w, h) = yuv.dimensions();
    let mut rgb = vec![0u8; w * h * 3];
    yuv.write_rgb8(&mut rgb);
    RgbImage::from_raw(w as u32, h as u32, rgb)
}

/// Shows pictures at their time: waits for early ones, drops late ones.
pub struct Pacer<'a> {
    picker: &'a Picker,
    size: &'a Mutex<(u16, u16)>,
    tx: &'a Sender<Box<Protocol>>,
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
    /// Flush delayed H.264 frames in presentation order, stopping when the viewer closes.
    pub fn finish(
        &mut self,
        decoder: &mut Decoder,
        pending: &mut BinaryHeap<Reverse<u64>>,
        stop: &AtomicBool,
    ) -> Result<(), String> {
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
            if !self.show(picture, pts)? {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Number of frames sent so far.
    pub fn shown(&self) -> usize {
        self.shown
    }

    /// Pace encoded frames into `tx`, using the current cell dimensions in `size`.
    pub fn new(
        picker: &'a Picker,
        size: &'a Mutex<(u16, u16)>,
        tx: &'a Sender<Box<Protocol>>,
    ) -> Self {
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
    pub fn show(&mut self, picture: Option<RgbImage>, pts: Option<u64>) -> Result<bool, String> {
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
        if self.tx.send(Box::new(p)).is_err() {
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
    #[test]
    fn mp4_decodes_to_encoded_frames_and_webm_explains_why_it_cannot() {
        let picker = Picker::halfblocks();
        let stop = AtomicBool::new(false);
        let size = Mutex::new((16, 9));
        let (tx, rx) = channel();
        play_file(
            &picker,
            include_bytes!("testdata/clip.mp4"),
            &stop,
            &size,
            &tx,
        )
        .unwrap();
        assert!(rx.try_iter().count() > 0);
        let why = play_file(&picker, b"\x1a\x45\xdf\xa3 webm", &stop, &size, &tx).unwrap_err();
        assert!(why.starts_with("the video is WebM"));
        stop.store(true, Ordering::Relaxed);
        play_file(&picker, b"bad", &stop, &size, &tx).unwrap();
        assert!(rx.try_recv().is_err());
    }
    /// Prints how long a 1280 x 720 video picture takes to be scaled and
    /// encoded for its box, the work behind every frame shown:
    /// `cargo test --release video_picture -- --ignored --nocapture`.
    #[test]
    fn kitty_pictures_of_a_video_take_two_images_in_turn_and_redraw_every_cell() {
        #[allow(
            deprecated,
            reason = "a picker of a known cell size, which a test has no terminal to ask for"
        )]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ProtocolType::Kitty);
        let area = Size::new(40, 12);
        let rect = ratatui::layout::Rect::new(0, 0, area.width, area.height);
        let frames: Vec<(String, ratatui::buffer::Buffer)> = [10u8, 200, 90]
            .into_iter()
            .enumerate()
            .map(|(nth, shade)| {
                let picture = RgbImage::from_pixel(640, 360, image::Rgb([shade; 3]));
                let img = scale::to_box(&DynamicImage::ImageRgb8(picture), area, (10, 20));
                let p = encode_picture(&picker, img.clone(), area, nth).unwrap();
                // The same box new_protocol gives, so the picture sits where
                // it did before.
                let theirs = picker
                    .new_protocol(img, area, Resize::Scale(Some(FilterType::Triangle)))
                    .unwrap();
                assert_eq!(p.size(), theirs.size());
                let mut buf = ratatui::buffer::Buffer::empty(rect);
                ratatui::widgets::Widget::render(ratatui_image::Image::new(&p), rect, &mut buf);
                let seq: String = buf
                    .content
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect();
                // The transmission starts "\x1b_Gq=2,i=<id>,a=T".
                let at = seq.find("_Gq=2,i=").unwrap() + 6;
                (seq[at..].split(',').next().unwrap().to_string(), buf)
            })
            .collect();
        let [a, b] = VIDEO_IMAGE_IDS;
        let ids: Vec<&str> = frames.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, [format!("i={a}"), format!("i={b}"), format!("i={a}")]);
        // Every cell the picture covers differs from the last picture's, so
        // all of them are written again and kitty redraws every row.
        let (before, after) = (&frames[0].1, &frames[1].1);
        let covered = before
            .content
            .iter()
            .zip(&after.content)
            .filter(|(x, _)| !x.symbol().trim().is_empty());
        assert!(covered.clone().count() > 0);
        assert!(covered.clone().all(|(x, y)| x != y));
    }
}
