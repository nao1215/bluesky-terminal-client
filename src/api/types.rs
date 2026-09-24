//! The subset of the `app.bsky.*` and `com.atproto.*` lexicons bsky reads.
//!
//! Every struct is lenient: unknown fields are ignored and optional fields
//! default, because the AppView adds fields over time and a client that fails
//! on a new field would break without any change on its side.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

// Readers for fields the AppView passes on as some client wrote them: a
// value of the wrong type reads as if it were missing, so one odd field
// neither loses its post nor fails the page it is on.

fn any_string<'de, D: Deserializer<'de>>(de: D) -> Result<String, D::Error> {
    Ok(Value::deserialize(de)?
        .as_str()
        .unwrap_or_default()
        .to_string())
}

fn any_opt_string<'de, D: Deserializer<'de>>(de: D) -> Result<Option<String>, D::Error> {
    Ok(Value::deserialize(de)?.as_str().map(str::to_string))
}

/// A count or a size: a whole number, a fraction (rounded), or digits in a
/// string; anything else, a negative number included, is `None`.
fn number(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f.round() as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn any_count<'de, D: Deserializer<'de>>(de: D) -> Result<u64, D::Error> {
    Ok(number(&Value::deserialize(de)?).unwrap_or(0))
}

fn any_opt_count<'de, D: Deserializer<'de>>(de: D) -> Result<Option<u64>, D::Error> {
    Ok(number(&Value::deserialize(de)?))
}

fn any_size<'de, D: Deserializer<'de>>(de: D) -> Result<u32, D::Error> {
    Ok(number(&Value::deserialize(de)?)
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0))
}

fn any_bool<'de, D: Deserializer<'de>>(de: D) -> Result<bool, D::Error> {
    Ok(Value::deserialize(de)?.as_bool().unwrap_or(false))
}

/// A nested object, or `None` when it does not read.
fn any_opt<'de, D: Deserializer<'de>, T: DeserializeOwned>(de: D) -> Result<Option<T>, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(de)?).ok())
}

/// A nested object, or its default when it does not read.
fn any_or_default<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(
    de: D,
) -> Result<T, D::Error> {
    Ok(serde_json::from_value(Value::deserialize(de)?).unwrap_or_default())
}

/// A list of which only the items that read are kept.
fn readable_items<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    de: D,
) -> Result<Vec<T>, D::Error> {
    Ok(match Value::deserialize(de)? {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

/// `app.bsky.actor.defs#viewerState`: follows, mutes and blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ActorViewer {
    /// AT-URI of the viewer's follow record for this actor, when following.
    #[serde(deserialize_with = "any_opt_string")]
    pub following: Option<String>,
    /// AT-URI of this actor's follow record for the viewer, when followed back.
    #[serde(deserialize_with = "any_opt_string")]
    pub followed_by: Option<String>,
    /// Whether the viewer has muted this actor, directly or by a mute list.
    #[serde(deserialize_with = "any_bool")]
    pub muted: bool,
    /// The mute list the actor is muted by, when it is.
    #[serde(deserialize_with = "any_opt")]
    pub muted_by_list: Option<ListBasic>,
    /// AT-URI of the viewer's block record for this actor, when blocking.
    #[serde(deserialize_with = "any_opt_string")]
    pub blocking: Option<String>,
}

/// `app.bsky.graph.defs#listViewBasic`, as far as it is shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListBasic {
    #[serde(deserialize_with = "any_string")]
    pub uri: String,
    #[serde(deserialize_with = "any_string")]
    pub name: String,
}

/// `app.bsky.actor.defs#profileView` and its basic/detailed variants.
///
/// The three lexicon shapes differ only in which optional fields are present,
/// so one struct covers them all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    #[serde(deserialize_with = "any_string")]
    pub did: String,
    #[serde(deserialize_with = "any_string")]
    pub handle: String,
    #[serde(deserialize_with = "any_opt_string")]
    pub display_name: Option<String>,
    #[serde(deserialize_with = "any_opt_string")]
    pub description: Option<String>,
    #[serde(deserialize_with = "any_opt_string")]
    pub avatar: Option<String>,
    #[serde(deserialize_with = "any_opt_string")]
    pub banner: Option<String>,
    #[serde(deserialize_with = "any_opt_count")]
    pub followers_count: Option<u64>,
    #[serde(deserialize_with = "any_opt_count")]
    pub follows_count: Option<u64>,
    #[serde(deserialize_with = "any_opt_count")]
    pub posts_count: Option<u64>,
    #[serde(deserialize_with = "any_opt")]
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

    /// Whether the viewer has muted this actor themselves: what `M` and
    /// `bsky unmute` undo. The server says `muted` of a mute by a list too,
    /// which only taking the actor off the list undoes.
    pub fn muted(&self) -> bool {
        self.viewer
            .as_ref()
            .is_some_and(|v| v.muted && v.muted_by_list.is_none())
    }

    /// The name of the mute list this actor is muted by, if any.
    pub fn muting_list(&self) -> Option<&str> {
        Some(self.viewer.as_ref()?.muted_by_list.as_ref()?.name.as_str())
    }

    /// The viewer's block record URI for this actor.
    pub fn blocking_uri(&self) -> Option<&str> {
        self.viewer.as_ref()?.blocking.as_deref()
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

/// The fields of an `app.bsky.feed.post` record bsky shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    #[serde(deserialize_with = "any_opt_string")]
    pub like: Option<String>,
    /// AT-URI of the viewer's repost record, when reposted.
    #[serde(deserialize_with = "any_opt_string")]
    pub repost: Option<String>,
}

/// `app.bsky.embed.defs#aspectRatio`: width and height in any unit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AspectRatio {
    #[serde(deserialize_with = "any_size")]
    pub width: u32,
    #[serde(deserialize_with = "any_size")]
    pub height: u32,
}

