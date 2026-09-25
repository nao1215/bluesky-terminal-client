//! Playing a Bluesky video in the terminal, without sound.
//!
//! A thread fetches the HLS playlists and segments, pulls the H.264 out of
//! each segment, decodes it with OpenH264 (built into bsky, so nothing else is
//! installed), and turns each picture into the terminal's image protocol at
//! the size of the box it is shown in, paced by the video's own clock. The
//! UI draws the newest picture it has been handed. A video bsky cannot play is
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
use ratatui_image::picker::{Capability, Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::protocol::kitty::Kitty;
use ratatui_image::{FilterType, Resize};

use crate::hls::{self, Demuxer};
use crate::tui::scale;

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
            crate::tui::spawn("bsky-player", move || {
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
        .map_err(|e| crate::i18n::tf("cannot load the video: {}", &[&e.to_string()]))?;
    if !resp.status().is_success() {
        return Err(crate::i18n::tf(
            "cannot load the video: HTTP {}",
            &[&(resp.status().as_u16()).to_string()],
        ));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|e| crate::i18n::tf("cannot load the video: {}", &[&e.to_string()]))
}

/// Bytes of a segment handed on at a time: the demuxer and the decoder start
/// on a segment as soon as its first part has arrived.
const CHUNK: usize = 32 * 1024;

/// What the download thread hands on: part of the segment under way, or
/// its end.
enum Piece {
    Data(Vec<u8>),
    End,
}

/// Download `url`, handing it on in pieces as they arrive. Returns false
/// once nobody takes them.
fn stream(
    agent: &ureq::Agent,
    url: &str,
    tx: &std::sync::mpsc::SyncSender<Result<Piece, String>>,
) -> Result<bool, String> {
    use std::io::Read;
    let mut resp = agent
        .get(url)
        .call()
        .map_err(|e| crate::i18n::tf("cannot load the video: {}", &[&e.to_string()]))?;
    if !resp.status().is_success() {
        return Err(crate::i18n::tf(
            "cannot load the video: HTTP {}",
            &[&(resp.status().as_u16()).to_string()],
        ));
    }
    let mut body = resp.body_mut().with_config().limit(MAX_BYTES).reader();
    loop {
        let mut buf = vec![0u8; CHUNK];
        let mut len = 0;
        // Fill the piece, unless the body ends first.
        while len < CHUNK {
            match body.read(&mut buf[len..]) {
                Ok(0) => break,
                Ok(n) => len += n,
                Err(e) => {
                    return Err(crate::i18n::tf(
                        "cannot load the video: {}",
                        &[&e.to_string()],
                    ));
                }
            }
        }
        if len == 0 {
            return Ok(tx.send(Ok(Piece::End)).is_ok());
        }
        buf.truncate(len);
        if tx.send(Ok(Piece::Data(buf))).is_err() {
            return Ok(false);
        }
    }
}

fn text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    String::from_utf8(fetch(agent, url)?)
        .map_err(|_| crate::i18n::n!("the video's playlist is not text").into())
}

/// One agent for every video played, so the next one reuses the
/// connections this one opened instead of paying for new handshakes.
fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(crate::api::agent)
}

/// Playlists read before the video is played: the URL played, the media
/// playlist chosen from it, that playlist's text, and when they were read.
type ReadPlaylists = Vec<(String, String, String, Instant)>;

fn read_ahead_playlists() -> &'static Mutex<ReadPlaylists> {
    static AHEAD: std::sync::OnceLock<Mutex<ReadPlaylists>> = std::sync::OnceLock::new();
    AHEAD.get_or_init(Mutex::default)
}

/// Videos whose playlists are kept read ahead.
const PLAYLISTS_KEPT: usize = 8;
/// How long playlists read ahead are used for. The media playlist's address
/// carries the session the video server opened for it, so they are used
/// once, soon; a video played again reads them again.
const PLAYLISTS_FOR: Duration = Duration::from_secs(300);

