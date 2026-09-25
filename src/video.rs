//! Local videos: what a video file is (its type, shape and length, read from
//! its header without decoding a frame) and getting it ready to post.
//!
//! Bluesky shows animation only as video, so an animated GIF is posted as a
//! video too: Bluesky's video service turns it into one. Nothing here decodes
//! or encodes video, so bsky needs no codec and no external program.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};

/// Most bytes a video may have.
pub const MAX_VIDEO_BYTES: u64 = 100 * 1024 * 1024;
/// Longest a video may run, in seconds.
pub const MAX_VIDEO_SECONDS: f64 = 180.0;
/// File name extensions shown as videos.
const EXTENSIONS: [&str; 6] = ["mp4", "m4v", "mov", "webm", "mpeg", "mpg"];

/// Whether a file name looks like a video.
pub fn is_video_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Whether the file is a GIF with more than one frame.
pub fn is_animated_gif(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    gif_has_two_frames(BufReader::new(file)).unwrap_or(false)
}

/// Whether a GIF holds a second frame, read from its block structure alone.
/// No frame is decoded: a decoder draws every frame on a canvas of the size
/// the header claims, up to 65535 x 65535, so a tiny file could take seconds
/// and gigabytes. `None` when the file ends or breaks before the answer.
fn gif_has_two_frames<R: Read>(mut r: R) -> Option<bool> {
    fn byte<R: Read>(r: &mut R) -> Option<u8> {
        let mut b = [0u8; 1];
        r.read_exact(&mut b).ok()?;
        Some(b[0])
    }
    fn skip<R: Read>(r: &mut R, n: u64) -> Option<()> {
        (std::io::copy(&mut r.by_ref().take(n), &mut std::io::sink()).ok()? == n).then_some(())
    }
    /// Data sub-blocks: a length byte, that many bytes, until a zero length.
    fn skip_sub_blocks<R: Read>(r: &mut R) -> Option<()> {
        loop {
            match byte(r)? {
                0 => return Some(()),
                n => skip(r, u64::from(n))?,
            }
        }
    }
    /// A color table of `2^(n+1)` RGB entries, present when bit 7 is set.
    fn color_table_len(flags: u8) -> u64 {
        if flags & 0x80 == 0 {
            0
        } else {
            3 << ((flags & 7) + 1)
        }
    }

    let mut header = [0u8; 13];
    r.read_exact(&mut header).ok()?;
    if &header[..3] != b"GIF" {
        return Some(false);
    }
    skip(&mut r, color_table_len(header[10]))?;
    let mut frames = 0;
    loop {
        match byte(&mut r)? {
            // An extension: its label, then its data.
            0x21 => {
                byte(&mut r)?;
                skip_sub_blocks(&mut r)?;
            }
            // An image: its descriptor, a local color table, the LZW code
            // size, then the compressed pixels, skipped.
            0x2c => {
                frames += 1;
                if frames == 2 {
                    return Some(true);
                }
                let mut descriptor = [0u8; 9];
                r.read_exact(&mut descriptor).ok()?;
                skip(&mut r, color_table_len(descriptor[8]))?;
                byte(&mut r)?;
                skip_sub_blocks(&mut r)?;
            }
            // The trailer, or something a GIF does not hold.
            _ => return Some(false),
        }
    }
}

/// What a video's header says about it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VideoInfo {
    /// Width and height as shown (a rotated phone video is taller than wide).
    pub dims: Option<(u32, u32)>,
    pub seconds: Option<f64>,
}

/// Read the shape and length of an MP4 or QuickTime file from its `moov`
/// box. Other formats, and files that are not what they claim, give nothing.
pub fn probe(path: &Path) -> VideoInfo {
    File::open(path)
        .ok()
        .and_then(|f| read_moov(&mut BufReader::new(f)))
        .map(|moov| parse_moov(&moov))
        .unwrap_or_default()
}

