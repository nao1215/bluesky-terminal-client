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