/// The media playlist to play `playlist` from, and its text: the one read
/// ahead, else read now. Reading them is two round trips before the first
/// segment can be asked for.
fn playlists(agent: &ureq::Agent, playlist: &str) -> Result<(String, String), String> {
    if let Ok(mut kept) = read_ahead_playlists().lock()
        && let Some(i) = kept.iter().position(|(p, _, _, _)| p == playlist)
    {
        let (_, url, media, at) = kept.remove(i);
        if at.elapsed() < PLAYLISTS_FOR {
            return Ok((url, media));
        }
    }
    let master = text(agent, playlist)?;
    Ok(match hls::pick_variant(&master, playlist) {
        Some(url) => {
            let media = text(agent, &url)?;
            (url, media)
        }
        None => (playlist.to_string(), master),
    })
}

/// Read the playlists of `playlist` on a thread of its own, for the video
/// to start without them if it is played: `Space` on a post the selection
/// rests on then waits only for the first segment. It also opens the
/// connection to the video server.
pub fn read_ahead(playlist: &str) {
    let playlist = playlist.to_string();
    if read_ahead_playlists().lock().is_ok_and(|kept| {
        kept.iter()
            .any(|(p, _, _, at)| *p == playlist && at.elapsed() < PLAYLISTS_FOR)
    }) {
        return;
    }
    crate::tui::spawn("bsky-playlists", move || {
        let Ok((url, media)) = playlists(agent(), &playlist) else {
            return;
        };
        if let Ok(mut kept) = read_ahead_playlists().lock() {
            kept.retain(|(p, _, _, _)| *p != playlist);
            if kept.len() == PLAYLISTS_KEPT {
                kept.remove(0);
            }
            kept.push((playlist, url, media, Instant::now()));
        }
    });
}