/// A length the way a player shows it: 0:07, 2:45.
pub fn format_seconds(s: f64) -> String {
    minutes_seconds(s.round())
}

/// Whole seconds as minutes and seconds.
fn minutes_seconds(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The `moov` box's content. It may sit after the media data, so the boxes
/// before it are skipped by seeking, never read.
fn read_moov<R: Read + Seek>(r: &mut R) -> Option<Vec<u8>> {
    for _ in 0..64 {
        let mut head = [0u8; 8];
        r.read_exact(&mut head).ok()?;
        let size = u64::from(u32::from_be_bytes(head[..4].try_into().ok()?));
        let (size, header) = match size {
            1 => {
                let mut large = [0u8; 8];
                r.read_exact(&mut large).ok()?;
                (u64::from_be_bytes(large), 16)
            }
            0 => return None,
            n => (n, 8),
        };
        let body = size.checked_sub(header)?;
        if &head[4..] == b"moov" {
            // A header describing tracks is small; a huge one is not a video
            // worth trusting.
            if body > 32 * 1024 * 1024 {
                return None;
            }
            let mut buf = vec![0u8; body as usize];
            r.read_exact(&mut buf).ok()?;
            return Some(buf);
        }
        r.seek(SeekFrom::Current(i64::try_from(body).ok()?)).ok()?;
    }
    None
}

/// The file name extension of a video's type.
pub fn extension(mime: &str) -> &'static str {
    match mime {
        "video/quicktime" => "mov",
        "video/webm" => "webm",
        "video/mpeg" => "mpg",
        "image/gif" => "gif",
        _ => "mp4",
    }
}

/// The boxes that say where, when, and with what a video was made: user
/// data (where a phone writes the place, `©xyz`), metadata (`meta`, with
/// Apple's location keys), `loci`, and `uuid` boxes (XMP). A picture is
/// posted without its location, and so is a video.
const PRIVATE_BOXES: &[&[u8; 4]] = &[b"udta", b"meta", b"loci", b"uuid"];

/// The boxes that hold other boxes, where the private ones are looked for.
const CONTAINERS: &[&[u8; 4]] = &[b"moov", b"trak", b"mdia", b"minf", b"edts"];

/// Blank the private boxes of an MP4 or QuickTime file in place: each
/// becomes a `free` box of the same size, its content zeroed, so every
/// offset in the file (the media data's included) stays where it was.
pub fn strip_metadata(file: &mut [u8]) {
    fn walk(buf: &mut [u8]) {
        let mut at = 0;
        while at + 8 <= buf.len() {
            let size = u32::from_be_bytes(buf[at..at + 4].try_into().expect("four bytes")) as usize;
            let (size, header) = match size {
                0 => (buf.len() - at, 8),
                1 => {
                    let Some(large) = buf.get(at + 8..at + 16) else {
                        return;
                    };
                    let large = u64::from_be_bytes(large.try_into().expect("eight bytes"));
                    (usize::try_from(large).unwrap_or(usize::MAX), 16)
                }
                n => (n, 8),
            };
            // Compared with what is left, not added to `at`: a 64-bit size
            // near the largest overflows the sum.
            if size < header || size > buf.len() - at {
                return;
            }
            let kind: [u8; 4] = buf[at + 4..at + 8].try_into().expect("four bytes");
            let body = at + header..at + size;
            if PRIVATE_BOXES.contains(&&kind) || kind == *b"\xA9xyz" {
                buf[at + 4..at + 8].copy_from_slice(b"free");
                buf[body].fill(0);
            } else if CONTAINERS.contains(&&kind) {
                walk(&mut buf[body]);
            }
            at += size;
        }
    }
    walk(file);
}

