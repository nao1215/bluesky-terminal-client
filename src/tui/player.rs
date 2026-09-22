//! Playing a Bluesky video in the terminal, without sound.
//!
//! A thread fetches the HLS playlists and segments, pulls the H.264 out of
//! each segment, decodes it with OpenH264 (built into bs, so nothing else is
//! installed), and turns each picture into the terminal's image protocol at
//! the size of the box it is shown in, paced by the video's own clock. The
//! UI draws the newest picture it has been handed. A video bs cannot play is
//! not an error: the viewer shows its thumbnail and says why.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use image::{DynamicImage, RgbImage};
use openh264::decoder::{DecodedYUV, Decoder};
use openh264::formats::YUVSource;
use ratatui::layout::Size;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Resize};

use crate::hls::{self, Demuxer};

/// Most pictures a second sent to the terminal; each is a whole image.
const MAX_FPS: f64 = 15.0;
/// Largest segment or playlist read.
const MAX_BYTES: u64 = 32 * 1024 * 1024;

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
        {
            let (url, stop, shared) =
                (playlist.to_string(), Arc::clone(&stop), Arc::clone(&shared));
            thread::spawn(move || {
                let state = match play(&picker, &url, &stop, &shared, &tx) {
                    Ok(()) => State::Ended,
                    Err(why) => State::Warning(why),
                };
                let _ = tx.send(Msg::State(state));
            });
        }
        Self {
            playlist: playlist.to_string(),
            generation,
            rx,
            stop,
            size: shared,
            frame: None,
            state: State::Loading,
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

fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut resp = agent
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

fn text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    String::from_utf8(fetch(agent, url)?).map_err(|_| "the video's playlist is not text".into())
}

/// Play to the end, or until `stop`.
fn play(
    picker: &Picker,
    playlist: &str,
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Msg>,
) -> Result<(), String> {
    let agent = crate::api::agent();
    let master = text(&agent, playlist)?;
    let (media_url, media) = match hls::pick_variant(&master, playlist) {
        Some(url) => {
            let media = text(&agent, &url)?;
            (url, media)
        }
        None => (playlist.to_string(), master),
    };
    let segments = hls::segments(&media, &media_url);
    if segments.is_empty() {
        return Err("the video's playlist lists nothing to play".into());
    }
    let mut decoder = Decoder::new().map_err(|e| format!("cannot start the video decoder: {e}"))?;
    let mut demux = Demuxer::new();
    let mut pacer = Pacer::new(picker, size, tx);
    // Pictures come out of the decoder in the order they are shown, which
    // with B-frames is not the order they go in; each takes the earliest
    // presentation time not yet used.
    let mut pending: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
    for (n, url) in segments.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let last = n + 1 == segments.len();
        let bytes = fetch(&agent, url)?;
        demux.feed(&bytes);
        let units = if last { demux.finish() } else { demux.take() };
        if let Some(kind) = demux.unsupported {
            return Err(format!(
                "this video is not H.264 (stream type 0x{kind:02x}), which bs cannot decode"
            ));
        }
        for unit in units {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            if let Some(p) = unit.pts {
                pending.push(Reverse(p));
            }
            let Ok(Some(yuv)) = decoder.decode(&unit.data) else {
                continue;
            };
            let picture = to_rgb(&yuv);
            let pts = pending.pop().map(|Reverse(p)| p);
            if !pacer.show(picture, pts)? {
                return Ok(());
            }
        }
        if last {
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
        }
    }
    if pacer.shown == 0 {
        return Err("no picture of the video could be decoded".into());
    }
    Ok(())
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
    cell: (u32, u32),
    /// The stream's clock against the wall clock: set by the first picture,
    /// and set again after a wait for the network, so a slow download
    /// pauses the video instead of skipping it.
    clock: Option<(Instant, u64)>,
    last_shown: Option<Instant>,
    shown: usize,
}

