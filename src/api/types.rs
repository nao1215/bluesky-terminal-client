//! The subset of the `app.bsky.*` and `com.atproto.*` lexicons bs reads.
//!
//! Every struct is lenient: unknown fields are ignored and optional fields
//! default, because the AppView adds fields over time and a client that fails
//! on a new field would break without any change on its side.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `app.bsky.actor.defs#viewerState`, reduced to the follow relationship.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActorViewer {
    /// AT-URI of the viewer's follow record for this actor, when following.
    pub following: Option<String>,
    /// AT-URI of this actor's follow record for the viewer, when followed back.
    pub followed_by: Option<String>,
}

/// `app.bsky.actor.defs#profileView` and its basic/detailed variants.
///
/// The three lexicon shapes differ only in which optional fields are present,
/// so one struct covers them all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub did: String,
    pub handle: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    pub followers_count: Option<u64>,
    pub follows_count: Option<u64>,
    pub posts_count: Option<u64>,
    pub viewer: Option<ActorViewer>,
}

impl Profile {
    /// Display name when set and non-blank, otherwise the handle.
    pub fn name(&self) -> &str {
        match self.display_name.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => name,
            _ => &self.handle,
        }
    }

    /// The viewer's follow record URI for this actor.
    pub fn following_uri(&self) -> Option<&str> {
        self.viewer.as_ref()?.following.as_deref()
    }
}

/// A `{uri, cid}` strong reference to a record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrongRef {
    pub uri: String,
    pub cid: String,
}

/// `app.bsky.feed.post#replyRef`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyRef {
    pub root: StrongRef,
    pub parent: StrongRef,
}

/// The fields of an `app.bsky.feed.post` record bs shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PostRecord {
    pub text: String,
    pub created_at: Option<String>,
    pub reply: Option<ReplyRef>,
}

/// `app.bsky.feed.defs#viewerState` for a post.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PostViewer {
    /// AT-URI of the viewer's like record, when liked.
    pub like: Option<String>,
    /// AT-URI of the viewer's repost record, when reposted.
    pub repost: Option<String>,
}

/// `app.bsky.embed.defs#aspectRatio`: width and height in any unit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AspectRatio {
    pub width: u32,
    pub height: u32,
}

/// One image of an `app.bsky.embed.images#view`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageView {
    pub thumb: String,
    pub fullsize: String,
    pub alt: String,
    pub aspect_ratio: Option<AspectRatio>,
}

/// An image an embed wants drawn inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbedImage<'a> {
    pub url: &'a str,
    /// Width and height when the AppView reported them; both are non-zero.
    pub aspect: Option<(u32, u32)>,
}

fn aspect(r: Option<AspectRatio>) -> Option<(u32, u32)> {
    r.filter(|r| r.width > 0 && r.height > 0)
        .map(|r| (r.width, r.height))
}

/// `app.bsky.embed.external#viewExternal`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExternalView {
    pub uri: String,
    pub title: String,
    pub description: String,
    pub thumb: Option<String>,
}

/// The embed kinds bs renders. Anything else is [`Embed::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "$type")]
pub enum Embed {
    #[serde(rename = "app.bsky.embed.images#view")]
    Images { images: Vec<ImageView> },
    #[serde(rename = "app.bsky.embed.external#view")]
    External { external: ExternalView },
    #[serde(rename = "app.bsky.embed.video#view")]
    Video {
        #[serde(default)]
        thumbnail: Option<String>,
        #[serde(default, rename = "aspectRatio")]
        aspect_ratio: Option<AspectRatio>,
    },
    #[serde(rename = "app.bsky.embed.record#view")]
    Record { record: Value },
    #[serde(rename = "app.bsky.embed.recordWithMedia#view")]
    RecordWithMedia { media: Box<Embed> },
    #[serde(other)]
    Other,
}

impl Embed {
    /// Thumbnails worth drawing inline, in display order.
    pub fn images(&self) -> Vec<EmbedImage<'_>> {
        match self {
            Embed::Images { images } => images
                .iter()
                .filter(|i| !i.thumb.is_empty())
                .map(|i| EmbedImage {
                    url: &i.thumb,
                    aspect: aspect(i.aspect_ratio),
                })
                .collect(),
            Embed::External { external } => external
                .thumb
                .as_deref()
                .map(|url| EmbedImage { url, aspect: None })
                .into_iter()
                .collect(),
            Embed::Video {
                thumbnail,
                aspect_ratio,
            } => thumbnail
                .as_deref()
                .map(|url| EmbedImage {
                    url,
                    aspect: aspect(*aspect_ratio),
                })
                .into_iter()
                .collect(),
            Embed::RecordWithMedia { media } => media.images(),
            Embed::Record { .. } | Embed::Other => Vec::new(),
        }
    }
}

/// `app.bsky.feed.defs#postView`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Post {
    pub uri: String,
    pub cid: String,
    pub author: Profile,
    /// The raw record; see [`Post::record`] for the typed view.
    #[serde(rename = "record")]
    pub raw_record: Value,
    #[serde(deserialize_with = "lenient_embed")]
    pub embed: Option<Embed>,
    pub reply_count: u64,
    pub repost_count: u64,
    pub like_count: u64,
    pub indexed_at: String,
    pub viewer: Option<PostViewer>,
}

fn lenient_embed<'de, D>(de: D) -> Result<Option<Embed>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // A malformed embed must not hide the post it belongs to.
    let value = Option::<Value>::deserialize(de)?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

impl Post {
    /// The record decoded into the fields bs shows.
    pub fn record(&self) -> PostRecord {
        serde_json::from_value(self.raw_record.clone()).unwrap_or_default()
    }

