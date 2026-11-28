//! Saving the files attached to statuses: where, under what name, and how
//! much, as msky saves a note's. A file is never written over: each is
//! created new, and a name that is taken gets ` (1)`, ` (2)`... as bsky's
//! downloads do.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::api::types::Media;
use crate::error::{Error, Result};

/// Largest file a download saves, as bsky's and msky's.
pub(crate) const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

/// How long one download may take: a video of 200 MB on a slow link takes
/// minutes, where an API call gets 30 s.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Environment variable naming the folder downloads are saved in.
pub(crate) const DOWNLOAD_DIR_ENV: &str = "MSTDN_DOWNLOAD_DIR";

/// Where files are saved: `-o`, else `MSTDN_DOWNLOAD_DIR`, else `mstdn` in
/// the platform download folder (`~/Downloads/mstdn`), as msky keeps its own.
pub(crate) fn download_dir(output: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = output.filter(|d| !d.as_os_str().is_empty()) {
        return Ok(dir);
    }
    if let Some(dir) = std::env::var_os(DOWNLOAD_DIR_ENV).filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .map(|d| d.join("mstdn"))
        .ok_or_else(|| {
            Error::io("there is no download folder")
                .with_hint(format!("pass -o DIR or set {DOWNLOAD_DIR_ENV}"))
        })
}

/// The extensions each MIME type is saved with, the usual one first.
/// Mastodon says only what kind a file is (`image`, `video`...), so the
/// type the file is served as decides, else the extension of its address
/// when it is one of its kind's; a file of a type not listed is saved as
/// `.bin`: a picture served as `cat.exe` must not be saved as a program.
const EXTENSIONS: &[(&str, &[&str])] = &[
    ("image/jpeg", &["jpg", "jpeg"]),
    ("image/png", &["png"]),
    ("image/apng", &["png", "apng"]),
    ("image/gif", &["gif"]),
    ("image/webp", &["webp"]),
    ("image/avif", &["avif"]),
    ("image/bmp", &["bmp"]),
    ("image/heic", &["heic"]),
    ("image/jxl", &["jxl"]),
    ("video/mp4", &["mp4", "m4v"]),
    ("video/webm", &["webm"]),
    ("video/quicktime", &["mov"]),
    ("audio/mpeg", &["mp3"]),
    ("audio/ogg", &["ogg", "oga", "opus"]),
    ("audio/opus", &["opus"]),
    ("audio/wav", &["wav"]),
    ("audio/x-wav", &["wav"]),
    ("audio/flac", &["flac"]),
    ("audio/mp4", &["m4a"]),
    ("audio/aac", &["aac"]),
];

/// The MIME types whose files are of Mastodon's `kind`.
fn of_kind(kind: &str, mime: &str) -> bool {
    match kind {
        "image" => mime.starts_with("image/"),
        "video" | "gifv" => mime.starts_with("video/"),
        "audio" => mime.starts_with("audio/"),
        _ => false,
    }
}

/// The extension a file of `kind` at `url`, served as `served`, is saved
/// with.
fn extension(kind: &str, url: &str, served: Option<&str>) -> &'static str {
    if let Some(mime) = served {
        let mime = mime
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if let Some((_, exts)) = EXTENSIONS.iter().find(|(m, _)| *m == mime) {
            return exts.first().copied().unwrap_or("bin");
        }
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or("");
    let given = last
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    EXTENSIONS
        .iter()
        .filter(|(m, _)| of_kind(kind, m))
        .flat_map(|(_, exts)| exts.iter())
        .find(|e| **e == given)
        .copied()
        .unwrap_or("bin")
}

/// Longest status ID kept in a name, in bytes; Mastodon's are under 20.
const MAX_STEM_BYTES: usize = 100;

/// `<status id>-<n>`: the status the file came from and its place among
/// the status's files from 1. Only ASCII letters, digits, `-` and `_` of
/// the ID are kept, so the name works on every system and never leaves the
/// folder. The extension is added once the file's type is known.
pub(crate) fn file_stem(status_id: &str, n: usize) -> String {
    let mut stem: String = status_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    stem.truncate(MAX_STEM_BYTES);
    if stem.is_empty() {
        stem = "status".to_string();
    }
    format!("{stem}-{n}")
}

