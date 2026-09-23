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

/// Send a post, uploading its pictures (up to four) or its one video first.
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
) -> Result<CreatedRecord> {
    let videos = media
        .iter()
        .filter(|a| media::inspect(&a.path).kind == media::Kind::Video)
        .count();
    let embed = match (media.len(), videos) {
        (0, _) => PostMedia::None,
        (1, 1) => {
            let a = &media[0];
            let v = video::prepare(&a.path)?;
            let name = a
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "video".into());
            let blob = client.upload_video(
                video_service,
                &v.bytes,
                v.mime,
                &name,
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
                "a post can have up to 4 pictures or one video, not both",
            ));
        }
    };
    client.create_post(text, reply, quote, &embed)
}
