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
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
#[cfg(test)]
use std::sync::mpsc::channel;
#[cfg(test)]
use std::thread;
#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
use image::RgbImage;
use openh264::decoder::Decoder;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::hls::{self, Demuxer};
use terminal_kit::media::player::{Pacer, to_rgb};
#[cfg(all(test, not(coverage)))]
use {
    image::DynamicImage,
    ratatui::layout::Size,
    ratatui_image::{FilterType, Resize},
    terminal_kit::scale,
};

mod ahead;
pub(crate) use ahead::{AheadSession, read_ahead};

/// Whether the selected post plays in the timeline (a trial, off by default).
pub(crate) fn inline_video() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("BSKY_INLINE_VIDEO").is_some_and(|v| v == "1"))
}

/// Largest segment or playlist read.
const MAX_BYTES: u64 = 32 * 1024 * 1024;

pub(crate) use terminal_kit::media::player::State;
/// Play HLS using the common frame queue and lifetime.
pub(crate) fn start(
    picker: Picker,
    url: &str,
    size: (u16, u16),
    generation: u32,
) -> Box<dyn terminal_kit::media::player::Video> {
    terminal_kit::media::player::start_with(picker, url, size, generation, play)
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

#[expect(
    clippy::map_err_ignore,
    reason = "the message is translated whole where it is shown, and the offset of the bad byte means nothing to the reader"
)]
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

fn read_playlists(agent: &ureq::Agent, playlist: &str) -> Result<(String, String), String> {
    let master = text(agent, playlist)?;
    Ok(match hls::pick_variant(&master, playlist) {
        Some(url) => {
            let media = text(agent, &url)?;
            (url, media)
        }
        None => (playlist.to_string(), master),
    })
}

/// Play to the end, or until `stop`.
fn play(
    picker: &Picker,
    playlist: &str,
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Box<Protocol>>,
) -> Result<(), String> {
    let Ok(ready) = ahead::cached(playlist, stop) else {
        return Ok(());
    };
    play_ready(picker, playlist, stop, size, tx, ready)
}

fn play_ready(
    picker: &Picker,
    playlist: &str,
    stop: &AtomicBool,
    size: &Mutex<(u16, u16)>,
    tx: &Sender<Box<Protocol>>,
    ready: Option<Arc<ahead::Startup>>,
) -> Result<(), String> {
    let agent = agent();
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    let segments = match &ready {
        Some(startup) => hls::segments(&startup.media, &startup.media_url),
        None => {
            let (media_url, media) = read_playlists(agent, playlist)?;
            hls::segments(&media, &media_url)
        }
    };
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
            let mut ready = ready;
            for url in segments {
                if let Some(startup) = ready.take()
                    && let Some(bytes) = &startup.first
                {
                    for chunk in bytes.chunks(CHUNK) {
                        if seg_tx.send(Ok(Piece::Data(chunk.to_vec()))).is_err() {
                            return;
                        }
                    }
                    if seg_tx.send(Ok(Piece::End)).is_err() {
                        return;
                    }
                    continue;
                }
                match stream(&agent, &url, &seg_tx) {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(why) => {
                        #[expect(
                            clippy::let_underscore_must_use,
                            reason = "the player stopped taking pieces: the video was closed, and the error is not wanted"
                        )]
                        let _: Result<(), _> = seg_tx.send(Err(why));
                        return;
                    }
                }
            }
        })
        .map_err(|e| crate::i18n::tf("cannot load the video: {}", &[&e.to_string()]))?;
    }
    let mut ended = 0;
    while ended < segments.len() {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        #[expect(
            clippy::map_err_ignore,
            reason = "RecvError says only that the download thread is gone, which the message says"
        )]
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
    pacer.finish(&mut decoder, &mut pending, stop)?;
    if pacer.shown() == 0 {
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
        let frames = rx.iter().count();
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
            if rx.recv_timeout(Duration::from_millis(50)).is_ok() {
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
        let frames = rx.iter().count();
        eprintln!("{frames} frames in {took:.2} s");
        assert!((16.8..18.5).contains(&took), "{took} s");
        assert!(frames as f64 >= 17.0 * 15.0 * 0.9, "{frames} frames");
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
        assert_eq!(rx.iter().count(), 0);
    }

    #[test]
    fn a_player_ends_with_its_state_and_forgets_the_video_on_drop() {
        let base = serve();
        let url = format!("{base}/playlist.m3u8");
        let mut player = start(Picker::halfblocks(), &url, (20, 10), 1);
        assert!(player.is(&url, 1));
        assert!(!player.is(&url, 2));
        assert_eq!(*player.state(), State::Loading);
        player.resize((30, 12));
        let begin = Instant::now();
        while *player.state() != State::Ended && begin.elapsed() < Duration::from_secs(10) {
            player.poll();
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*player.state(), State::Ended);
        assert!(player.frame().is_some());

        let broken = start(
            Picker::halfblocks(),
            &format!("{base}/gone.m3u8"),
            (20, 10),
            1,
        );
        let mut broken = broken;
        let begin = Instant::now();
        while *broken.state() == State::Loading && begin.elapsed() < Duration::from_secs(10) {
            broken.poll();
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            *broken.state(),
            State::Warning("cannot load the video: HTTP 404".into())
        );
        assert!(broken.frame().is_none());
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
            #[allow(
                deprecated,
                reason = "a picker of a known cell size, which a test has no terminal to ask for"
            )]
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
        #[allow(
            deprecated,
            reason = "a picker of a known cell size, which a test has no terminal to ask for"
        )]
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
