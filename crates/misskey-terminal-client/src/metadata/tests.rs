//! The fixtures in `src/testdata/metadata` are small pictures and videos
//! of ffmpeg's test pattern, with metadata written by exiftool: a GPS
//! position (Tokyo Station's square), the camera `SecretCam`, the creator
//! `Taro Secret`, a comment. None of it is anyone's: the strings are what
//! the tests look for once the metadata is taken off.

use super::*;
use rstest::rstest;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/testdata/metadata")
            .join(name),
    )
    .unwrap()
}

fn has(data: &[u8], s: &str) -> bool {
    data.windows(s.len()).any(|w| w == s.as_bytes())
}

const PRIVATE: [&str; 5] = [
    "SecretCam",
    "Taro Secret",
    "private comment",
    "+35.68",
    "Exif\0\0",
];

// Each fixture loses every private string, keeps its kind, and still
// reads as the same picture.
#[rstest]
#[case("gps.jpg", Format::Jpeg)]
#[case("gps.png", Format::Png)]
#[case("gps.webp", Format::WebP)]
#[case("gps.gif", Format::Gif)]
#[case("gps.mp4", Format::Mp4)]
#[case("gps.webm", Format::WebM)]
fn the_metadata_goes_and_the_picture_stays(#[case] name: &str, #[case] kind: Format) {
    let data = fixture(name);
    assert_eq!(format(&data), Some(kind));
    assert!(
        PRIVATE.iter().any(|p| has(&data, p)),
        "the fixture {name} carries something to take off"
    );
    let out = strip(name, &data).unwrap();
    for p in PRIVATE {
        assert!(!has(&out, p), "{name} still has {p:?}");
    }
    assert_eq!(format(&out), Some(kind));
    match kind {
        Format::Mp4 | Format::WebM => {
            // Every offset stays where it was.
            assert_eq!(out.len(), data.len());
            assert_eq!(
                strip(name, &out).unwrap(),
                out,
                "stripping twice changes nothing"
            );
        }
        _ => {
            let before = image::load_from_memory(&data).unwrap().to_rgba8();
            let after = image::load_from_memory(&out).unwrap().to_rgba8();
            assert_eq!(before.dimensions(), after.dimensions());
            assert_eq!(
                before.as_raw(),
                after.as_raw(),
                "{name}: the pixels are the same"
            );
            assert_eq!(
                strip(name, &out).unwrap(),
                out,
                "stripping twice changes nothing"
            );
        }
    }
}

// A photo the camera stored sideways is turned upright before its EXIF,
// where that was said, goes.
#[test]
fn a_sideways_photo_is_turned_upright() {
    let data = fixture("turned.jpg");
    let before = image::load_from_memory(&data).unwrap();
    assert_eq!((before.width(), before.height()), (24, 16));
    let out = strip("turned.jpg", &data).unwrap();
    assert!(!has(&out, "SecretCam") && !has(&out, "Exif\0\0"));
    let after = image::load_from_memory(&out).unwrap();
    assert_eq!(
        (after.width(), after.height()),
        (16, 24),
        "turned a quarter"
    );
}

// What the tests above look for by string, exiftool reads as the
// fields themselves: none of GPS, the camera, the creator or a comment is
// left. Skipped where exiftool is not installed.
#[test]
fn exiftool_finds_nothing_left() {
    if std::process::Command::new("exiftool")
        .arg("-ver")
        .output()
        .is_err()
    {
        eprintln!("exiftool is not installed; skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "gps.jpg",
        "turned.jpg",
        "gps.png",
        "gps.webp",
        "gps.gif",
        "gps.mp4",
        "gps.webm",
    ] {
        let data = fixture(name);
        let out = strip(name, &data).unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, &out[..]).unwrap();
        let o = std::process::Command::new("exiftool")
            .args([
                "-s",
                "-s",
                "-s",
                "-GPS:all",
                "-GPSCoordinates",
                "-Location",
                "-Make",
                "-Creator",
                "-Artist",
                "-Title",
                "-City",
                "-Comment",
            ])
            .arg(&path)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&o.stdout);
        let left: Vec<&str> = said
            .lines()
            .filter(|l| !l.starts_with("Lavc") && !l.trim().is_empty())
            .collect();
        assert!(left.is_empty(), "{name}: exiftool still reads {left:?}");
    }
}

