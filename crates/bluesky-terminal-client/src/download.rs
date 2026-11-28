//! Saving a post's pictures and videos: what the viewer's `d` and the
//! `bsky download` command share, so both name, limit and write a file the
//! same way. A picture is saved full size with the extension of what its
//! bytes are; a video is its best variant with the segments joined into one
//! MPEG transport stream. Nothing is overwritten: a taken name gets ` (1)`.

use std::path::PathBuf;

use crate::api::{self, types::Media};
use crate::error::{Error, Result};

/// Largest file a download saves.
const MAX_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;

fn fetch_bytes(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    let mut resp = agent.get(url).call().map_err(|e| {
        Error::api(crate::i18n::tf(
            "cannot download {}: {}",
            &[url, &e.to_string()],
        ))
    })?;
    if !resp.status().is_success() {
        return Err(Error::api(crate::i18n::tf(
            "cannot download {}: HTTP {}",
            &[url, &(resp.status().as_u16()).to_string()],
        )));
    }
    resp.body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD_BYTES)
        .read_to_vec()
        .map_err(|e| {
            Error::api(crate::i18n::tf(
                "cannot download {}: {}",
                &[url, &e.to_string()],
            ))
        })
}

/// The file name a Bluesky media URL suggests: `<cid>.<ext>` for
/// `.../plain/<did>/<cid>@jpeg`, `<cid>.ts` for a video's
/// `.../watch/<did>/<cid>/playlist.m3u8`. Only ASCII letters, digits, `-`,
/// `_`, and inner dots are kept, and a name Windows reserves for a device
/// (`CON`, `NUL`, `COM1`...) becomes the fallback, so the name works on
/// every system and never leaves the download folder.
fn download_name(media: &Media) -> String {
    let (last, fallback) = match media {
        Media::Image { url, .. } => (url_path(url).rsplit('/').next().unwrap_or(""), "picture"),
        Media::Video { playlist, .. } => {
            (url_path(playlist).rsplit('/').nth(1).unwrap_or(""), "video")
        }
    };
    let (stem, ext) = match media {
        Media::Video { .. } => (last, "ts"),
        Media::Image { .. } => match last.split_once('@') {
            Some(parts) => parts,
            None => last.rsplit_once('.').unwrap_or((last, "jpg")),
        },
    };
    let ext = match clean_name(ext).to_ascii_lowercase().as_str() {
        "" => "jpg".to_string(),
        "jpeg" => "jpg".to_string(),
        e => e.to_string(),
    };
    let mut stem = clean_name(stem);
    // Most file systems take 255 bytes a name; the room left leaves space
    // for " (9999)" and the extension. The name is ASCII, so any cut is on
    // a character boundary.
    stem.truncate(MAX_STEM_BYTES);
    let stem = stem.trim_end_matches('.');
    let stem = if stem.is_empty() || is_reserved_name(stem) {
        fallback
    } else {
        stem
    };
    format!("{stem}.{ext}")
}

/// Longest stem of a download's name, in bytes.
const MAX_STEM_BYTES: usize = 100;

/// A URL without its query and fragment.
fn url_path(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

/// `s` with only the characters a file name keeps everywhere, and without
/// dots at either end (Windows drops trailing ones; leading ones hide it).
fn clean_name(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    kept.trim_matches('.').to_string()
}

/// Whether Windows reserves `stem` for a device, whatever follows its first
/// dot: `nul.txt` opens the device as `nul` does.
fn is_reserved_name(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or(stem).to_ascii_uppercase();
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit())
}

/// The names to try for `name` in `dir`: `name`, then `name (1)`...
fn candidate_path(dir: &std::path::Path, name: &str, i: usize) -> PathBuf {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    dir.join(match (i, ext.is_empty()) {
        (0, _) => name.to_string(),
        (i, true) => format!("{stem} ({i})"),
        (i, false) => format!("{stem} ({i}).{ext}"),
    })
}