/// One image of an `app.bsky.embed.images#view`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageView {
    #[serde(deserialize_with = "any_string")]
    pub thumb: String,
    #[serde(deserialize_with = "any_string")]
    pub fullsize: String,
    #[serde(deserialize_with = "any_string")]
    pub alt: String,
    #[serde(deserialize_with = "any_opt")]
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

/// The embed kinds bsky renders. Anything else is [`Embed::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "$type")]
pub enum Embed {
    #[serde(rename = "app.bsky.embed.images#view")]
    Images {
        #[serde(default, deserialize_with = "readable_items")]
        images: Vec<ImageView>,
    },
    #[serde(rename = "app.bsky.embed.external#view")]
    External { external: ExternalView },
    #[serde(rename = "app.bsky.embed.video#view")]
    Video {
        #[serde(default, deserialize_with = "any_opt_string")]
        thumbnail: Option<String>,
        #[serde(default, rename = "aspectRatio", deserialize_with = "any_opt")]
        aspect_ratio: Option<AspectRatio>,
        /// The HLS playlist to play it from.
        #[serde(default, deserialize_with = "any_opt_string")]
        playlist: Option<String>,
        #[serde(default, deserialize_with = "any_opt_string")]
        alt: Option<String>,
    },
    #[serde(rename = "app.bsky.embed.record#view")]
    Record {
        record: Value,
        /// The quoted post's own picture or video, read once when the post
        /// is read: a quote of a picture post draws that picture, and the
        /// drawing asks for it on every frame.
        #[serde(skip)]
        quoted_media: Option<Box<Embed>>,
    },
    #[serde(rename = "app.bsky.embed.recordWithMedia#view")]
    RecordWithMedia {
        media: Box<Embed>,
        /// The `app.bsky.embed.record#view` beside the media, which holds
        /// the quoted post: a quote with a picture is one of these.
        #[serde(default)]
        record: Value,
    },
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
                ..
            } => thumbnail
                .as_deref()
                .map(|url| EmbedImage {
                    url,
                    aspect: aspect(*aspect_ratio),
                })
                .into_iter()
                .collect(),
            Embed::RecordWithMedia { media, .. } => media.images(),
            // A plain quote draws what the quoted post drew.
            Embed::Record { quoted_media, .. } => quoted_media
                .as_deref()
                .map(Embed::images)
                .unwrap_or_default(),
            Embed::Other => Vec::new(),
        }
    }

    /// Take the quoted post's own media as this embed's own. `viewRecord`
    /// carries the embeds of the post it quotes as raw JSON; they are read
    /// here, once, when the post arrives. Only a picture or a video, and
    /// only one post deep, so a quote of a quote does not reach through to
    /// a third post's pictures.
    fn with_quoted_media(mut self) -> Self {
        if let Embed::Record {
            record,
            quoted_media,
        } = &mut self
        {
            *quoted_media = record
                .get("embeds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|e| serde_json::from_value::<Embed>(e.clone()).ok())
                .map(|e| match e {
                    Embed::RecordWithMedia { media, .. } => *media,
                    other => other,
                })
                .find(|e| matches!(e, Embed::Images { .. } | Embed::Video { .. }))
                .map(Box::new);
        }
        self
    }

    /// The post (or feed, or list) this embed quotes, as the server sent
    /// it. A quote with a picture keeps it beside the media, so both a
    /// plain quote and that one answer here.
    pub fn quoted(&self) -> Option<&Value> {
        match self {
            Embed::Record { record, .. } => Some(record),
            Embed::RecordWithMedia { record, .. } => record.get("record"),
            _ => None,
        }
    }
}

/// Something of a post the viewer opens full screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Media {
    Image {
        /// The full-size picture.
        url: String,
        /// Its small version, already on screen in the list, shown while
        /// the full size downloads.
        thumb: String,
        alt: String,
        aspect: Option<(u32, u32)>,
    },
    Video {
        /// The HLS playlist.
        playlist: String,
        thumbnail: Option<String>,
        alt: String,
        aspect: Option<(u32, u32)>,
    },
}