// A file of another kind goes as it is; one that says it is a picture but
// cannot be read as one is refused, since what it carries is not known.
#[test]
fn another_kind_goes_as_it_is_and_a_broken_picture_is_refused() {
    let pdf = b"%PDF-1.4 whatever".to_vec();
    assert!(matches!(strip("a.pdf", &pdf).unwrap(), Cow::Borrowed(_)));
    let mut jpeg = fixture("gps.jpg");
    jpeg.truncate(30);
    let e = strip("cut.jpg", &jpeg).unwrap_err();
    assert!(e.message().contains("cannot take the metadata"), "{e:?}");
    let mut png = fixture("gps.png");
    png.truncate(40);
    assert!(strip("cut.png", &png).is_err());
}

// Cut or changed anywhere, a file is never read out of bounds nor loops:
// it is stripped or refused.
#[test]
fn mangled_files_are_stripped_or_refused() {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rand = move |n: usize| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n as u64) as usize
    };
    for name in [
        "gps.jpg", "gps.png", "gps.webp", "gps.gif", "gps.mp4", "gps.webm",
    ] {
        let data = fixture(name);
        for _ in 0..300 {
            let mut d = data.clone();
            match rand(3) {
                0 => d.truncate(rand(d.len()).max(13)),
                1 => {
                    for _ in 0..4 {
                        let i = rand(d.len());
                        d[i] = rand(256) as u8;
                    }
                }
                _ => {
                    let i = rand(d.len());
                    d.splice(i..i, std::iter::repeat_n(0xFF, rand(8)));
                }
            }
            let _ = strip(name, &d);
        }
    }
}

// A video stripped still plays: ffprobe decodes as many frames of it as of
// the original. Skipped where ffprobe is not installed.
#[test]
fn a_stripped_video_still_plays() {
    let frames = |path: &std::path::Path| -> Option<String> {
        let o = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-count_frames",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=nb_read_frames",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .ok()?;
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let dir = tempfile::tempdir().unwrap();
    for name in ["gps.mp4", "gps.webm"] {
        let data = fixture(name);
        let before = dir.path().join(format!("before-{name}"));
        std::fs::write(&before, &data).unwrap();
        let Some(want) = frames(&before) else {
            eprintln!("ffprobe is not installed; skipped");
            return;
        };
        let out = strip(name, &data).unwrap();
        let after = dir.path().join(name);
        std::fs::write(&after, &out[..]).unwrap();
        assert_eq!(frames(&after).as_deref(), Some(want.as_str()), "{name}");
    }
}

// --- Crafted files: the arithmetic of each container's walk ----------------

/// A JPEG segment: its marker, its length (which counts itself) and `body`.
fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, marker];
    v.extend_from_slice(&u16::try_from(body.len() + 2).unwrap().to_be_bytes());
    v.extend_from_slice(body);
    v
}

fn stripped(name: &str, data: &[u8]) -> Option<Vec<u8>> {
    strip(name, data).ok().map(Cow::into_owned)
}

// Fill bytes before a marker are passed over, and the marker after them
// read as such.
#[test]
fn fill_bytes_before_a_jpeg_marker_are_passed_over() {
    let mut data = vec![0xFF, 0xD8, 0xFF];
    data.extend(segment(0xFE, b"ab"));
    data.extend([0xFF, 0xD9]);
    assert_eq!(stripped("a.jpg", &data), Some(vec![0xFF, 0xD8, 0xFF, 0xD9]));
}

// The markers without a length (a restart, TEM) are kept as two bytes, and
// the end of the picture ends the file: what trails it goes.
#[test]
fn jpeg_markers_without_a_length_are_kept_and_the_end_ends_it() {
    let mut data = vec![0xFF, 0xD8];
    data.extend(segment(0xE0, b"JF"));
    data.extend([0xFF, 0xD0, 0xFF, 0x01, 0xFF, 0xD9]);
    let want = data.clone();
    data.extend_from_slice(b"junk");
    assert_eq!(stripped("a.jpg", &data), Some(want));
}

// A segment of no body is a segment; one running past the end is refused.
#[test]
fn an_empty_jpeg_segment_is_kept_and_a_cut_one_refused() {
    let mut data = vec![0xFF, 0xD8];
    data.extend(segment(0xE0, b""));
    data.extend([0xFF, 0xD9]);
    assert_eq!(stripped("a.jpg", &data), Some(data.clone()));
    let mut cut = vec![0xFF, 0xD8];
    cut.extend(segment(0xE0, b"abcd"));
    cut.truncate(cut.len() - 2);
    assert_eq!(stripped("cut.jpg", &cut), None);
}