/// Save a picture (full size) or a video (its best variant, the segments
/// joined into one MPEG transport stream, which players play as it is).
pub(crate) fn save(media: &Media, dir: Option<&std::path::Path>) -> Result<PathBuf> {
    let dir = dir.ok_or_else(|| {
        Error::io(crate::i18n::t("there is no download folder")).with_hint(crate::i18n::t(
            "choose one on the settings screen (s on your Profile tab)",
        ))
    })?;
    let agent = api::agent();
    let bytes = match media {
        Media::Image { url, .. } => fetch_bytes(&agent, url)?,
        Media::Video { playlist, .. } => {
            let text = |url: &str| {
                fetch_bytes(&agent, url).map(|b| String::from_utf8_lossy(&b).into_owned())
            };
            let master = text(playlist)?;
            let (media_url, media_text) = match crate::hls::pick_best_variant(&master, playlist) {
                Some(url) => {
                    let t = text(&url)?;
                    (url, t)
                }
                None => (playlist.clone(), master),
            };
            let all = join_segments(
                crate::hls::segments(&media_text, &media_url),
                MAX_DOWNLOAD_BYTES,
                |seg| fetch_bytes(&agent, seg),
            )?;
            if all.is_empty() {
                return Err(Error::api(crate::i18n::t(
                    "the video's playlist lists nothing to download",
                )));
            }
            all
        }
    };
    let name = match media {
        Media::Image { .. } => picture_name(&download_name(media), &bytes)?,
        Media::Video { .. } => download_name(media),
    };
    std::fs::create_dir_all(dir).map_err(|e| {
        Error::io(crate::i18n::tf(
            "cannot create {}: {}",
            &[&(dir.display()).to_string(), &e.to_string()],
        ))
    })?;
    save_new(dir, &name, &bytes)
}

/// The segments of a video fetched with `fetch` and joined, refused once
/// they come to more than `limit` bytes: each is limited on its own, and a
/// playlist may list any number of them.
fn join_segments(
    segments: impl IntoIterator<Item = String>,
    limit: u64,
    mut fetch: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut all = Vec::new();
    for seg in segments {
        all.extend(fetch(&seg)?);
        if all.len() as u64 > limit {
            return Err(Error::api(crate::i18n::tf(
                "the video is larger than {} MB",
                &[&(limit / (1024 * 1024)).to_string()],
            )));
        }
    }
    Ok(all)
}

/// `name` with the extension of the picture `bytes` hold. The server's
/// address names the file, but only the bytes say what it is: a picture at
/// an address ending in `.exe` must not be saved as a program, and what is
/// not a picture at all is not saved.
fn picture_name(name: &str, bytes: &[u8]) -> Result<String> {
    use image::ImageFormat;
    let ext = match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => "png",
        Ok(ImageFormat::Jpeg) => "jpg",
        Ok(ImageFormat::Gif) => "gif",
        Ok(ImageFormat::WebP) => "webp",
        _ => {
            return Err(Error::api(crate::i18n::tf(
                "{} is not a picture bsky can save; nothing was written",
                &[name],
            )));
        }
    };
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    Ok(format!("{stem}.{ext}"))
}

/// Write `bytes` to a new file in `dir` named `name`, or `name (1)`... A
/// name is taken when anything is there, a link to nothing included: the
/// file is created only if it does not exist (`create_new`), so nothing is
/// overwritten or written through a link, even if it appears meanwhile.
fn save_new(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    use std::io::Write;
    save_new_with(dir, name, |f| f.write_all(bytes))
}