impl Embed {
    /// The pictures and videos of the post, in order, for the viewer.
    pub fn media(&self) -> Vec<Media> {
        match self {
            Embed::Images { images } => images
                .iter()
                .filter(|i| !i.fullsize.is_empty() || !i.thumb.is_empty())
                .map(|i| Media::Image {
                    url: if i.fullsize.is_empty() {
                        i.thumb.clone()
                    } else {
                        i.fullsize.clone()
                    },
                    thumb: i.thumb.clone(),
                    alt: i.alt.clone(),
                    aspect: aspect(i.aspect_ratio),
                })
                .collect(),
            Embed::Video {
                thumbnail,
                aspect_ratio,
                playlist: Some(playlist),
                alt,
            } => vec![Media::Video {
                playlist: playlist.clone(),
                thumbnail: thumbnail.clone(),
                alt: alt.clone().unwrap_or_default(),
                aspect: aspect(*aspect_ratio),
            }],
            Embed::RecordWithMedia { media, .. } => media.media(),
            // The viewer opens the quoted post's pictures too: they are
            // what the row drew.
            Embed::Record { quoted_media, .. } => quoted_media
                .as_deref()
                .map(Embed::media)
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}

/// `app.bsky.feed.defs#postView`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Post {
    #[serde(deserialize_with = "any_string")]
    pub uri: String,
    #[serde(deserialize_with = "any_string")]
    pub cid: String,
    #[serde(deserialize_with = "any_or_default")]
    pub author: Profile,
    /// The raw record; see [`Post::record`] for the typed view.
    #[serde(rename = "record")]
    pub raw_record: Value,
    #[serde(deserialize_with = "lenient_embed")]
    pub embed: Option<Embed>,
    #[serde(deserialize_with = "any_count")]
    pub reply_count: u64,
    #[serde(deserialize_with = "any_count")]
    pub repost_count: u64,
    #[serde(deserialize_with = "any_count")]
    pub like_count: u64,
    #[serde(deserialize_with = "any_string")]
    pub indexed_at: String,
    #[serde(deserialize_with = "any_opt")]
    pub viewer: Option<PostViewer>,
    /// The thread above a reply, when the feed gave it. Not part of the
    /// lexicon's postView: bsky attaches it from the feedViewPost around it.
    #[serde(skip)]
    pub context: Option<Box<ReplyContext>>,
}

/// A post referenced from a reply or a thread: the post itself, or what the
/// AppView says instead when it cannot show it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "$type")]
pub enum RefPost {
    #[serde(rename = "app.bsky.feed.defs#postView")]
    Post(Box<Post>),
    #[serde(rename = "app.bsky.feed.defs#notFoundPost")]
    NotFound {
        #[serde(default)]
        uri: String,
    },
    #[serde(rename = "app.bsky.feed.defs#blockedPost")]
    Blocked {
        #[serde(default)]
        uri: String,
    },
    #[serde(other)]
    Other,
}

impl RefPost {
    /// The referenced post's URI, when known.
    pub fn uri(&self) -> Option<&str> {
        match self {
            RefPost::Post(p) => Some(&p.uri),
            RefPost::NotFound { uri } | RefPost::Blocked { uri } => Some(uri),
            RefPost::Other => None,
        }
    }
}

/// `app.bsky.feed.defs#replyRef` in a feed: the posts a reply belongs under.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedReply {
    pub root: RefPost,
    pub parent: RefPost,
    #[serde(default)]
    pub grandparent_author: Option<Profile>,
}

/// What is shown above a reply in a feed, so it reads in context.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyContext {
    /// The thread's first post, when the reply is not directly under it.
    pub root: Option<RefPost>,
    /// Whether posts sit between the root and the parent, shown as a gap.
    pub gap: bool,
    /// The post being answered.
    pub parent: RefPost,
}

impl ReplyContext {
    /// The context for a reply, from the feed's reply references.
    pub fn from_reply(reply: FeedReply) -> Self {
        let root_uri = reply.root.uri().map(str::to_string);
        let same = root_uri.is_some() && root_uri.as_deref() == reply.parent.uri();
        // The parent answers something other than the root: there is more
        // thread between them than is shown.
        let gap = !same
            && match &reply.parent {
                RefPost::Post(p) => p
                    .record()
                    .reply
                    .is_some_and(|r| Some(r.parent.uri.as_str()) != root_uri.as_deref()),
                _ => false,
            };
        Self {
            root: (!same).then_some(reply.root),
            gap,
            parent: reply.parent,
        }
    }
}

/// `app.bsky.feed.defs#threadViewPost` and the placeholders a thread can hold.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "$type")]
pub enum ThreadNode {
    #[serde(rename = "app.bsky.feed.defs#threadViewPost")]
    Post {
        post: Box<Post>,
        #[serde(default, deserialize_with = "lenient_parent")]
        parent: Option<Box<ThreadNode>>,
        #[serde(default, deserialize_with = "lenient_replies")]
        replies: Vec<ThreadNode>,
    },
    #[serde(rename = "app.bsky.feed.defs#notFoundPost")]
    NotFound {
        #[serde(default)]
        uri: String,
    },
    #[serde(rename = "app.bsky.feed.defs#blockedPost")]
    Blocked {
        #[serde(default)]
        uri: String,
    },
    #[serde(other)]
    Other,
}

fn lenient_parent<'de, D>(de: D) -> Result<Option<Box<ThreadNode>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(de)?;
    Ok(value
        .and_then(|v| serde_json::from_value(v).ok())
        .map(Box::new))
}

fn lenient_replies<'de, D>(de: D) -> Result<Vec<ThreadNode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // One reply the client cannot read must not hide the others.
    let values = Option::<Vec<Value>>::deserialize(de)?.unwrap_or_default();
    Ok(values
        .into_iter()
        .map(|v| serde_json::from_value(v).unwrap_or(ThreadNode::Other))
        .collect())
}