/// Play to the end, or until `stop`.
fn play(
    picker: &Picker,
    playlist: &str,
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Msg>,
) -> Result<(), String> {
    let agent = agent();
    let (media_url, media) = playlists(agent, playlist)?;
    let segments = hls::segments(&media, &media_url);
    if segments.is_empty() {
        return Err(crate::i18n::n!("the video's playlist lists nothing to play").into());
    }
    let mut decoder = decoder()?;
    let mut demux = Demuxer::new();
    let mut pacer = Pacer::new(picker, size, tx);
    // Pictures come out of the decoder in the order they are shown, which
    // with B-frames is not the order they go in; each takes the earliest
    // presentation time not yet used.
    let mut pending: BinaryHeap<Reverse<u64>> = BinaryHeap::new();
    // The segments download ahead on a thread of their own, a couple of
    // segments' worth at most, in pieces, so the first picture is decoded
    // from the first piece of the first segment instead of after all of it:
    // on a slow link that was most of the wait before a video started.
    let (seg_tx, seg_rx) = std::sync::mpsc::sync_channel::<Result<Piece, String>>(64);
    {
        let (segments, agent) = (segments.clone(), agent.clone());
        crate::tui::spawn("bsky-segments", move || {
            for url in segments {
                match stream(&agent, &url, &seg_tx) {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(why) => {
                        let _ = seg_tx.send(Err(why));
                        return;
                    }
                }
            }
        });
    }
    let mut ended = 0;
    while ended < segments.len() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        let piece = seg_rx
            .recv()
            .map_err(|_| crate::i18n::n!("the video download stopped").to_string())??;
        let units = match piece {
            Piece::Data(bytes) => {
                demux.feed(&bytes);
                demux.take()
            }
            Piece::End => {
                ended += 1;
                if ended < segments.len() {
                    continue;
                }
                demux.finish()
            }
        };
        if let Some(kind) = demux.unsupported {
            return Err(crate::i18n::tf(
                "this video is not H.264 (stream type 0x{}), which bsky cannot decode",
                &[&format!("{:02x}", kind)],
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
        return Err(crate::i18n::n!("no picture of the video could be decoded").into());
    }
    Ok(())
}

/// A decoder that holds pictures until their turn. Forcing one out after
/// every frame (the crate's default) breaks streams with B-frames, which
/// Bluesky's are: most frames then fail to decode.
fn decoder() -> Result<Decoder, String> {
    use openh264::OpenH264API;
    use openh264::decoder::{DecoderConfig, Flush};
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    Decoder::with_api_config(OpenH264API::from_source(), config)
        .map_err(|e| crate::i18n::tf("cannot start the video decoder: {}", &[&e.to_string()]))
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
        let (cols, rows) = *self
            .size
            .lock()
            .map_err(|_| crate::i18n::t("the player stopped"))?;
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

    // Times that jump far ahead (the next part of a stream starting at
    // another clock) play on at once; they do not wait out the jump.
    #[test]
    fn a_jump_ahead_in_the_times_does_not_stop_the_video() {
        let played = thread::spawn(|| {
            let (tx, _rx) = channel();
            let size = Mutex::new((20, 10));
            let picker = Picker::halfblocks();
            let mut pacer = Pacer::new(&picker, &size, &tx);
            let pic = || Some(RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])));
            pacer.show(pic(), Some(0)).unwrap();
            pacer.show(pic(), Some(3600 * 90_000)).unwrap();
        });
        let started = Instant::now();
        while !played.is_finished() && started.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            played.is_finished(),
            "still waiting after {:?}",
            started.elapsed()
        );
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
    // Not built for scripts/coverage.sh: it never runs there, so it would
    // count as untested code.
    #[cfg(not(coverage))]
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
        assert!(frames >= 50, "{frames} frames in 4 seconds");
    }

    /// The whole video plays in its own length (17.3 s by its playlist).
    // Not built for scripts/coverage.sh: it never runs there, so it would
    // count as untested code.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "needs the network"]
    fn a_real_bluesky_video_takes_its_own_time() {
        let url = "https://video.bsky.app/watch/did%3Aplc%3Az72i7hdynmk6r22z27h6tvur/bafkreifhuv36ji7vcq3tmdjltceyrfaat6vdccn2pklxf7j7dgsobdlgbm/playlist.m3u8";
        let (tx, rx) = channel();
        let (stop, size) = (AtomicBool::new(false), Mutex::new((20, 40)));
        let begin = Instant::now();
        assert_eq!(play(&Picker::halfblocks(), url, &stop, &size, &tx), Ok(()));
        let took = begin.elapsed().as_secs_f64();
        drop(tx);
        let frames = rx.iter().filter(|m| matches!(m, Msg::Frame(_))).count();
        eprintln!("{frames} frames in {took:.2} s");
        assert!((16.8..18.5).contains(&took), "{took} s");
        assert!(frames as f64 >= 17.0 * MAX_FPS * 0.9, "{frames} frames");
    }

    /// Serve `routes` (path to body) over HTTP; any other path is a 404.
    // Space on a video waited for its two playlists before the first
    // segment could be asked for. Read ahead, they are not asked for again.
    #[test]
    fn playlists_read_ahead_are_not_read_again() {
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let counted = std::sync::Arc::clone(&hits);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = if path.ends_with("playlist.m3u8") {
                    "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=640x360\n360p/video.m3u8\n"
                } else {
                    "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nvideo0.ts\n#EXT-X-ENDLIST\n"
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let playlist = format!("{base}/watch/ahead-🎬/playlist.m3u8");
        read_ahead(&playlist);
        let start = Instant::now();
        while !read_ahead_playlists()
            .lock()
            .unwrap()
            .iter()
            .any(|(p, _, _, _)| *p == playlist)
        {
            assert!(start.elapsed() < Duration::from_secs(5), "never read");
            thread::sleep(Duration::from_millis(5));
        }
        // Asked again while they are kept, they are not read twice.
        read_ahead(&playlist);
        thread::sleep(Duration::from_millis(50));
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        let (url, media) = playlists(agent(), &playlist).unwrap();
        assert_eq!(url, format!("{base}/watch/ahead-🎬/360p/video.m3u8"));
        assert_eq!(hls::segments(&media, &url).len(), 1);
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
        // Played again, the video's playlists are read again: the session
        // in the media playlist's address is used once.
        playlists(agent(), &playlist).unwrap();
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    fn serve_routes(routes: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req.split_whitespace().nth(1).unwrap_or("/");
                let found = routes.iter().find(|(p, _)| *p == path);
                let (status, body) = match found {
                    Some((_, b)) => ("200 OK", b.clone()),
                    None => ("404 Not Found", b"missing".to_vec()),
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

    fn fixture_segment() -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata/hls/v/seg0.ts"),
        )
        .unwrap()
    }

    const ONE_SEGMENT: &str =
        "#EXTM3U\n#EXT-X-TARGETDURATION:1\n#EXTINF:1.000,\nseg0.ts\n#EXT-X-ENDLIST\n";

    /// One 188-byte transport stream packet on `pid` carrying `payload`.
    fn ts_packet(pid: u16, payload: &[u8]) -> Vec<u8> {
        let mut p = vec![0x47, 0x40 | (pid >> 8) as u8, pid as u8, 0x10];
        p.extend_from_slice(payload);
        p.resize(188, 0xff);
        p
    }

    /// A program whose only video stream is HEVC (stream type 0x24).
    fn hevc_segment() -> Vec<u8> {
        // PAT: program 1 has its PMT on PID 0x1000 (the CRC is not checked).
        let pat = [
            0, 0x00, 0xb0, 0x0d, 0, 1, 0xc1, 0, 0, 0, 1, 0xf0, 0x00, 0, 0, 0, 0,
        ];
        // PMT: PCR on 0x100, no program info, one HEVC stream on 0x100.
        let pmt = [
            0, 0x02, 0xb0, 0x12, 0, 1, 0xc1, 0, 0, 0xe1, 0x00, 0xf0, 0x00, 0x24, 0xe1, 0x00, 0xf0,
            0x00, 0, 0, 0, 0,
        ];
        [ts_packet(0, &pat), ts_packet(0x1000, &pmt)].concat()
    }

    #[test]
    fn a_media_playlist_without_variants_plays_directly() {
        let base = serve_routes(vec![
            ("/media.m3u8", ONE_SEGMENT.into()),
            ("/seg0.ts", fixture_segment()),
        ]);
        let (result, frames) = run(&format!("{base}/media.m3u8"));
        assert_eq!(result, Ok(()));
        assert!(frames >= 8, "{frames} frames");
    }

    #[rstest::rstest]
    #[case::nothing_listed(
        "#EXTM3U\n#EXT-X-ENDLIST\n",
        None,
        "the video's playlist lists nothing to play"
    )]
    #[case::not_h264(
        ONE_SEGMENT,
        Some(hevc_segment()),
        "this video is not H.264 (stream type 0x24), which bsky cannot decode"
    )]
    #[case::no_picture(ONE_SEGMENT, Some(vec![0x47; 188 * 4]), "no picture of the video could be decoded")]
    #[case::segment_missing(ONE_SEGMENT, None, "cannot load the video: HTTP 404")]
    fn a_video_that_cannot_play_says_why(
        #[case] playlist: &'static str,
        #[case] segment: Option<Vec<u8>>,
        #[case] why: &str,
    ) {
        let mut routes = vec![("/media.m3u8", playlist.as_bytes().to_vec())];
        if let Some(seg) = segment {
            routes.push(("/seg0.ts", seg));
        }
        let base = serve_routes(routes);
        let (result, frames) = run(&format!("{base}/media.m3u8"));
        assert_eq!(result, Err(why.to_string()));
        assert_eq!(frames, 0);
    }

    #[test]
    fn a_stopped_player_shows_nothing_more_and_ends_quietly() {
        let base = serve();
        let (tx, rx) = channel();
        let stop = AtomicBool::new(true);
        let size = Mutex::new((20, 10));
        let result = play(
            &Picker::halfblocks(),
            &format!("{base}/playlist.m3u8"),
            &stop,
            &size,
            &tx,
        );
        drop(tx);
        assert_eq!(result, Ok(()));
        assert_eq!(rx.iter().filter(|m| matches!(m, Msg::Frame(_))).count(), 0);
    }

    #[test]
    fn a_player_ends_with_its_state_and_forgets_the_video_on_drop() {
        let base = serve();
        let url = format!("{base}/playlist.m3u8");
        let mut player = Player::start(Picker::halfblocks(), &url, (20, 10), 1);
        assert!(player.is(&url, 1));
        assert!(!player.is(&url, 2));
        assert_eq!(player.state, State::Loading);
        player.resize((30, 12));
        let begin = Instant::now();
        while player.state != State::Ended && begin.elapsed() < Duration::from_secs(10) {
            player.poll();
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(player.state, State::Ended);
        assert!(player.frame().is_some());

        let broken = Player::start(
            Picker::halfblocks(),
            &format!("{base}/gone.m3u8"),
            (20, 10),
            1,
        );
        let mut broken = broken;
        let begin = Instant::now();
        while broken.state == State::Loading && begin.elapsed() < Duration::from_secs(10) {
            broken.poll();
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            broken.state,
            State::Warning("cannot load the video: HTTP 404".into())
        );
        assert!(broken.frame().is_none());
    }

    /// Prints how long a 1280 x 720 video picture takes to be scaled and
    /// encoded for its box, the work behind every frame shown:
    /// `cargo test --release video_picture -- --ignored --nocapture`.
    #[test]
    fn kitty_pictures_of_a_video_take_two_images_in_turn_and_redraw_every_cell() {
        #[allow(deprecated)]
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

    #[cfg(not(coverage))]
    #[test]
    #[ignore = "measurement"]
    fn a_video_picture_until_encoded() {
        use ratatui_image::picker::ProtocolType;
        let picture = RgbImage::from_fn(1280, 720, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8])
        });
        for proto in [
            ProtocolType::Kitty,
            ProtocolType::Sixel,
            ProtocolType::Iterm2,
        ] {
            #[allow(deprecated)]
            let mut picker = Picker::from_fontsize((10, 20).into());
            picker.set_protocol_type(proto);
            for (cols, rows) in [(160u16, 45u16), (80, 24)] {
                let area = Size::new(cols, rows);
                let mut samples: Vec<f64> = (0..11)
                    .map(|_| {
                        let start = Instant::now();
                        let img = scale::to_box(
                            &DynamicImage::ImageRgb8(picture.clone()),
                            area,
                            (10, 20),
                        );
                        let _ = picker
                            .new_protocol(img, area, Resize::Scale(Some(FilterType::Triangle)))
                            .unwrap();
                        start.elapsed().as_secs_f64() * 1000.0
                    })
                    .collect();
                samples.sort_by(f64::total_cmp);
                println!(
                    "{proto:?} {cols}x{rows} cells: {:.1} ms a picture (median of 11)",
                    samples[5]
                );
            }
        }
    }

    #[test]
    fn a_missing_video_is_a_reason_not_a_panic() {
        let base = serve();
        let (result, frames) = run(&format!("{base}/nothing.m3u8"));
        assert_eq!(result, Err("cannot load the video: HTTP 404".into()));
        assert_eq!(frames, 0);
    }
}