impl<'a> Pacer<'a> {
    fn new(picker: &'a Picker, size: &'a Mutex<(u16, u16)>, tx: &'a Sender<Msg>) -> Self {
        let f = picker.font_size();
        Self {
            picker,
            size,
            tx,
            cell: (u32::from(f.width.max(1)), u32::from(f.height.max(1))),
            clock: None,
            last_shown: None,
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
                if now > due + Duration::from_secs(1) {
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
        // Late, or too soon after the last one shown (measured from when
        // this one is due): decoded, not shown.
        let too_soon = self.last_shown.is_some_and(|l| {
            due.saturating_duration_since(l) < Duration::from_secs_f64(1.0 / MAX_FPS)
        });
        if (now > due + Duration::from_millis(250) || too_soon) && self.shown > 0 {
            return Ok(true);
        }
        if due > now {
            thread::sleep(due - now);
        }
        let (cols, rows) = *self.size.lock().map_err(|_| "the player stopped")?;
        if cols == 0 || rows == 0 {
            return Ok(true);
        }
        // Scaled here, off the UI thread, to the pixels of its box.
        let img = DynamicImage::ImageRgb8(picture).resize(
            u32::from(cols) * self.cell.0,
            u32::from(rows) * self.cell.1,
            image::imageops::FilterType::Triangle,
        );
        let Ok(p) = self.picker.new_protocol(
            img,
            Size::new(cols, rows),
            Resize::Scale(Some(FilterType::Triangle)),
        ) else {
            return Ok(true);
        };
        if self.tx.send(Msg::Frame(Box::new(p))).is_err() {
            return Ok(false);
        }
        self.last_shown = Some(Instant::now());
        self.shown += 1;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::Path;

    /// Serve the HLS fixture over HTTP, every path from `e2e/atago/testdata/hls`.
    fn serve() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata/hls");
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/");
                let path = path
                    .split('?')
                    .next()
                    .unwrap_or(path)
                    .trim_start_matches('/');
                let (status, body) = match std::fs::read(root.join(path)) {
                    Ok(b) => ("200 OK", b),
                    Err(_) => ("404 Not Found", b"missing".to_vec()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://{addr}")
    }

    fn run(url: &str) -> (Result<(), String>, usize) {
        let (tx, rx) = channel();
        let stop = AtomicBool::new(false);
        let size = Mutex::new((20, 10));
        let result = play(&Picker::halfblocks(), url, &stop, &size, &tx);
        drop(tx);
        let frames = rx.iter().filter(|m| matches!(m, Msg::Frame(_))).count();
        (result, frames)
    }

    #[test]
    fn a_video_plays_every_picture_in_about_its_own_time() {
        let base = serve();
        let started = Instant::now();
        let (result, frames) = run(&format!("{base}/playlist.m3u8"));
        assert_eq!(result, Ok(()));
        // Ten pictures a second for one second.
        assert!(frames >= 8, "{frames} frames");
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(700), "paced: {took:?}");
    }

    /// Against Bluesky itself; run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the network"]
    fn a_real_bluesky_video_plays() {
        let url = "https://video.bsky.app/watch/did%3Aplc%3Az72i7hdynmk6r22z27h6tvur/bafkreifhuv36ji7vcq3tmdjltceyrfaat6vdccn2pklxf7j7dgsobdlgbm/playlist.m3u8";
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let size = Arc::new(Mutex::new((20, 40)));
        let (s2, z2) = (Arc::clone(&stop), Arc::clone(&size));
        let t = thread::spawn(move || play(&Picker::halfblocks(), url, &s2, &z2, &tx));
        let begin = Instant::now();
        let mut times = Vec::new();
        while begin.elapsed() < Duration::from_secs(4) {
            if let Ok(Msg::Frame(_)) = rx.recv_timeout(Duration::from_millis(50)) {
                times.push(begin.elapsed().as_millis());
            }
        }
        stop.store(true, Ordering::Relaxed);
        assert_eq!(t.join().unwrap(), Ok(()));
        let frames = times.len();
        eprintln!("frame times (ms): {times:?}");
        assert!(frames >= 10, "{frames} frames in 4 seconds");
    }

    #[test]
    fn a_missing_video_is_a_reason_not_a_panic() {
        let base = serve();
        let (result, frames) = run(&format!("{base}/nothing.m3u8"));
        assert_eq!(result, Err("cannot load the video: HTTP 404".into()));
        assert_eq!(frames, 0);
    }
}