/// [`save_new`], with `write` filling the new file.
fn save_new_with(
    dir: &std::path::Path,
    name: &str,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
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
            Err(e) => {
                return Err(Error::io(crate::i18n::tf(
                    "cannot write {}: {}",
                    &[&(path.display()).to_string(), &e.to_string()],
                )));
            }
        };
        if let Err(e) = write(&mut file).and_then(|()| file.sync_all()) {
            // What was written of it is not the picture or the video: a
            // full disk left a cut file that looked like the download, and
            // took its name from the next try.
            drop(file);
            let shown = path.display().to_string();
            let mut why = crate::i18n::tf("cannot write {}: {}", &[&shown, &e.to_string()]);
            // Left behind, it looks like the download: the user is told.
            match std::fs::remove_file(&path) {
                Err(left) if left.kind() != std::io::ErrorKind::NotFound => {
                    why.push_str("; ");
                    why.push_str(&crate::i18n::tf(
                        "cannot remove {}: {}",
                        &[&shown, &left.to_string()],
                    ));
                }
                _ => {}
            }
            return Err(Error::io(why));
        }
        return Ok(path);
    }
    Err(Error::io(crate::i18n::tf(
        "{} has too many files named like {}",
        &[&(dir.display()).to_string(), name],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downloads_are_named_after_the_media() {
        let img = |url: &str| Media::Image {
            url: url.into(),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(
            download_name(&img(
                "https://cdn.bsky.app/img/feed_fullsize/plain/did:plc:x/bafkreiabc@jpeg"
            )),
            "bafkreiabc.jpg"
        );
        assert_eq!(
            download_name(&img("http://127.0.0.1:1/img/full1.png")),
            "full1.png"
        );
        let video = Media::Video {
            playlist: "https://video.bsky.app/watch/did%3Aplc%3Ax/bafkreivid/playlist.m3u8".into(),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&video), "bafkreivid.ts");
    }

    fn encoded(format: image::ImageFormat) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut out, format)
            .unwrap();
        out.into_inner()
    }

    // A picture is saved with the extension of what it is, not of what its
    // address says: a post's picture at an address ending in .exe was
    // saved as an .exe, with whatever bytes the server sent.
    #[rstest::rstest]
    #[case::exe_that_is_a_png("setup.exe", image::ImageFormat::Png, "setup.png")]
    #[case::jpeg_named_png("photo.png", image::ImageFormat::Jpeg, "photo.jpg")]
    #[case::gif("anim.jpg", image::ImageFormat::Gif, "anim.gif")]
    #[case::webp("bafkreiabc.jpg", image::ImageFormat::WebP, "bafkreiabc.webp")]
    #[case::no_dot("picture", image::ImageFormat::Png, "picture.png")]
    fn a_picture_is_saved_as_what_it_is(
        #[case] name: &str,
        #[case] format: image::ImageFormat,
        #[case] want: &str,
    ) {
        assert_eq!(picture_name(name, &encoded(format)).unwrap(), want);
    }

    #[rstest::rstest]
    #[case::program(b"MZ\x90\x00 a program".as_slice())]
    #[case::script(b"#!/bin/sh\nrm -rf ~\n".as_slice())]
    #[case::html(b"<html>not found</html>".as_slice())]
    #[case::empty(b"".as_slice())]
    fn what_is_not_a_picture_is_not_saved(#[case] bytes: &[u8]) {
        let err = picture_name("setup.exe", bytes).unwrap_err();
        assert!(err.message().contains("is not a picture"), "{err}");
    }

    #[rstest::rstest]
    #[case::reserved_on_windows("https://cdn.test/plain/did:plc:x/CON@jpeg", "picture.jpg")]
    #[case::reserved_with_a_dot("https://cdn.test/x/nul.", "picture.jpg")]
    #[case::no_extension("https://cdn.test/plain/did:plc:x/abc@", "abc.jpg")]
    #[case::only_dots("https://cdn.test/plain/did:plc:x/..@..", "picture.jpg")]
    #[case::fragment("https://cdn.test/plain/did:plc:x/abc@png#a/b", "abc.png")]
    #[case::emoji_and_cjk("https://cdn.test/x/写真👨‍👩‍👧.png", "picture.png")]
    fn a_download_name_is_safe_on_every_system(#[case] url: &str, #[case] want: &str) {
        let media = Media::Image {
            url: url.into(),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&media), want);
    }

    #[rstest::rstest]
    #[case::reserved("https://video.test/watch/did/COM1/playlist.m3u8", "video.ts")]
    #[case::dot_dot("https://video.test/watch/did/../playlist.m3u8", "video.ts")]
    #[case::fragment("https://video.test/watch/did/cid/playlist.m3u8#a/b", "cid.ts")]
    fn a_video_download_name_is_safe_on_every_system(#[case] url: &str, #[case] want: &str) {
        let media = Media::Video {
            playlist: url.into(),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert_eq!(download_name(&media), want);
    }

    // A name already taken by a link, even one to nothing, is not written
    // through: that would create or replace the file it points to.
    #[cfg(unix)]
    #[test]
    fn a_download_never_writes_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.txt");
        let downloads = dir.path().join("dl");
        std::fs::create_dir(&downloads).unwrap();
        std::os::unix::fs::symlink(&outside, downloads.join("a.jpg")).unwrap();
        let saved = save_new(&downloads, "a.jpg", b"picture").unwrap();
        assert_eq!(saved, downloads.join("a (1).jpg"));
        assert!(!outside.exists(), "wrote through the link");
        assert_eq!(std::fs::read(saved).unwrap(), b"picture");
    }

    // A name longer than a file system allows (255 bytes on most) failed
    // the download with "File name too long" after the whole file had come.
    #[test]
    fn a_long_address_still_names_a_file_that_can_be_written() {
        let stem = "a".repeat(300);
        let media = Media::Image {
            url: format!("https://cdn.test/plain/did:plc:x/{stem}@jpeg"),
            thumb: String::new(),
            alt: String::new(),
            aspect: None,
        };
        let name = download_name(&media);
        assert!(name.len() <= 128, "{} bytes", name.len());
        assert!(name.ends_with(".jpg") && name.starts_with("aaaa"), "{name}");
        let video = Media::Video {
            playlist: format!("https://video.test/watch/did/{stem}/playlist.m3u8"),
            thumbnail: None,
            alt: String::new(),
            aspect: None,
        };
        assert!(download_name(&video).len() <= 128);
        let dir = tempfile::tempdir().unwrap();
        save_new(dir.path(), &name, b"x").unwrap();
        let again = save_new(dir.path(), &name, b"x").unwrap();
        assert!(again.to_string_lossy().ends_with(" (1).jpg"));
    }

    // The size limit held for each segment but not for the video: a playlist
    // of many segments was kept in memory whole, however large.
    #[test]
    fn a_video_download_stops_once_its_segments_pass_the_limit() {
        let segments = ["a", "b", "c", "d"].map(String::from);
        let mut fetched = Vec::new();
        let got = join_segments(segments.clone(), 15, |url| {
            fetched.push(url.to_string());
            Ok(vec![0; 10])
        });
        let e = got.unwrap_err();
        assert!(e.message().contains("MB"), "{}", e.message());
        assert_eq!(fetched, ["a", "b"], "fetched after the limit was passed");

        let all = join_segments(segments.clone(), 40, |_| Ok(vec![1; 10])).unwrap();
        assert_eq!(all.len(), 40);

        let e = join_segments(segments, 40, |url| {
            if url == "c" {
                Err(Error::api("gone"))
            } else {
                Ok(vec![1; 10])
            }
        })
        .unwrap_err();
        assert_eq!(e.message(), "gone");
    }

    // A write that fails part way (a full disk) left a cut file under the
    // download's name, which looked saved and took the name from a retry.
    #[test]
    fn a_download_that_cannot_be_written_whole_leaves_no_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let err = save_new_with(dir.path(), "a.jpg", |f| {
            f.write_all(b"half of it")?;
            Err(std::io::Error::other("no space left on device"))
        })
        .unwrap_err();
        assert!(err.message().contains("no space left"), "{err}");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(
            save_new(dir.path(), "a.jpg", b"whole").unwrap(),
            dir.path().join("a.jpg")
        );
    }

    // The cut file could also fail to go (a folder the user cannot remove
    // from); that was dropped silently, and the file looked like the
    // download. The error says it is there.
    #[cfg(unix)]
    #[test]
    fn a_cut_download_that_cannot_be_removed_is_named_in_the_error() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let set_mode = |mode| {
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
        };
        let err = save_new_with(dir.path(), "a.jpg", |f| {
            f.write_all(b"half of it")?;
            set_mode(0o500);
            Err(std::io::Error::other("no space left on device"))
        })
        .unwrap_err();
        set_mode(0o700);
        let cut = dir.path().join("a.jpg");
        assert!(cut.exists(), "the folder let it be removed; run as a user");
        assert!(err.message().contains("no space left"), "{err}");
        assert!(
            err.message()
                .contains(&format!("cannot remove {}", cut.display())),
            "{err}"
        );
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), "old").unwrap();
        std::fs::write(dir.path().join("a (1).jpg"), "old").unwrap();
        let saved = save_new(dir.path(), "a.jpg", b"new").unwrap();
        assert_eq!(saved, dir.path().join("a (2).jpg"));
        assert_eq!(std::fs::read(dir.path().join("a.jpg")).unwrap(), b"old");
        let first = save_new(dir.path(), "b", b"x").unwrap();
        assert_eq!(first, dir.path().join("b"));
        assert_eq!(
            save_new(dir.path(), "b", b"x").unwrap(),
            dir.path().join("b (1)")
        );
    }
}