/// `app.bsky.feed.getPostThread` output.
#[derive(Debug, Clone, Deserialize)]
pub struct PostThread {
    pub thread: ThreadNode,
}

fn lenient_embed<'de, D>(de: D) -> Result<Option<Embed>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // A malformed embed must not hide the post it belongs to.
    let value = Option::<Value>::deserialize(de)?;
    Ok(value
        .and_then(|v| serde_json::from_value::<Embed>(v).ok())
        .map(Embed::with_quoted_media))
}

impl Post {
    /// The record decoded into the fields bsky shows.
    ///
    /// Each field is read on its own: any client writes records and the
    /// AppView passes them on as written, so a reply without its CIDs or a
    /// time that is not a string leaves the text shown, not a blank post.
    pub fn record(&self) -> PostRecord {
        let r = &self.raw_record;
        let text = |k: &str| r.get(k).and_then(Value::as_str).map(str::to_string);
        PostRecord {
            text: text("text").unwrap_or_default(),
            created_at: text("createdAt"),
            reply: r.get("reply").and_then(|v| ReplyRef::deserialize(v).ok()),
        }
    }

    /// The post on bsky.app, for `at://<did>/app.bsky.feed.post/<rkey>`.
    pub fn web_url(&self) -> Option<String> {
        let rest = self.uri.strip_prefix("at://")?;
        let (did, rkey) = rest.split_once("/app.bsky.feed.post/")?;
        (!did.is_empty() && !rkey.is_empty() && !rkey.contains('/'))
            .then(|| format!("https://bsky.app/profile/{did}/post/{rkey}"))
    }

    /// The web links of the post, in order: its link card, then the links in
    /// its text. Only http(s) links; each once.
    pub fn links(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut add = |u: &str| {
            if (u.starts_with("https://") || u.starts_with("http://"))
                && !out.iter().any(|x| x == u)
            {
                out.push(u.to_string());
            }
        };
        let card = match &self.embed {
            Some(Embed::External { external }) => Some(&external.uri),
            Some(Embed::RecordWithMedia { media, .. }) => match media.as_ref() {
                Embed::External { external } => Some(&external.uri),
                _ => None,
            },
            _ => None,
        };
        if let Some(uri) = card {
            add(uri);
        }
        let facets = self.raw_record.get("facets").and_then(Value::as_array);
        for feature in facets
            .into_iter()
            .flatten()
            .filter_map(|f| f.get("features")?.as_array())
            .flatten()
        {
            if feature.get("$type").and_then(Value::as_str) == Some("app.bsky.richtext.facet#link")
                && let Some(uri) = feature.get("uri").and_then(Value::as_str)
            {
                add(uri);
            }
        }
        out
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

    /// The viewer's repost record URI, when reposted.
    pub fn repost_uri(&self) -> Option<&str> {
        self.viewer.as_ref()?.repost.as_deref()
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
    /// Present when the post is a reply: the thread it belongs under.
    #[serde(deserialize_with = "lenient_reply")]
    pub reply: Option<FeedReply>,
}

fn lenient_reply<'de, D>(de: D) -> Result<Option<FeedReply>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // A reply context the client cannot read must not hide the reply.
    let value = Option::<Value>::deserialize(de)?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

/// `app.bsky.feed.getTimeline` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Timeline {
    #[serde(deserialize_with = "readable_items")]
    pub feed: Vec<FeedItem>,
    pub cursor: Option<String>,
}

/// `app.bsky.actor.getPreferences` output.
#[derive(Debug, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub preferences: Vec<Value>,
}

/// A custom feed's generator view, as `app.bsky.feed.getFeedGenerators`
/// returns it (only what bsky shows).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedGenerator {
    pub uri: String,
    #[serde(default, deserialize_with = "any_string")]
    pub display_name: String,
}

/// `app.bsky.feed.getFeedGenerators` output. A feed the server describes
/// oddly (no address) is left out, not the whole list.
#[derive(Debug, Deserialize)]
pub struct FeedGenerators {
    #[serde(default, deserialize_with = "readable_items")]
    pub feeds: Vec<FeedGenerator>,
}

/// A pinned custom feed: where it is and what it is called.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedInfo {
    pub uri: String,
    pub name: String,
}

/// `app.bsky.feed.getAuthorFeed` output (same shape as the timeline).
pub type AuthorFeed = Timeline;

/// `app.bsky.feed.searchPosts` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchPosts {
    #[serde(deserialize_with = "readable_items")]
    pub posts: Vec<Post>,
    pub cursor: Option<String>,
}

/// `app.bsky.actor.searchActors` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchActors {
    #[serde(deserialize_with = "readable_items")]
    pub actors: Vec<Profile>,
    pub cursor: Option<String>,
}