/// The child boxes of a box's content, as (type, content).
fn boxes(mut buf: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    std::iter::from_fn(move || {
        if buf.len() < 8 {
            return None;
        }
        let size = u32::from_be_bytes(buf[..4].try_into().ok()?) as usize;
        let kind: [u8; 4] = buf[4..8].try_into().ok()?;
        if size < 8 || size > buf.len() {
            return None;
        }
        let body = &buf[8..size];
        buf = &buf[size..];
        Some((kind, body))
    })
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

fn parse_moov(moov: &[u8]) -> VideoInfo {
    let mut info = VideoInfo::default();
    for (kind, body) in boxes(moov) {
        match &kind {
            b"mvhd" => info.seconds = mvhd_seconds(body),
            b"trak" if info.dims.is_none() => {
                info.dims = boxes(body)
                    .find(|(k, _)| k == b"tkhd")
                    .and_then(|(_, tkhd)| tkhd_dims(tkhd));
            }
            _ => {}
        }
    }
    info
}

fn mvhd_seconds(b: &[u8]) -> Option<f64> {
    let (timescale, duration) = match b.first()? {
        0 => (be32(b, 12)?, u64::from(be32(b, 16)?)),
        1 => (be32(b, 20)?, be64(b, 24)?),
        _ => return None,
    };
    (timescale > 0).then(|| duration as f64 / f64::from(timescale))
}

/// The track's display size; zero for an audio track, which is skipped.
fn tkhd_dims(b: &[u8]) -> Option<(u32, u32)> {
    // After version and flags: times and ids (20 bytes, or 32 in version 1),
    // 8 reserved, layer, group, volume, 2 reserved, the 3x3 matrix, then
    // width and height in 16.16 fixed point.
    let matrix = match b.first()? {
        0 => 4 + 20 + 8 + 8,
        1 => 4 + 32 + 8 + 8,
        _ => return None,
    };
    let (a, c) = (be32(b, matrix)? as i32, be32(b, matrix + 4)? as i32);
    let w = be32(b, matrix + 36)? >> 16;
    let h = be32(b, matrix + 40)? >> 16;
    if w == 0 || h == 0 {
        return None;
    }
    // A matrix that turns the picture a quarter shows it the other way up.
    Some(if a == 0 && c != 0 { (h, w) } else { (w, h) })
}

/// The other boxes a QuickTime file may open with. It had no `ftyp` box
/// before 2001, and cameras and editors still write files without one, so a
/// file whose first box is one of these is a QuickTime movie.
const QUICKTIME_FIRST: &[&[u8; 4]] = &[b"moov", b"mdat", b"free", b"skip", b"wide", b"pnot"];

/// Guess a video's MIME type from its first bytes.
pub fn sniff_mime(b: &[u8]) -> Option<&'static str> {
    match b {
        [
            _,
            _,
            _,
            _,
            b'f',
            b't',
            b'y',
            b'p',
            b'q',
            b't',
            b' ',
            b' ',
            ..,
        ] => Some("video/quicktime"),
        [_, _, _, _, b'f', b't', b'y', b'p', ..] => Some("video/mp4"),
        [0x1a, 0x45, 0xdf, 0xa3, ..] => Some("video/webm"),
        [0, 0, 1, 0xba | 0xb3, ..] => Some("video/mpeg"),
        // A box whose size covers at least its own header, of a kind a file
        // opens with. The size keeps text and other formats out.
        [s0, s1, s2, s3, kind @ ..] if kind.len() >= 4 => {
            let size = u32::from_be_bytes([*s0, *s1, *s2, *s3]);
            let first: &[u8; 4] = kind[..4].try_into().ok()?;
            (size >= 8 && QUICKTIME_FIRST.contains(&first)).then_some("video/quicktime")
        }
        _ => None,
    }
}

/// A video ready to upload.
#[derive(Debug, Clone, PartialEq)]
pub struct Prepared {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub dims: Option<(u32, u32)>,
}

