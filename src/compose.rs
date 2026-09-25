//! Sending a post with its pictures or video: what the client's composer
//! and `bsky post` both do, so a post is the same record whichever sent it.

use std::path::PathBuf;

use crate::api::types::{CreatedRecord, ReplyRef, StrongRef};
use crate::api::{Client, PostImage, PostMedia, PostVideo};
use crate::error::{Error, Kind, Result};
use crate::{media, video};

/// A picture or video on the user's disk to attach to a post.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub path: PathBuf,
    /// Text describing it for people who cannot see it.
    pub alt: String,
}

/// The most a video's alt text may hold (`app.bsky.embed.video`): more is
/// refused with the post, after the video was uploaded.
pub const MAX_VIDEO_ALT_GRAPHEMES: usize = 1000;
pub const MAX_VIDEO_ALT_BYTES: usize = 10000;

/// Why `alt` cannot describe a video, if it cannot.
pub fn video_alt_problem(alt: &str) -> Option<String> {
    let alt = alt.trim();
    let n = crate::api::grapheme_len(alt);
    if n > MAX_VIDEO_ALT_GRAPHEMES {
        return Some(crate::i18n::tf(
            "the video's alt text is {} characters long; it can have {}",
            &[&n.to_string(), &MAX_VIDEO_ALT_GRAPHEMES.to_string()],
        ));
    }
    // Emoji reach the byte limit well under the character one, and then
    // the count of characters would be under the limit it is compared to.
    (alt.len() > MAX_VIDEO_ALT_BYTES).then(|| {
        crate::i18n::tf(
            "the video's alt text is {} bytes; the limit is {}",
            &[&alt.len().to_string(), &MAX_VIDEO_ALT_BYTES.to_string()],
        )
    })
}

/// Send a post, uploading its pictures (up to four) or its one video first.
/// `writer` is the language the user reads bsky in, which settles the
/// post's language where its letters do not (see [`crate::langs::guess`]).
/// Everything is prepared before anything is uploaded, so a file that cannot
/// be read stops the post before anything reaches the server. Nothing is
/// sent twice: a failure is returned, not tried again.
pub fn send_post(
    client: &Client,
    text: &str,
    reply: Option<&ReplyRef>,
    quote: Option<&StrongRef>,
    media: &[Attachment],
    video_service: &str,
    writer: crate::i18n::Lang,
) -> Result<CreatedRecord> {
    let videos = media
        .iter()
        .filter(|a| media::inspect(&a.path).kind == media::Kind::Video)
        .count();
    let embed = match (media.len(), videos) {
        (0, _) => PostMedia::None,
        (1, 1) => {
            let a = &media[0];
            if let Some(why) = video_alt_problem(&a.alt) {
                return Err(Error::new(Kind::Usage, why));
            }
            let v = video::prepare(&a.path)?;
            // The name the video service is told, which it may keep: what
            // the file is, not what it was called on this computer.
            let name = format!("video.{}", video::extension(v.mime));
            let blob = client.upload_video(
                video_service,
                &v.bytes,
                v.mime,
                &name,
                &a.path
                    .file_name()
                    .map_or_else(|| name.clone(), |n| n.to_string_lossy().into_owned()),
                std::time::Duration::from_secs(1),
            )?;
            PostMedia::Video(PostVideo {
                blob,
                alt: a.alt.trim().to_string(),
                dims: v.dims,
            })
        }
        (_, 0) => {
            let prepared = media
                .iter()
                .map(|a| media::prepare(&a.path))
                .collect::<Result<Vec<_>>>()?;
            let mut uploaded = Vec::with_capacity(media.len());
            for (a, p) in media.iter().zip(prepared) {
                let blob = client.upload_blob(&p.bytes, p.mime)?;
                uploaded.push(PostImage {
                    blob,
                    alt: a.alt.trim().to_string(),
                    width: p.width,
                    height: p.height,
                });
            }
            PostMedia::Images(uploaded)
        }
        _ => {
            return Err(Error::new(
                Kind::Usage,
                crate::i18n::t("a post can have up to 4 pictures or one video, not both"),
            ));
        }
    };
    client.create_post(text, reply, quote, &embed, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Emoji reach the 10000 bytes long before the 1000 characters: the
    // message then says the bytes, not a count under the limit it names.
    #[test]
    fn a_video_alt_text_says_which_limit_it_is_over() {
        assert_eq!(video_alt_problem(&"a".repeat(1000)), None);
        assert_eq!(
            video_alt_problem(&"a".repeat(1001)).as_deref(),
            Some("the video's alt text is 1001 characters long; it can have 1000")
        );
        let family = "👨‍👩‍👧‍👦"; // 25 bytes
        assert_eq!(video_alt_problem(&family.repeat(400)), None);
        assert_eq!(
            video_alt_problem(&format!(" {} ", family.repeat(401))).as_deref(),
            Some("the video's alt text is 10025 bytes; the limit is 10000")
        );
    }
}