/// `app.bsky.notification.listNotifications#notification`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Notification {
    /// The record that caused it: the like, the follow, the reply post, ...
    #[serde(deserialize_with = "any_string")]
    pub uri: String,
    #[serde(deserialize_with = "any_string")]
    pub cid: String,
    #[serde(deserialize_with = "any_or_default")]
    pub author: Profile,
    /// like, repost, follow, mention, reply, quote, and newer values.
    #[serde(deserialize_with = "any_string")]
    pub reason: String,
    /// For a like or repost, the post (or repost) it is about.
    #[serde(deserialize_with = "any_opt_string")]
    pub reason_subject: Option<String>,
    #[serde(deserialize_with = "any_bool")]
    pub is_read: bool,
    #[serde(deserialize_with = "any_string")]
    pub indexed_at: String,
}

/// `app.bsky.notification.listNotifications` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Notifications {
    #[serde(deserialize_with = "readable_items")]
    pub notifications: Vec<Notification>,
    pub cursor: Option<String>,
}

/// `app.bsky.feed.getPosts` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Posts {
    #[serde(deserialize_with = "readable_items")]
    pub posts: Vec<Post>,
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
    #[serde(default)]
    pub cid: String,
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
    // Read leniently: a `"message": null` failed the whole body, and the
    // error name with it (an ExpiredToken was then not refreshed).
    #[serde(deserialize_with = "any_string")]
    pub error: String,
    #[serde(deserialize_with = "any_string")]
    pub message: String,
}

#[cfg(test)]
mod link_tests {
    use super::*;
    use serde_json::json;