/// Whether `url` is an http(s) address with something after the scheme.
fn is_web_address(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["https://", "http://"]
        .iter()
        .any(|p| lower.starts_with(p) && lower.len() > p.len())
}

/// `n` bytes in MB, rounded up, for messages.
fn megabytes(n: u64) -> u64 {
    n.div_ceil(1024 * 1024)
}

/// Save `media` in `dir` as `stem.<ext>` (or `stem (1).<ext>`...), and say
/// where.
pub(crate) fn save(agent: &ureq::Agent, media: &Media, dir: &Path, stem: &str) -> Result<PathBuf> {
    let url = media
        .source()
        .ok_or_else(|| Error::api(format!("{stem}: the server gave no address for the file")))?;
    if !is_web_address(url) {
        return Err(Error::api(format!(
            "{} is not an http(s) address; nothing was written",
            crate::text::one_line(url)
        )));
    }
    let too_big = || {
        Error::api(format!(
            "{url} is larger than {} MB, the most mstdn saves; nothing was written",
            megabytes(MAX_DOWNLOAD_BYTES)
        ))
    };
    let mut resp = agent
        .get(url)
        .config()
        .timeout_global(Some(DOWNLOAD_TIMEOUT))
        // A file is often on a CDN the server's address sends to.
        .max_redirects(5)
        .build()
        .call()
        .map_err(|e| Error::api(format!("cannot download {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::api(format!(
            "cannot download {url}: HTTP {}",
            resp.status().as_u16()
        )));
    }
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let declared = header("content-length").and_then(|v| v.trim().parse::<u64>().ok());
    if declared.is_some_and(|n| n > MAX_DOWNLOAD_BYTES) {
        return Err(too_big());
    }
    let name = format!(
        "{stem}.{}",
        extension(&media.kind, url, header("content-type").as_deref())
    );
    std::fs::create_dir_all(dir)
        .map_err(|e| Error::io(format!("cannot create {}: {e}", dir.display())))?;
    // One byte past the limit is read, so a body that goes over it is told
    // from one that ends on it.
    let mut body = resp
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES + 1)
        .reader();
    save_new_with(dir, &name, |path, f| {
        copy_within(&mut body, f, MAX_DOWNLOAD_BYTES, url, path)
    })
}

/// Copy `from` into `to`, refused once it comes to more than `limit` bytes.
/// A failure to read is the server's; one to write is the disk's.
fn copy_within(
    from: &mut dyn Read,
    to: &mut dyn Write,
    limit: u64,
    url: &str,
    path: &Path,
) -> Result<()> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Error::api(format!("cannot download {url}: {e}"))),
        };
        total += n as u64;
        if total > limit {
            return Err(Error::api(format!(
                "{url} is larger than {} MB, the most mstdn saves; nothing was written",
                megabytes(limit)
            )));
        }
        to.write_all(&buf[..n])
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
    }
    Ok(())
}

/// The names to try for `name` in `dir`: `name`, then `name (1)`...
fn candidate_path(dir: &Path, name: &str, i: usize) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    dir.join(match (i, ext.is_empty()) {
        (0, _) => name.to_string(),
        (i, true) => format!("{stem} ({i})"),
        (i, false) => format!("{stem} ({i}).{ext}"),
    })
}