#[cfg(test)]
mod latency {
    use super::*;

    /// Where the time to a video's first picture goes, for the playlists in
    /// $BSKY_VIDEOS (one per line):
    /// `BSKY_VIDEOS=... cargo test --release first_picture -- --ignored --nocapture`.
    #[cfg(not(coverage))]
    #[test]
    #[ignore = "measurement"]
    fn first_picture() {
        let Ok(list) = std::env::var("BSKY_VIDEOS") else {
            return;
        };
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((10, 20).into());
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
        for playlist in list.lines().filter(|l| !l.is_empty()) {
            let agent = crate::api::agent();
            let t0 = Instant::now();
            let master = text(&agent, playlist).unwrap();
            let t_master = t0.elapsed();
            let url = hls::pick_variant(&master, playlist).unwrap();
            let media = text(&agent, &url).unwrap();
            let t_media = t0.elapsed();
            let segs = hls::segments(&media, &url);
            let seg = fetch(&agent, &segs[0]).unwrap();
            let t_seg = t0.elapsed();
            let mut demux = Demuxer::new();
            demux.feed(&seg);
            let units = demux.take();
            let mut decoder = decoder().unwrap();
            let mut first = None;
            for u in &units {
                if let Ok(Some(yuv)) = decoder.decode(&u.data) {
                    first = to_rgb(&yuv);
                    break;
                }
            }
            let t_dec = t0.elapsed();
            let pic = first.unwrap();
            let area = Size::new(100, 30);
            let img = scale::to_box(&DynamicImage::ImageRgb8(pic), area, (10, 20));
            let _ = picker
                .new_protocol(img, area, Resize::Scale(Some(FilterType::Triangle)))
                .unwrap();
            let t_enc = t0.elapsed();
            println!(
                "{} segs, first {} KB, {} units: master {:?} media {:?} segment {:?} decoded {:?} encoded {:?}",
                segs.len(),
                seg.len() / 1024,
                units.len(),
                t_master,
                t_media,
                t_seg,
                t_dec,
                t_enc
            );
        }
    }
}