    /// Strong reference to this post.
    pub fn strong_ref(&self) -> StrongRef {
        StrongRef {
            uri: self.uri.clone(),
            cid: self.cid.clone(),
        }
    }

    /// The viewer's like record URI, when liked.
    pub fn like_uri(&self) -> Option<&str> {
        self.viewer.as_ref()?.like.as_deref()
    }

    /// The `reply` field for a new post answering this one: the thread root
    /// stays the root of this post's thread, and this post becomes the parent.
    pub fn reply_ref(&self) -> ReplyRef {
        let parent = self.strong_ref();
        let root = self
            .record()
            .reply
            .map(|r| r.root)
            .unwrap_or_else(|| parent.clone());
        ReplyRef { root, parent }
    }
}

/// `app.bsky.feed.defs#feedViewPost`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FeedItem {
    pub post: Post,
    /// Present when the item is in the feed because of a repost.
    pub reason: Option<Value>,
}

/// `app.bsky.feed.getTimeline` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Timeline {
    pub feed: Vec<FeedItem>,
    pub cursor: Option<String>,
}

/// `app.bsky.feed.getAuthorFeed` output (same shape as the timeline).
pub type AuthorFeed = Timeline;

/// `app.bsky.feed.searchPosts` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchPosts {
    pub posts: Vec<Post>,
    pub cursor: Option<String>,
}

/// `app.bsky.actor.searchActors` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchActors {
    pub actors: Vec<Profile>,
    pub cursor: Option<String>,
}

/// `com.atproto.server.createSession` / `refreshSession` output.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTokens {
    pub did: String,
    pub handle: String,
    pub access_jwt: String,
    pub refresh_jwt: String,
}

/// `com.atproto.repo.createRecord` output.
#[derive(Debug, Clone, Deserialize)]
pub struct CreatedRecord {
    pub uri: String,
}

/// `com.atproto.repo.getRecord` output.
#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    pub cid: Option<String>,
    pub value: Value,
}

/// `com.atproto.identity.resolveHandle` output.
#[derive(Debug, Clone, Deserialize)]
pub struct ResolvedHandle {
    pub did: String,
}

/// `com.atproto.repo.uploadBlob` output; the blob is kept verbatim so it can
/// be embedded in a record unchanged.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadedBlob {
    pub blob: Value,
}

/// The error body every XRPC endpoint returns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct XrpcError {
    pub error: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn post(value: Value) -> Post {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn profile_name_falls_back_to_handle_when_blank() {
        let mut p = Profile {
            handle: "alice.test".into(),
            display_name: Some("  ".into()),
            ..Profile::default()
        };
        assert_eq!(p.name(), "alice.test");
        p.display_name = Some("Alice".into());
        assert_eq!(p.name(), "Alice");
    }

    #[test]
    fn images_embed_exposes_thumbnails_in_order() {
        let p = post(json!({
            "uri": "at://a/app.bsky.feed.post/1", "cid": "c",
            "author": {"did": "did:plc:a", "handle": "a.test"},
            "record": {"text": "hi"},
            "embed": {"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://cdn/1", "fullsize": "https://cdn/1f", "alt": "",
                 "aspectRatio": {"width": 4, "height": 3}},
                {"thumb": "https://cdn/2", "fullsize": "https://cdn/2f", "alt": "two",
                 "aspectRatio": {"width": 0, "height": 3}}
            ]}
        }));
        let embed = p.embed.unwrap();
        let got: Vec<_> = embed.images().iter().map(|i| (i.url, i.aspect)).collect();
        // A zero side is no ratio at all, not a division waiting to happen.
        assert_eq!(
            got,
            [("https://cdn/1", Some((4, 3))), ("https://cdn/2", None)]
        );
    }

    #[test]
    fn record_with_media_uses_the_media_thumbnails() {
        let e: Embed = serde_json::from_value(json!({
            "$type": "app.bsky.embed.recordWithMedia#view",
            "record": {"record": {}},
            "media": {"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://cdn/m", "fullsize": "f", "alt": ""}
            ]}
        }))
        .unwrap();
        assert_eq!(e.images()[0].url, "https://cdn/m");
    }

    #[test]
    fn unknown_or_malformed_embed_keeps_the_post() {
        let p = post(json!({
            "uri": "u", "cid": "c", "author": {"did": "d", "handle": "h"},
            "record": {"text": "x"},
            "embed": {"$type": "app.bsky.embed.future#view"}
        }));
        assert_eq!(p.embed, Some(Embed::Other));
        let p = post(json!({
            "uri": "u", "cid": "c", "author": {"did": "d", "handle": "h"},
            "record": {"text": "x"},
            "embed": {"$type": "app.bsky.embed.images#view", "images": 3}
        }));
        assert_eq!(p.embed, None);
        assert_eq!(p.record().text, "x");
    }

    #[test]
    fn reply_ref_keeps_the_thread_root() {
        let top =
            post(json!({"uri": "at://top", "cid": "c1", "author": {}, "record": {"text": "t"}}));
        assert_eq!(top.reply_ref().root.uri, "at://top");
        assert_eq!(top.reply_ref().parent.uri, "at://top");

        let reply = post(json!({
            "uri": "at://mid", "cid": "c2", "author": {},
            "record": {"text": "r", "reply": {
                "root": {"uri": "at://top", "cid": "c1"},
                "parent": {"uri": "at://top", "cid": "c1"}
            }}
        }));
        let r = reply.reply_ref();
        assert_eq!(
            (r.root.uri.as_str(), r.parent.uri.as_str()),
            ("at://top", "at://mid")
        );
    }
}