    #[rstest::rstest]
    #[case(
        "at://did:plc:alice/app.bsky.feed.post/3k",
        Some("https://bsky.app/profile/did:plc:alice/post/3k")
    )]
    #[case("at://did:plc:alice/app.bsky.feed.like/3k", None)]
    #[case("at://did:plc:alice/app.bsky.feed.post/", None)]
    #[case("https://bsky.app/x", None)]
    fn a_post_has_its_page_on_bsky_app(#[case] uri: &str, #[case] want: Option<&str>) {
        let post = Post {
            uri: uri.into(),
            ..Post::default()
        };
        assert_eq!(post.web_url().as_deref(), want);
    }

    #[test]
    fn links_come_from_the_card_then_the_text_web_only_and_once() {
        let post: Post = serde_json::from_value(json!({
            "uri": "at://a/p/1", "cid": "c", "author": {"did": "d", "handle": "h.test"},
            "record": {"text": "see", "facets": [
                {"index": {"byteStart": 0, "byteEnd": 3}, "features": [
                    {"$type": "app.bsky.richtext.facet#link", "uri": "https://b.test/x"}]},
                {"index": {"byteStart": 4, "byteEnd": 5}, "features": [
                    {"$type": "app.bsky.richtext.facet#mention", "did": "did:plc:z"}]},
                {"index": {"byteStart": 6, "byteEnd": 7}, "features": [
                    {"$type": "app.bsky.richtext.facet#link", "uri": "javascript:alert(1)"},
                    {"$type": "app.bsky.richtext.facet#link", "uri": "https://a.test/card"}]}
            ]},
            "embed": {"$type": "app.bsky.embed.external#view",
                      "external": {"uri": "https://a.test/card", "title": "A", "description": ""}}
        }))
        .unwrap();
        assert_eq!(post.links(), ["https://a.test/card", "https://b.test/x"]);
    }
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
    fn a_quote_with_a_picture_keeps_the_post_it_quotes() {
        let e: Embed = serde_json::from_value(json!({
            "$type": "app.bsky.embed.recordWithMedia#view",
            "record": {"$type": "app.bsky.embed.record#view", "record": {
                "$type": "app.bsky.embed.record#viewRecord",
                "uri": "at://did:plc:bob/app.bsky.feed.post/q", "cid": "c",
                "author": {"did": "did:plc:bob", "handle": "bob.test"},
                "value": {"text": "the quoted words"}
            }},
            "media": {"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://cdn/m", "fullsize": "f", "alt": ""}
            ]}
        }))
        .unwrap();
        let quoted = e.quoted().expect("the quoted post");
        assert_eq!(quoted.pointer("/author/handle").unwrap(), "bob.test");
        assert_eq!(quoted.pointer("/value/text").unwrap(), "the quoted words");
        // A plain quote gives the same thing.
        let e: Embed = serde_json::from_value(json!({
            "$type": "app.bsky.embed.record#view",
            "record": {"author": {"handle": "bob.test"}, "value": {"text": "hi"}}
        }))
        .unwrap();
        assert_eq!(
            e.quoted().unwrap().pointer("/author/handle").unwrap(),
            "bob.test"
        );
        // A post with nothing quoted says so.
        let e: Embed = serde_json::from_value(json!({
            "$type": "app.bsky.embed.images#view", "images": []
        }))
        .unwrap();
        assert!(e.quoted().is_none());
    }

    /// A quote of a post that has a picture shows that picture, the way
    /// the quoted post itself would, and the viewer opens it.
    #[test]
    fn a_quote_shows_the_quoted_post_s_picture() {
        let quote = |embeds: serde_json::Value| -> Post {
            post(json!({
                "uri": "at://a/p/1", "cid": "c",
                "author": {"did": "d", "handle": "alice.test"},
                "record": {"text": "look"},
                "embed": {"$type": "app.bsky.embed.record#view", "record": {
                    "$type": "app.bsky.embed.record#viewRecord",
                    "uri": "at://b/p/q", "cid": "cq",
                    "author": {"did": "db", "handle": "bob.test"},
                    "value": {"text": "the quoted words"},
                    "embeds": embeds,
                }}
            }))
        };
        let e = quote(json!([{"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://cdn/q", "fullsize": "https://cdn/qfull", "alt": "山",
             "aspectRatio": {"width": 4, "height": 3}}
        ]}]))
        .embed
        .unwrap();
        assert_eq!(e.images()[0].url, "https://cdn/q");
        assert_eq!(e.images()[0].aspect, Some((4, 3)));
        assert!(
            matches!(&e.media()[..], [Media::Image { url, alt, .. }]
                     if url == "https://cdn/qfull" && alt == "山"),
            "{:?}",
            e.media()
        );
        // A quoted post with a video gives the video.
        let e = quote(json!([{"$type": "app.bsky.embed.video#view",
                              "playlist": "https://v/p.m3u8", "alt": "a cat"}]))
        .embed
        .unwrap();
        assert!(
            matches!(&e.media()[..], [Media::Video { playlist, .. }] if playlist == "https://v/p.m3u8"),
            "{:?}",
            e.media()
        );
        // A quoted post with only words has nothing to draw.
        let e = quote(json!([])).embed.unwrap();
        assert!(e.images().is_empty());
        assert!(e.media().is_empty());
        // A quote of a quote does not reach through two posts.
        let e = quote(json!([{"$type": "app.bsky.embed.record#view", "record": {
            "$type": "app.bsky.embed.record#viewRecord",
            "author": {"handle": "carol.test"}, "value": {"text": "deeper"},
            "embeds": [{"$type": "app.bsky.embed.images#view", "images": [
                {"thumb": "https://cdn/deep", "fullsize": "https://cdn/deep", "alt": ""}
            ]}]
        }}]))
        .embed
        .unwrap();
        assert!(e.images().is_empty());
    }

    /// A quote that carries a picture of its own draws that one: the row
    /// shows what the post added, not what it quotes.
    #[test]
    fn a_quote_with_its_own_picture_draws_its_own() {
        let p = post(json!({
            "uri": "at://a/p/1", "cid": "c",
            "author": {"did": "d", "handle": "alice.test"},
            "record": {"text": "look"},
            "embed": {"$type": "app.bsky.embed.recordWithMedia#view",
                "record": {"$type": "app.bsky.embed.record#view", "record": {
                    "$type": "app.bsky.embed.record#viewRecord",
                    "author": {"handle": "bob.test"}, "value": {"text": "quoted"},
                    "embeds": [{"$type": "app.bsky.embed.images#view", "images": [
                        {"thumb": "https://cdn/theirs", "fullsize": "https://cdn/theirs", "alt": ""}
                    ]}]
                }},
                "media": {"$type": "app.bsky.embed.images#view", "images": [
                    {"thumb": "https://cdn/mine", "fullsize": "https://cdn/mine", "alt": ""}
                ]}
            }
        }));
        let e = p.embed.unwrap();
        assert_eq!(e.images().len(), 1);
        assert_eq!(e.images()[0].url, "https://cdn/mine");
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
        assert!(p.embed.as_ref().is_none_or(|e| e.images().is_empty()));
        assert_eq!(p.record().text, "x");
    }

    // Records are written by any client and the AppView passes them on as
    // they are: one bad field must not blank the whole post.
    #[rstest::rstest]
    #[case::reply_without_cids(json!({"root": {"uri": "at://top"}, "parent": {"uri": "at://top"}}))]
    #[case::reply_as_a_string(json!("at://top"))]
    #[case::reply_null(json!(null))]
    fn a_post_with_a_damaged_reply_keeps_its_text_and_time(#[case] reply: Value) {
        let text = "家族 👨‍👩‍👧 と 🇯🇵 へ 1️⃣ ❤️ e\u{301} שלום";
        let p = post(json!({
            "uri": "at://mid", "cid": "c2", "author": {},
            "record": {"text": text, "createdAt": "2026-09-20T10:00:00Z", "reply": reply}
        }));
        let r = p.record();
        assert_eq!(r.text, text);
        assert_eq!(r.created_at.as_deref(), Some("2026-09-20T10:00:00Z"));
        assert_eq!(r.reply, None);
        // Answering it starts from the post itself, as for a top post.
        let reply = p.reply_ref();
        assert_eq!(
            (reply.root.uri.as_str(), reply.parent.uri.as_str()),
            ("at://mid", "at://mid")
        );
    }

    #[test]
    fn a_post_whose_time_is_not_a_string_keeps_its_text_and_reply() {
        let p = post(json!({
            "uri": "at://mid", "cid": "c2", "author": {},
            "record": {"text": "still here", "createdAt": 1_758_000_000, "reply": {
                "root": {"uri": "at://top", "cid": "c1"},
                "parent": {"uri": "at://top", "cid": "c1"}
            }}
        }));
        let r = p.record();
        assert_eq!(r.text, "still here");
        assert_eq!(r.created_at, None);
        assert_eq!(r.reply.map(|x| x.root.uri).as_deref(), Some("at://top"));
    }

    fn feed_post(uri: &str) -> Value {
        json!({"post": {
            "uri": uri, "cid": "c", "author": {"did": "did:plc:a", "handle": "a.test"},
            "record": {"text": format!("text of {uri}")}, "likeCount": 4,
            "indexedAt": "2026-09-20T10:00:00Z"
        }})
    }

    /// A page of three posts whose middle one has `path` replaced by `bad`.
    fn page_with(path: &[&str], bad: Value) -> Timeline {
        let mut middle = feed_post("at://2");
        let mut at = &mut middle["post"];
        for k in &path[..path.len() - 1] {
            at = &mut at[*k];
        }
        at[path[path.len() - 1]] = bad;
        serde_json::from_value(json!({"feed": [feed_post("at://1"), middle, feed_post("at://3")]}))
            .unwrap()
    }

    // One odd field in one post must neither fail the page nor lose the
    // post: the field reads as missing, the rest as sent.
    #[rstest::rstest]
    #[case::count_null(&["likeCount"], json!(null))]
    #[case::count_string(&["likeCount"], json!("3"))]
    #[case::count_negative(&["replyCount"], json!(-1))]
    #[case::time_null(&["indexedAt"], json!(null))]
    #[case::cid_null(&["cid"], json!(null))]
    #[case::viewer_like_number(&["viewer"], json!({"like": 1}))]
    #[case::avatar_number(&["author", "avatar"], json!(5))]
    #[case::handle_null(&["author", "handle"], json!(null))]
    #[case::followers_negative(&["author", "followersCount"], json!(-1))]
    #[case::display_name_object(&["author", "displayName"], json!({"x": 1}))]
    fn a_post_with_one_odd_field_keeps_its_page(#[case] path: &[&str], #[case] bad: Value) {
        let page = page_with(path, bad);
        let uris: Vec<_> = page.feed.iter().map(|i| i.post.uri.as_str()).collect();
        assert_eq!(uris, ["at://1", "at://2", "at://3"]);
        assert_eq!(page.feed[1].post.record().text, "text of at://2");
    }

    #[test]
    fn an_item_that_is_not_a_post_is_left_out_of_its_page() {
        let page: Timeline = serde_json::from_value(
            json!({"feed": [feed_post("at://1"), 7, {"post": "x"}, feed_post("at://3")], "cursor": "c"}),
        )
        .unwrap();
        let uris: Vec<_> = page.feed.iter().map(|i| i.post.uri.as_str()).collect();
        assert_eq!(uris, ["at://1", "at://3"]);
        assert_eq!(page.cursor.as_deref(), Some("c"));
    }

    #[test]
    fn other_lists_keep_their_readable_items() {
        let n: Notifications = serde_json::from_value(json!({"notifications": [
            {"uri": "at://n/1", "author": {"did": "d", "handle": "h", "avatar": 1}, "reason": "like", "isRead": "no"},
            "junk",
            {"uri": "at://n/2", "author": {"did": "d", "handle": "h"}, "reason": "follow"}
        ]}))
        .unwrap();
        let uris: Vec<_> = n.notifications.iter().map(|x| x.uri.as_str()).collect();
        assert_eq!(uris, ["at://n/1", "at://n/2"]);
        let a: SearchActors = serde_json::from_value(json!({"actors": [
            {"did": "d1", "handle": "a.test", "followsCount": "many"}, null, {"did": "d2", "handle": "b.test"}
        ]}))
        .unwrap();
        assert_eq!(a.actors.len(), 2);
        let s: SearchPosts =
            serde_json::from_value(json!({"posts": [feed_post("at://1")["post"], 3]})).unwrap();
        assert_eq!(s.posts.len(), 1);
    }

    // alt is written by whoever posted; one odd image must not hide the
    // others, nor a fractional aspect ratio the video.
    #[test]
    fn an_odd_image_or_aspect_ratio_keeps_the_media() {
        let e: Embed = serde_json::from_value(json!({"$type": "app.bsky.embed.images#view", "images": [
            {"thumb": "https://t/1", "fullsize": "https://f/1", "alt": null},
            {"thumb": "https://t/2", "fullsize": "https://f/2", "alt": "b", "aspectRatio": {"width": "4", "height": 3}},
            "not an image"
        ]}))
        .unwrap();
        let urls: Vec<_> = e.images().iter().map(|i| i.url).collect();
        assert_eq!(urls, ["https://t/1", "https://t/2"]);
        let v: Embed = serde_json::from_value(json!({"$type": "app.bsky.embed.video#view",
            "playlist": "https://v/p.m3u8", "thumbnail": "https://v/t.jpg",
            "aspectRatio": {"width": 1080.0, "height": 1920.5}}))
        .unwrap();
        assert_eq!(v.images()[0].aspect, Some((1080, 1921)));
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

    fn reply_item(reply: Value) -> FeedItem {
        serde_json::from_value(json!({
            "post": {"uri": "at://me/p/child", "cid": "c", "author": {"did": "d", "handle": "h"}, "record": {"text": "child"}},
            "reply": reply,
        }))
        .unwrap()
    }

    fn view(uri: &str, parent_of: Option<&str>) -> Value {
        let mut record = json!({"text": format!("text of {uri}")});
        if let Some(p) = parent_of {
            record["reply"] =
                json!({"root": {"uri": "at://root", "cid": "c"}, "parent": {"uri": p, "cid": "c"}});
        }
        json!({"$type": "app.bsky.feed.defs#postView", "uri": uri, "cid": "c",
               "author": {"did": "d", "handle": "a.test"}, "record": record})
    }

    #[test]
    fn a_reply_directly_under_the_root_shows_only_the_parent() {
        let item =
            reply_item(json!({"root": view("at://root", None), "parent": view("at://root", None)}));
        let ctx = ReplyContext::from_reply(item.reply.unwrap());
        assert!(ctx.root.is_none());
        assert!(!ctx.gap);
        assert_eq!(ctx.parent.uri(), Some("at://root"));
    }

    #[test]
    fn a_deeper_reply_shows_the_root_and_marks_the_gap() {
        // parent answers "mid", not the root: something sits in between.
        let item = reply_item(json!({
            "root": view("at://root", None),
            "parent": view("at://parent", Some("at://mid")),
            "grandparentAuthor": {"did": "g", "handle": "g.test"}
        }));
        let ctx = ReplyContext::from_reply(item.reply.unwrap());
        assert_eq!(ctx.root.as_ref().and_then(RefPost::uri), Some("at://root"));
        assert!(ctx.gap);
        // parent answers the root itself: no gap.
        let item = reply_item(
            json!({"root": view("at://root", None), "parent": view("at://parent", Some("at://root"))}),
        );
        assert!(!ReplyContext::from_reply(item.reply.unwrap()).gap);
    }

    #[test]
    fn missing_and_blocked_posts_in_a_reply_are_read_as_placeholders() {
        let item = reply_item(json!({
            "root": {"$type": "app.bsky.feed.defs#notFoundPost", "uri": "at://gone", "notFound": true},
            "parent": {"$type": "app.bsky.feed.defs#blockedPost", "uri": "at://b", "blocked": true, "author": {"did": "x"}}
        }));
        let reply = item.reply.unwrap();
        assert!(matches!(reply.root, RefPost::NotFound { ref uri } if uri == "at://gone"));
        assert!(matches!(reply.parent, RefPost::Blocked { ref uri } if uri == "at://b"));
    }

    #[test]
    fn an_unreadable_reply_context_keeps_the_post() {
        let item = reply_item(json!({"root": 1}));
        assert!(item.reply.is_none());
        assert_eq!(item.post.record().text, "child");
        let item = reply_item(
            json!({"root": {"$type": "app.bsky.feed.defs#future"}, "parent": view("at://p", None)}),
        );
        assert_eq!(item.reply.unwrap().root, RefPost::Other);
    }
}

