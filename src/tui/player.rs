//! Playing a Bluesky video in the terminal, without sound.
//!
//! A thread fetches the HLS playlists and segments, pulls the H.264 out of
//! each segment, decodes it with OpenH264 (built into bs, so nothing else is
//! installed), and turns each picture into the terminal's image protocol at
//! the size of the box it is shown in, paced by the video's own clock. The
//! UI draws the newest picture it has been handed. A video bs cannot play is
//! not an error: the viewer shows its thumbnail and says why.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use image::{DynamicImage, RgbImage};
use openh264::decoder::Decoder;
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
    // The stream's clock against the wall clock: set by the first picture,
    // and set again after a wait for the network, so a slow download pauses
    // the video instead of skipping it.
    let mut clock: Option<(Instant, u64)> = None;
    let mut last_shown: Option<Instant> = None;
    let mut shown = 0usize;
    let (cw, ch) = {
        let f = picker.font_size();
        (u32::from(f.width.max(1)), u32::from(f.height.max(1)))
    };
    for (n, url) in segments.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let bytes = fetch(&agent, url)?;
        demux.feed(&bytes);
        let units = if n + 1 == segments.len() {
            demux.finish()
        } else {
            demux.take()
        };
        if let Some(kind) = demux.unsupported {
            return Err(format!(
                "this video is not H.264 (stream type 0x{kind:02x}), which bs cannot decode"
            ));
        }
        for unit in units {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            let Ok(Some(yuv)) = decoder.decode(&unit.data) else {
                continue;
            };
            // When this picture is due, by the stream's clock.
            let now = Instant::now();
            let due = match (unit.pts, clock) {
                (Some(p), Some((at, first))) => {
                    let due =
                        at + Duration::from_secs_f64(p.saturating_sub(first) as f64 / 90_000.0);
                    if now > due + Duration::from_secs(1) {
                        clock = Some((now, p));
                        now
                    } else {
                        due
                    }
                }
                (Some(p), None) => {
                    clock = Some((now, p));
                    now
                }
                (None, _) => now,
            };
            // Late, or too soon after the last one: decoded, not shown.
            // Measured from when this one is due, not from now: a picture
            // decoded early is still shown when its time comes.
            let too_soon = last_shown.is_some_and(|l| {
                due.saturating_duration_since(l) < Duration::from_secs_f64(1.0 / MAX_FPS)
            });
            if (now > due + Duration::from_millis(250) || too_soon) && shown > 0 {
                continue;
            }
            if due > now {
                thread::sleep(due - now);
            }
            let (w, h) = yuv.dimensions();
            let mut rgb = vec![0u8; w * h * 3];
            yuv.write_rgb8(&mut rgb);
            let Some(img) = RgbImage::from_raw(w as u32, h as u32, rgb) else {
                continue;
            };
            let (cols, rows) = *size.lock().map_err(|_| "the player stopped")?;
            if cols == 0 || rows == 0 {
                continue;
            }
            // Scaled here, off the UI thread, to the pixels of its box.
            let img = DynamicImage::ImageRgb8(img).resize(
                u32::from(cols) * cw,
                u32::from(rows) * ch,
                image::imageops::FilterType::Triangle,
            );
            let Ok(p) = picker.new_protocol(
                img,
                Size::new(cols, rows),
                Resize::Scale(Some(FilterType::Triangle)),
            ) else {
                continue;
            };
            if tx.send(Msg::Frame(Box::new(p))).is_err() {
                return Ok(());
            }
            last_shown = Some(Instant::now());
            shown += 1;
        }
    }
    if shown == 0 {
        return Err("no picture of the video could be decoded".into());
    }
    Ok(())
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

    #[test]
    fn a_missing_video_is_a_reason_not_a_panic() {
        let base = serve();
        let (result, frames) = run(&format!("{base}/nothing.m3u8"));
        assert_eq!(result, Err("cannot load the video: HTTP 404".into()));
        assert_eq!(frames, 0);
    }
}