/// A WebP of `chunks`, each padded to an even length, with its RIFF size.
fn riff(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let mut v = b"RIFF\0\0\0\0WEBP".to_vec();
    for (kind, body) in chunks {
        v.extend_from_slice(*kind);
        v.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
    }
    let size = u32::try_from(v.len() - 8).unwrap();
    v[4..8].copy_from_slice(&size.to_le_bytes());
    v
}

// A chunk of odd length keeps its padding byte, the last one too, and the
// RIFF size is the size of what is left.
#[test]
fn webp_chunks_of_odd_length_keep_their_padding() {
    let data = riff(&[
        (b"VP8L", b"abc"),
        (b"EXIF", b"SecretCam"),
        (b"ALPH", b"defgh"),
    ]);
    let want = riff(&[(b"VP8L", b"abc"), (b"ALPH", b"defgh")]);
    assert_eq!(stripped("a.webp", &data), Some(want));
}

// The last chunk may lack its padding byte: the file is read to its end.
#[test]
fn the_last_webp_chunk_may_lack_its_padding_byte() {
    let mut data = riff(&[(b"VP8L", b"abcd"), (b"ALPH", b"efg")]);
    data.pop();
    let size = u32::try_from(data.len() - 8).unwrap();
    data[4..8].copy_from_slice(&size.to_le_bytes());
    assert_eq!(stripped("a.webp", &data), Some(data.clone()));
}

#[test]
fn a_gif_shorter_than_its_header_is_refused() {
    assert_eq!(stripped("a.gif", b"GIF89a\x01\x00"), None);
}

// A GIF without a global colour table, a picture with a local one of four
// colours, the loop extension kept and the comment gone.
#[test]
fn a_gif_keeps_its_loop_and_pictures_and_loses_its_comment() {
    let header = b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec();
    let looping = [&[0x21, 0xFF, 0x0B][..], b"NETSCAPE2.0", &[3, 1, 0, 0, 0]].concat();
    let comment = [&[0x21, 0xFE, 0x07][..], b"private", &[0x00]].concat();
    let mut picture = vec![0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0x81];
    picture.extend_from_slice(&[0x10; 12]);
    picture.extend_from_slice(&[0x02, 0x02, 0x44, 0x01, 0x00]);
    let data = [&header[..], &looping, &comment, &picture, &[0x3B]].concat();
    let want = [&header[..], &looping, &picture, &[0x3B]].concat();
    assert_eq!(stripped("a.gif", &data), Some(want));
}

/// An MP4 box of `kind` around `body`.
fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = u32::try_from(body.len() + 8)
        .unwrap()
        .to_be_bytes()
        .to_vec();
    v.extend_from_slice(kind);
    v.extend_from_slice(body);
    v
}

/// A `free` box with `size` as it was written and `body_len` zeros.
fn blanked(size: &[u8], body_len: usize) -> Vec<u8> {
    [size, b"free", &vec![0; body_len]].concat()
}

// A box is as long as all four bytes of its size say.
#[test]
fn an_mp4_box_over_64_kib_is_blanked_whole() {
    let mut data = mp4_box(b"udta", &vec![b'X'; 0x1_0008]);
    mp4(&mut data);
    assert_eq!(data, blanked(&[0, 1, 0, 0x10], 0x1_0008));
}

// A box of size 0 runs to the end of the file.
#[test]
fn an_mp4_box_to_the_end_of_the_file_is_blanked() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0");
    let mut data = [&ftyp[..], &[0, 0, 0, 0], b"udta", b"Taro Secret here"].concat();
    mp4(&mut data);
    assert_eq!(data, [&ftyp[..], &blanked(&[0, 0, 0, 0], 16)].concat());
}

// A box of size 1 has its size in the eight bytes after its kind.
#[test]
fn an_mp4_box_with_a_64_bit_size_is_blanked() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0");
    let size = 24u64.to_be_bytes();
    let mut data = [&ftyp[..], &[0, 0, 0, 1], b"udta", &size, b"SecretCa"].concat();
    mp4(&mut data);
    let free = [&[0, 0, 0, 1][..], b"free", &size, &[0; 8]].concat();
    assert_eq!(data, [ftyp, free].concat());
}

// An empty box (only its header) is a box: the walk goes on past it.
#[test]
fn an_empty_mp4_box_does_not_stop_the_walk() {
    let empty = mp4_box(b"free", b"");
    let mut data = [&empty[..], &mp4_box(b"udta", b"SecretCam")].concat();
    mp4(&mut data);
    assert_eq!(data, [empty, blanked(&[0, 0, 0, 17], 9)].concat());
}