/// A message of a conversation, `chat.bsky.convo.defs#messageView`, or one
/// that was deleted or is the service's own note, read the same way.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatMessage {
    pub id: String,
    /// Empty for a deleted message.
    pub text: String,
    /// The sender's DID.
    pub sender: String,
    pub sent_at: String,
    /// Deleted by its sender: there is no text to show.
    pub deleted: bool,
    /// A note of the service (someone joined, left...), not a message.
    pub system: bool,
}

impl ChatMessage {
    /// Read one message of any kind; `None` for what is not one at all.
    pub fn from_value(v: &Value) -> Option<Self> {
        let kind = v.get("$type").and_then(Value::as_str).unwrap_or("");
        let text = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let id = text("id");
        if id.is_empty() {
            return None;
        }
        Some(Self {
            id,
            text: text("text"),
            sender: v
                .pointer("/sender/did")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            sent_at: text("sentAt"),
            deleted: kind.ends_with("#deletedMessageView"),
            system: kind.ends_with("#systemMessageView"),
        })
    }
}

impl<'de> Deserialize<'de> for ChatMessage {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(de)?;
        ChatMessage::from_value(&v).ok_or_else(|| serde::de::Error::custom("not a message"))
    }
}

/// A conversation, `chat.bsky.convo.defs#convoView`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Convo {
    #[serde(deserialize_with = "any_string")]
    pub id: String,
    #[serde(deserialize_with = "readable_items")]
    pub members: Vec<Profile>,
    #[serde(deserialize_with = "lenient_message")]
    pub last_message: Option<ChatMessage>,
    #[serde(deserialize_with = "any_count")]
    pub unread_count: u64,
    #[serde(deserialize_with = "any_bool")]
    pub muted: bool,
}

impl Convo {
    /// Everyone in it but `me`: the other person of a one-to-one chat.
    pub fn others(&self, me: &str) -> Vec<&Profile> {
        self.members.iter().filter(|m| m.did != me).collect()
    }
}

fn lenient_message<'de, D: Deserializer<'de>>(de: D) -> Result<Option<ChatMessage>, D::Error> {
    Ok(ChatMessage::from_value(&Value::deserialize(de)?))
}

/// `chat.bsky.convo.listConvos` output.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Convos {
    #[serde(deserialize_with = "readable_items")]
    pub convos: Vec<Convo>,
    pub cursor: Option<String>,
}

/// `chat.bsky.convo.getMessages` output: newest first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Messages {
    #[serde(deserialize_with = "readable_items")]
    pub messages: Vec<ChatMessage>,
    pub cursor: Option<String>,
}