/// Make a new file in `dir` named `name`, or `name (1)`..., and fill it
/// with `write`. A name is taken when anything is there, a link to nothing
/// included: the file is created only if it does not exist (`create_new`),
/// so nothing is overwritten or written through a link, even if it appears
/// meanwhile. A file `write` could not finish is removed: cut short, it
/// looks like the download.
fn save_new_with(
    dir: &Path,
    name: &str,
    write: impl FnOnce(&Path, &mut std::fs::File) -> Result<()>,
) -> Result<PathBuf> {
    for i in 0..10_000 {
        let path = candidate_path(dir, name, i);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(Error::io(format!("cannot write {}: {e}", path.display()))),
        };
        let written = write(&path, &mut file).and_then(|()| {
            file.sync_all()
                .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))
        });
        if let Err(e) = written {
            drop(file);
            return Err(match std::fs::remove_file(&path) {
                Err(left) if left.kind() != std::io::ErrorKind::NotFound => {
                    let why = format!("{}; cannot remove {}: {left}", e.message(), path.display());
                    Error::new(e.kind(), why)
                }
                _ => e,
            });
        }
        return Ok(path);
    }
    Err(Error::io(format!(
        "{} has too many files named like {name}",
        dir.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::served_type_wins("image", "https://c.test/a.exe", Some("image/png"), "png")]
    #[case::served_with_parameters(
        "video",
        "https://c.test/a",
        Some("video/webm; codecs=vp9"),
        "webm"
    )]
    #[case::address_of_its_kind("image", "https://c.test/original/a.JPEG?x=1", None, "jpeg")]
    #[case::gifv_is_a_video(
        "gifv",
        "https://c.test/a.mp4",
        Some("application/octet-stream"),
        "mp4"
    )]
    #[case::address_not_of_its_kind("image", "https://c.test/a.mp4", None, "bin")]
    #[case::program(
        "unknown",
        "https://c.test/tool.exe",
        Some("application/x-msdownload"),
        "bin"
    )]
    #[case::no_extension("audio", "https://c.test/sound", None, "bin")]
    fn a_file_is_named_after_its_type(
        #[case] kind: &str,
        #[case] url: &str,
        #[case] served: Option<&str>,
        #[case] want: &str,
    ) {
        assert_eq!(extension(kind, url, served), want);
    }

    #[rstest]
    #[case("109372", "109372-2")]
    #[case("../../etc/passwd", "etcpasswd-2")]
    #[case("a/b\\c:d", "abcd-2")]
    #[case("", "status-2")]
    #[case("投稿", "status-2")]
    fn a_name_never_leaves_the_folder(#[case] id: &str, #[case] want: &str) {
        assert_eq!(file_stem(id, 2), want);
    }

    // A file already there is never written over: the new one takes the
    // next free name, and the old one keeps what it had.
    #[test]
    fn a_name_that_is_taken_gets_the_next_number() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("1-1.png");
        std::fs::write(&old, b"old").unwrap();
        let path = save_new_with(dir.path(), "1-1.png", |p, f| {
            f.write_all(b"new")
                .map_err(|e| Error::io(format!("{}: {e}", p.display())))
        })
        .unwrap();
        assert_eq!(path, dir.path().join("1-1 (1).png"));
        assert_eq!(std::fs::read(&old).unwrap(), b"old");
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    // What could not be finished is not left to look like the download.
    #[test]
    fn a_file_that_could_not_be_finished_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let e = save_new_with(dir.path(), "1-1.png", |_, f| {
            f.write_all(b"half").unwrap();
            Err(Error::api("cut"))
        })
        .unwrap_err();
        assert_eq!(e.message(), "cut");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_body_over_the_limit_is_refused() {
        let mut out = Vec::new();
        let p = Path::new("x");
        copy_within(&mut &b"12345"[..], &mut out, 5, "u", p).unwrap();
        assert_eq!(out, b"12345");
        let e = copy_within(&mut &b"123456"[..], &mut Vec::new(), 5, "u", p).unwrap_err();
        assert!(e.message().contains("larger than"), "{}", e.message());
    }

    #[rstest]
    #[case("https://cdn.test/a.png", true)]
    #[case("HTTP://cdn.test/a.png", true)]
    #[case("https://", false)]
    #[case("file:///etc/passwd", false)]
    #[case("ftp://cdn.test/a.png", false)]
    #[case("javascript:alert(1)", false)]
    #[case("/files/a.png", false)]
    fn only_web_addresses_are_fetched(#[case] url: &str, #[case] want: bool) {
        assert_eq!(is_web_address(url), want);
    }

    // A file that is not at a web address is refused before anything is
    // fetched or written.
    #[rstest]
    #[case(Some("file:///etc/passwd"))]
    #[case(None)]
    fn a_file_that_cannot_be_saved_writes_nothing(#[case] url: Option<&str>) {
        let dir = tempfile::tempdir().unwrap();
        let m = Media {
            url: url.map(str::to_string),
            kind: "image".into(),
            ..Default::default()
        };
        let e = save(&crate::api::agent(), &m, &dir.path().join("out"), "1-1").unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Api, "{}", e.message());
        assert!(!dir.path().join("out").exists());
    }

    #[test]
    fn the_folder_given_wins() {
        assert_eq!(
            download_dir(Some(PathBuf::from("写真👨‍👩‍👧"))).unwrap(),
            PathBuf::from("写真👨‍👩‍👧")
        );
    }
}