// A handler's name is blanked, the 24 bytes before it kept; another box as
// long is left as it is.
#[test]
fn an_mp4_handler_loses_only_its_name() {
    let media = mp4_box(b"mdat", &[0x55; 32]);
    let handler = [&[0x11; 24][..], b"SecretCam\0"].concat();
    let mut data = [&media[..], &mp4_box(b"hdlr", &handler)].concat();
    mp4(&mut data);
    let name_gone = [&[0x11; 24][..], &[0; 10]].concat();
    assert_eq!(data, [media, mp4_box(b"hdlr", &name_gone)].concat());
}

// A box that cannot be read hides what comes after it, a place among it:
// the file is refused, not sent with that unread.
#[rstest]
#[case::smaller_than_its_header(&[0, 0, 0, 4, b'b', b'a', b'd', b'!'])]
#[case::longer_than_the_file(&[0, 0, 0x10, 0, b'm', b'd', b'a', b't'])]
#[case::a_64_bit_size_cut_short(&[0, 0, 0, 1, b'm', b'd', b'a', b't', 0, 0])]
fn an_mp4_with_a_box_it_cannot_read_is_refused(#[case] bad: &[u8]) {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0");
    let data = [&ftyp[..], bad, &mp4_box(b"udta", b"\xA9xyz+35.6+139.7/")].concat();
    assert!(strip("cut.mp4", &data).is_err());
}

// The same holds inside a track: a broken box in `moov` is not stepped over.
#[test]
fn an_mp4_with_a_box_it_cannot_read_inside_moov_is_refused() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0");
    let inner = [&[0, 0, 0, 2][..], b"bad!", &mp4_box(b"udta", b"SecretCam")].concat();
    let data = [ftyp, mp4_box(b"moov", &inner)].concat();
    assert!(strip("cut.mp4", &data).is_err());
}

// Bytes too few for a box at the end are not one, and do not refuse a file.
#[test]
fn an_mp4_with_a_few_bytes_after_its_last_box_is_still_stripped() {
    let ftyp = mp4_box(b"ftyp", b"isom\0\0\0\0");
    let data = [&ftyp[..], &mp4_box(b"udta", b"SecretCam"), &[0; 3]].concat();
    let out = strip("tail.mp4", &data).unwrap();
    assert_eq!(
        &out[..],
        [&ftyp[..], &blanked(&[0, 0, 0, 17], 9), &[0; 3]].concat()
    );
}

/// An EBML element: `id` (its length marker in it) and `body`, its size in
/// one byte or eight.
fn ebml(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    if body.len() < 127 {
        v.push(0x80 | u8::try_from(body.len()).unwrap());
    } else {
        v.push(0x01);
        v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    }
    v.extend_from_slice(body);
    v
}

const EBML_HEADER: &[u8] = &[0x1A, 0x45, 0xDF, 0xA3, 0x80];
const SEGMENT: &[u8] = &[0x18, 0x53, 0x80, 0x67];
const INFO: &[u8] = &[0x15, 0x49, 0xA9, 0x66];
const TITLE: &[u8] = &[0x7B, 0xA9];
const TIMESTAMP_SCALE: &[u8] = &[0x2A, 0xD7, 0xB1];

// A segment of unknown size (all ones, in eight bytes as a live recording
// writes it) runs to the end of the file, and its title goes.
#[test]
fn a_webm_segment_of_unknown_size_is_walked_to_the_end() {
    let scale = ebml(TIMESTAMP_SCALE, &[0x0F, 0x42, 0x40]);
    let unknown = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let file = |title: &[u8]| {
        let info = ebml(INFO, &[title, &scale].concat());
        [EBML_HEADER, SEGMENT, &unknown, &info].concat()
    };
    let void = [&[0xEC, 0x87][..], &[0; 7]].concat();
    assert_eq!(
        stripped("a.webm", &file(&ebml(TITLE, b"Secret"))),
        Some(file(&void))
    );
}

// An element of 128 bytes in all is the longest a Void with a one-byte
// size stands for.
#[test]
fn a_webm_title_of_128_bytes_becomes_a_void_with_a_one_byte_size() {
    let title = ebml(TITLE, &[b'x'; 125]);
    assert_eq!(title.len(), 128);
    let file = |title: &[u8]| [EBML_HEADER, &ebml(SEGMENT, &ebml(INFO, title))].concat();
    let void = [&[0xEC, 0xFE][..], &[0; 126]].concat();
    assert_eq!(stripped("a.webm", &file(&title)), Some(file(&void)));
}