/// The bytes of `file`, or its length when that is over `limit`. The read
/// itself stops past the limit, so a file that grows while it is read is
/// refused too.
fn read_at_most(file: &Path, limit: u64) -> std::io::Result<std::result::Result<Vec<u8>, u64>> {
    use std::io::Read;
    let f = std::fs::File::open(file)?;
    let len = f.metadata()?.len();
    if len > limit {
        return Ok(Err(len));
    }
    let mut bytes = Vec::new();
    f.take(limit + 1).read_to_end(&mut bytes)?;
    let read = bytes.len() as u64;
    Ok(if read > limit {
        Err(read.max(len))
    } else {
        Ok(bytes)
    })
}

/// Read a video (or an animated GIF) and check it against Bluesky's limits.
/// The error names the file and what is wrong.
pub fn prepare(file: &Path) -> Result<Prepared> {
    let name = file.display();
    let read = read_at_most(file, MAX_VIDEO_BYTES).map_err(|e| {
        Error::io(crate::i18n::tf(
            "cannot read {}: {}",
            &[&name.to_string(), &e.to_string()],
        ))
    })?;
    let bytes = read.map_err(|len| {
        Error::io(crate::i18n::tf(
            "{} is {} MB; videos must be at most {} MB",
            &[
                &name.to_string(),
                &(len / (1024 * 1024)).to_string(),
                &(MAX_VIDEO_BYTES / (1024 * 1024)).to_string(),
            ],
        ))
    })?;
    if bytes.starts_with(b"GIF8") {
        return Ok(Prepared {
            dims: image::image_dimensions(file).ok(),
            bytes,
            mime: "image/gif",
        });
    }
    let mime = sniff_mime(&bytes).ok_or_else(|| {
        Error::io(crate::i18n::tf(
            "{} is not a video bsky can post (MP4, MOV, WebM, MPEG)",
            &[&name.to_string()],
        ))
    })?;
    let mut bytes = bytes;
    if matches!(mime, "video/mp4" | "video/quicktime") {
        strip_metadata(&mut bytes);
    }
    let info = probe(file);
    if let Some(s) = info.seconds
        && s > MAX_VIDEO_SECONDS
    {
        // Rounded up, so that 180.4 s does not read as the 3:00 allowed.
        return Err(Error::io(crate::i18n::tf(
            "{} runs {}; videos can be at most {}",
            &[
                &name.to_string(),
                &(minutes_seconds(s.ceil())),
                &(format_seconds(MAX_VIDEO_SECONDS)),
            ],
        )));
    }
    Ok(Prepared {
        bytes,
        mime,
        dims: info.dims,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::io::Cursor;

    // The size was checked before the file was read, and the read took
    // whatever was there by then: a file still being written got past it.
    #[test]
    fn a_file_is_read_up_to_the_limit_and_refused_past_it() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("v.mp4");
        std::fs::write(&f, [7u8; 10]).unwrap();
        assert_eq!(read_at_most(&f, 10).unwrap(), Ok(vec![7u8; 10]));
        assert_eq!(read_at_most(&f, 9).unwrap(), Err(10));
        assert!(read_at_most(&dir.path().join("none.mp4"), 9).is_err());
    }

    #[test]
    fn a_video_over_the_size_limit_is_refused_with_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("big.mp4");
        let file = std::fs::File::create(&f).unwrap();
        file.set_len(MAX_VIDEO_BYTES + 3 * 1024 * 1024).unwrap();
        let e = prepare(&f).unwrap_err();
        assert!(e.message().contains("103 MB"), "{}", e.message());
    }

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn mvhd(timescale: u32, duration: u32) -> Vec<u8> {
        let mut b = vec![0u8; 100];
        b[12..16].copy_from_slice(&timescale.to_be_bytes());
        b[16..20].copy_from_slice(&duration.to_be_bytes());
        bx(b"mvhd", &b)
    }

    fn tkhd(w: u32, h: u32, rotated: bool) -> Vec<u8> {
        let mut b = vec![0u8; 84];
        let m = 40;
        let one = 0x0001_0000u32;
        let (a, c) = if rotated { (0, one) } else { (one, 0) };
        b[m..m + 4].copy_from_slice(&a.to_be_bytes());
        b[m + 4..m + 8].copy_from_slice(&c.to_be_bytes());
        b[m + 36..m + 40].copy_from_slice(&(w << 16).to_be_bytes());
        b[m + 40..m + 44].copy_from_slice(&(h << 16).to_be_bytes());
        bx(b"tkhd", &b)
    }

    /// An MP4 with its media data before the header, as a camera writes it.
    fn mp4(seconds: f64, w: u32, h: u32, rotated: bool) -> Vec<u8> {
        let mut file = bx(b"ftyp", b"isom\0\0\x02\0isomiso2");
        file.extend(bx(b"mdat", &vec![0u8; 5000]));
        let audio = bx(b"trak", &tkhd(0, 0, false));
        let video = bx(b"trak", &tkhd(w, h, rotated));
        let moov = [mvhd(1000, (seconds * 1000.0) as u32), audio, video].concat();
        file.extend(bx(b"moov", &moov));
        file
    }

    #[test]
    fn the_header_gives_the_shape_and_length_skipping_audio() {
        let data = mp4(12.0, 1920, 1080, false);
        let moov = read_moov(&mut Cursor::new(&data)).unwrap();
        assert_eq!(
            parse_moov(&moov),
            VideoInfo {
                dims: Some((1920, 1080)),
                seconds: Some(12.0)
            }
        );
    }

    #[test]
    fn a_real_video_file_is_read() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata");
        let info = probe(&dir.join("clip.mp4"));
        assert_eq!(info.dims, Some((64, 36)));
        assert!((info.seconds.unwrap() - 1.0).abs() < 0.05, "{info:?}");
        assert!(is_animated_gif(&dir.join("moving.gif")));
        assert!(!is_animated_gif(&dir.join("photo.png")));
    }

    #[test]
    fn a_video_turned_a_quarter_is_taller_than_wide() {
        let moov = read_moov(&mut Cursor::new(mp4(3.0, 1920, 1080, true))).unwrap();
        assert_eq!(parse_moov(&moov).dims, Some((1080, 1920)));
    }

    // was: a QuickTime file whose first box is not ftyp (what QuickTime
    // wrote before 2001, and what some cameras and editors still write) was
    // refused as not a video at all.
    #[test]
    fn a_quicktime_file_without_an_ftyp_box_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clip.mov");
        let audio = bx(b"trak", &tkhd(0, 0, false));
        let video = bx(b"trak", &tkhd(640, 480, false));
        let moov = [mvhd(600, 6000), audio, video].concat();
        let mut file = bx(b"wide", &[]);
        file.extend(bx(b"mdat", &vec![0u8; 5000]));
        file.extend(bx(b"moov", &moov));
        std::fs::write(&path, &file).unwrap();
        assert_eq!(sniff_mime(&file), Some("video/quicktime"));
        let p = prepare(&path).unwrap();
        assert_eq!((p.mime, p.dims), ("video/quicktime", Some((640, 480))));
    }

    // Where a phone says it was filmed, and what an editor wrote in XMP, are
    // blanked before the video goes anywhere; its length, shape and media
    // data stay where they were.
    #[test]
    fn a_videos_location_and_metadata_are_blanked_in_place() {
        let place = b"+35.6895+139.6917/";
        let mut xyz = vec![0u8, 18, 0, 0];
        xyz.extend_from_slice(place);
        let udta = bx(b"udta", &bx(b"\xA9xyz", &xyz));
        let trak_meta = bx(b"meta", b"com.apple.quicktime.location.ISO6709 +35.6895");
        let mut file = bx(b"ftyp", b"isom\0\0\x02\0isomiso2");
        file.extend(bx(b"uuid", b"<x:xmpmeta>secret</x:xmpmeta>"));
        file.extend(bx(b"mdat", &vec![7u8; 500]));
        let video = bx(b"trak", &[tkhd(640, 360, false), trak_meta].concat());
        let moov = [mvhd(1000, 3000), video, udta].concat();
        file.extend(bx(b"moov", &moov));
        let before = file.clone();
        strip_metadata(&mut file);
        assert_eq!(file.len(), before.len());
        let text = String::from_utf8_lossy(&file);
        for gone in ["+35.6895", "xmpmeta", "com.apple.quicktime", "udta", "uuid"] {
            assert!(!text.contains(gone), "{gone} is still there");
        }
        // The media data is untouched, and the header still reads.
        assert!(file.windows(500).any(|w| w.iter().all(|b| *b == 7)));
        let info = parse_moov(&read_moov(&mut Cursor::new(&file[..])).unwrap());
        assert_eq!(info.dims, Some((640, 360)));
        assert_eq!(info.seconds, Some(3.0));
    }

    #[test]
    fn a_box_claiming_a_64_bit_size_past_the_file_is_left_alone() {
        // `ftyp`, then a box whose 64-bit size runs past the end of any
        // memory: adding it to where the box starts overflowed.
        for large in [u64::MAX, u64::MAX - 7, 1 << 63] {
            let mut file = bx(b"ftyp", b"isom\0\0\x02\0isomiso2");
            file.extend([0, 0, 0, 1]);
            file.extend(b"udta");
            file.extend(large.to_be_bytes());
            file.extend(b"+35.6895+139.6917/");
            let before = file.clone();
            strip_metadata(&mut file);
            assert_eq!(file, before);
        }
    }

    #[test]
    fn a_file_without_a_header_gives_nothing() {
        assert_eq!(read_moov(&mut Cursor::new(b"not a video at all")), None);
        assert_eq!(parse_moov(b"\0\0\0\x04junk"), VideoInfo::default());
    }

    #[rstest]
    #[case(b"\0\0\0\x18ftypisom".as_slice(), Some("video/mp4"))]
    #[case(b"\0\0\0\x14ftypqt  ".as_slice(), Some("video/quicktime"))]
    #[case(b"\x1a\x45\xdf\xa3\x01".as_slice(), Some("video/webm"))]
    #[case(b"\0\0\x01\xba\x44".as_slice(), Some("video/mpeg"))]
    #[case(b"GIF89a".as_slice(), None)]
    // A QuickTime file older than the ftyp box starts with one of its other
    // top-level boxes.
    #[case(b"\0\0\x13\x88mdat\0\0".as_slice(), Some("video/quicktime"))]
    #[case(b"\0\0\0\x68moov\0\0".as_slice(), Some("video/quicktime"))]
    #[case(b"\0\0\0\x08wide".as_slice(), Some("video/quicktime"))]
    #[case(b"\0\0\0\x10free\0\0\0\0\0\0\0\0".as_slice(), Some("video/quicktime"))]
    #[case(b"\0\0\0\x0cskip\0\0\0\0".as_slice(), Some("video/quicktime"))]
    #[case(b"\0\0\0\x14pnot\0\0\0\0\0\0\0\0\0\0\0\0".as_slice(), Some("video/quicktime"))]
    // A box size smaller than the header itself, or a first box that is not
    // one a file starts with, is not a video.
    #[case(b"\0\0\0\x04mdat".as_slice(), None)]
    #[case(b"\0\0\0\x20trak\0\0".as_slice(), None)]
    #[case(b"\0\0\0\x20abcd\0\0".as_slice(), None)]
    fn video_types_are_sniffed(#[case] bytes: &[u8], #[case] want: Option<&str>) {
        assert_eq!(sniff_mime(bytes), want);
    }

    #[rstest]
    #[case(7.4, "0:07")]
    #[case(165.0, "2:45")]
    #[case(180.0, "3:00")]
    fn lengths_read_like_a_player(#[case] s: f64, #[case] want: &str) {
        assert_eq!(format_seconds(s), want);
    }

    #[test]
    fn a_video_is_read_and_checked_against_the_limits() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("clip.mp4");
        std::fs::write(&ok, mp4(12.0, 640, 360, false)).unwrap();
        let p = prepare(&ok).unwrap();
        assert_eq!((p.mime, p.dims), ("video/mp4", Some((640, 360))));

        let long = dir.path().join("long.mp4");
        std::fs::write(&long, mp4(181.0, 640, 360, false)).unwrap();
        let e = prepare(&long).unwrap_err();
        assert!(
            e.message()
                .contains("runs 3:01; videos can be at most 3:00"),
            "{e}"
        );

        // A length just over the limit is not shown as the limit itself.
        let just_over = dir.path().join("just-over.mp4");
        std::fs::write(&just_over, mp4(180.4, 640, 360, false)).unwrap();
        let e = prepare(&just_over).unwrap_err();
        assert!(
            e.message()
                .contains("runs 3:01; videos can be at most 3:00"),
            "{e}"
        );

        let text = dir.path().join("notes.mp4");
        std::fs::write(&text, "hello").unwrap();
        let e = prepare(&text).unwrap_err();
        assert!(e.message().contains("notes.mp4 is not a video"), "{e}");
    }

    #[test]
    fn a_still_gif_is_not_animated_and_a_two_frame_one_is() {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame, RgbaImage};
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, frames: usize| {
            let path = dir.path().join(name);
            let mut enc = GifEncoder::new(File::create(&path).unwrap());
            for i in 0..frames {
                let img = RgbaImage::from_pixel(4, 4, image::Rgba([i as u8 * 90, 0, 0, 255]));
                enc.encode_frame(Frame::from_parts(
                    img,
                    0,
                    0,
                    Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
            }
            drop(enc);
            path
        };
        assert!(!is_animated_gif(&write("still.gif", 1)));
        assert!(is_animated_gif(&write("moving.gif", 2)));
        assert!(!is_animated_gif(&dir.path().join("missing.gif")));
    }

    /// A GIF by hand: a logical screen of `w` x `h`, then `frames` frames of
    /// one pixel, each after a graphic control extension.
    fn gif(w: u16, h: u16, frames: usize) -> Vec<u8> {
        let mut b = b"GIF89a".to_vec();
        b.extend_from_slice(&w.to_le_bytes());
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0]);
        for _ in 0..frames {
            b.extend_from_slice(&[0x21, 0xf9, 4, 0, 10, 0, 0, 0]);
            b.extend_from_slice(&[0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
            b.extend_from_slice(&[2, 2, 0x4c, 0x01, 0]);
        }
        b.push(0x3b);
        b
    }

    /// A header may claim a canvas of up to 65535 x 65535. Deciding whether
    /// a GIF is animated must not build that canvas: the file browser asks
    /// on the UI thread, where a 17-byte file found by fuzzing froze the
    /// screen for two seconds and asked for gigabytes.
    #[test]
    fn a_gif_claiming_a_huge_canvas_is_judged_from_its_blocks_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            (
                "fuzzed.gif",
                b"GIF89a(\xc6\x00\xf6\x1e\x00\x00\x00\x07\x00@".to_vec(),
                false,
            ),
            ("huge-still.gif", gif(65535, 65535, 1), false),
            ("huge-moving.gif", gif(65535, 65535, 3), true),
            ("small-moving.gif", gif(4, 4, 2), true),
            ("not-a-gif.gif", b"PNG and something".to_vec(), false),
        ];
        for (name, bytes, animated) in cases {
            let path = dir.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            let start = std::time::Instant::now();
            assert_eq!(is_animated_gif(&path), animated, "{name}");
            let took = start.elapsed();
            assert!(
                took < std::time::Duration::from_millis(200),
                "{name} took {took:?}"
            );
        }
    }

    /// Real media files from the E2E fixtures, damaged at random (bits
    /// flipped, bytes inserted and removed, cut short, length fields set to
    /// 0, 1, or the largest values), through every reader bsky points at a
    /// file of unknown origin: none may panic or stall.
    /// `BSKY_FUZZ_INPUTS=100000` (in a release build) for a long run.
    #[test]
    fn damaged_media_files_neither_panic_nor_stall() {
        struct Rng(u64);
        impl Rng {
            fn next(&mut self) -> u64 {
                self.0 ^= self.0 >> 12;
                self.0 ^= self.0 << 25;
                self.0 ^= self.0 >> 27;
                self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
            }
            fn below(&mut self, n: usize) -> usize {
                (self.next() % n.max(1) as u64) as usize
            }
        }
        fn damage(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
            let mut b = seed.to_vec();
            for _ in 0..1 + rng.below(8) {
                match rng.below(6) {
                    0 if !b.is_empty() => {
                        let i = rng.below(b.len());
                        b[i] ^= 1 << rng.below(8);
                    }
                    1 if !b.is_empty() => {
                        let i = rng.below(b.len());
                        b[i] = rng.next() as u8;
                    }
                    2 => {
                        let i = rng.below(b.len() + 1);
                        b.insert(i, rng.next() as u8);
                    }
                    3 if !b.is_empty() => {
                        let i = rng.below(b.len());
                        b.remove(i);
                    }
                    4 => {
                        let n = rng.below(b.len() + 1);
                        b.truncate(n);
                    }
                    _ if b.len() >= 4 => {
                        let i = rng.below(b.len() - 3);
                        let v: u32 = [0, 1, 7, 8, 0x7fff_ffff, u32::MAX][rng.below(6)];
                        b[i..i + 4].copy_from_slice(&v.to_be_bytes());
                    }
                    _ => {}
                }
            }
            b
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata");
        let seeds: Vec<(&str, Vec<u8>)> = [
            ("ts", "hls/v/seg0.ts"),
            ("mp4", "clip.mp4"),
            ("gif", "moving.gif"),
            ("png", "photo.png"),
            ("m3u8", "hls/playlist.m3u8"),
        ]
        .into_iter()
        .map(|(kind, file)| (kind, std::fs::read(root.join(file)).unwrap()))
        .collect();
        let inputs: usize = std::env::var("BSKY_FUZZ_INPUTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000);
        let dir = tempfile::tempdir().unwrap();
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for i in 0..inputs {
            let (kind, seed) = &seeds[i % seeds.len()];
            let data = damage(&mut rng, seed);
            let path = dir.path().join(format!("f.{kind}"));
            std::fs::write(&path, &data).unwrap();
            // Timed from here: writing the file is not the readers' cost.
            let start = std::time::Instant::now();
            match *kind {
                "ts" => {
                    let mut d = crate::hls::Demuxer::new();
                    d.feed(&data[..data.len() / 2]);
                    d.feed(&data[data.len() / 2..]);
                    let _ = d.finish();
                }
                "m3u8" => {
                    let text = String::from_utf8_lossy(&data);
                    let _ = crate::hls::pick_variant(&text, "https://v.test/a/playlist.m3u8");
                    let _ = crate::hls::segments(&text, "https://v.test/a/v/video.m3u8");
                }
                _ => {
                    let _ = probe(&path);
                    let _ = sniff_mime(&data);
                    let _ = is_animated_gif(&path);
                    let _ = crate::media::inspect(&path);
                    strip_metadata(&mut data.clone());
                }
            }
            let took = start.elapsed();
            // Generous for a debug build on a busy CI runner; the stall this
            // guards against took 48 s there.
            assert!(
                took < std::time::Duration::from_secs(5),
                "damaged {kind} #{i} took {took:?}: {data:?}"
            );
        }
    }

    #[test]
    fn an_animated_gif_goes_as_it_is_for_the_video_service() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata");
        let p = prepare(&dir.join("moving.gif")).unwrap();
        assert_eq!((p.mime, p.dims), ("image/gif", Some((40, 30))));
        assert!(p.bytes.starts_with(b"GIF8"));
    }
}
