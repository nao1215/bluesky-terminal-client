//! The parts of Mastodon's entities mstdn reads
//! (docs.joinmastodon.org/entities). Every field has a default, so an
//! answer from an older version, a fork, or another server software that
//! speaks the Mastodon API still reads: what it leaves out is empty.

use serde::Deserialize;

/// An account (`Account`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Account {
    pub id: String,
    /// The name without the server.
    pub username: String,
    /// `username` for an account of the server asked, `username@host` for
    /// one of another.
    pub acct: String,
    pub display_name: String,
    pub emojis: Vec<Emoji>,
    /// The profile's address on its own server.
    pub url: Option<String>,
    /// What the account says about itself, as HTML.
    pub note: String,
    pub source: Option<AccountSource>,
    pub discoverable: Option<bool>,
    pub locked: bool,
    pub bot: bool,
    pub followers_count: Option<u64>,
    pub following_count: Option<u64>,
    pub statuses_count: Option<u64>,
}

/// Unformatted fields returned only for the authenticated account.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct AccountSource {
    pub note: String,
    pub fields: Vec<ProfileField>,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProfileField {
    pub name: String,
    pub value: String,
}

impl Account {
    /// The server the account is on: the one after `@` in `acct`, else
    /// `local`, the host of the server asked.
    pub(crate) fn host<'a>(&'a self, local: &'a str) -> &'a str {
        match self.acct.split_once('@') {
            Some((_, h)) if !h.is_empty() => h,
            _ => local,
        }
    }
}

/// Custom emoji supplied with the text that references them.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Emoji {
    pub shortcode: String,
    pub url: String,
    pub static_url: String,
}

/// A file attached to a status (`MediaAttachment`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Media {
    pub id: String,
    /// `image`, `gifv`, `video`, `audio` or `unknown`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The file on the server asked; `None` while it is being processed.
    pub url: Option<String>,
    pub preview_url: Option<String>,
    /// The file on its own server, for one of another server.
    pub remote_url: Option<String>,
    /// The alt text.
    pub description: Option<String>,
}

impl Media {
    /// The address to fetch the file from: the server's copy, else the
    /// one on the server it came from.
    pub(crate) fn source(&self) -> Option<&str> {
        [self.url.as_deref(), self.remote_url.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|u| !u.is_empty())
    }
}

/// An account a status mentions.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Mention {
    pub id: String,
    pub username: String,
    pub acct: String,
    pub url: String,
}

/// A hashtag a status has.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Tag {
    pub name: String,
    pub url: String,
}

/// One choice of a poll.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct PollOption {
    pub title: String,
    pub votes_count: Option<u64>,
}

/// A poll.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Poll {
    pub expired: bool,
    pub votes_count: u64,
    pub options: Vec<PollOption>,
}

/// A post (`Status`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Status {
    pub id: String,
    pub created_at: String,
    /// The ActivityPub address of the status, on its own server.
    pub uri: String,
    /// Its web page, on its own server.
    pub url: Option<String>,
    pub account: Account,
    /// The text, as HTML.
    pub content: String,
    /// The content warning; empty when there is none.
    pub spoiler_text: String,
    pub sensitive: bool,
    /// `public`, `unlisted`, `private` or `direct`.
    pub visibility: String,
    pub language: Option<String>,
    pub in_reply_to_id: Option<String>,
    pub in_reply_to_account_id: Option<String>,
    /// The status this one boosts, for a boost.
    pub reblog: Option<Box<Status>>,
    pub quote: Option<Quote>,
    pub pleroma: PleromaStatus,
    pub quote_approval: Option<QuoteApproval>,
    pub media_attachments: Vec<Media>,
    pub emojis: Vec<Emoji>,
    pub mentions: Vec<Mention>,
    pub tags: Vec<Tag>,
    pub poll: Option<Poll>,
    pub replies_count: u64,
    pub reblogs_count: u64,
    pub favourites_count: u64,
    /// Whether the account asking favourited it; a visitor is told nothing.
    pub favourited: Option<bool>,
    /// Whether the account asking boosted it.
    pub reblogged: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Quote {
    pub state: String,
    pub quoted_status: Option<Box<Status>>,
}
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct QuoteApproval {
    pub current_user: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct PleromaStatus {
    pub quote: Option<Box<Status>>,
    pub quote_id: Option<String>,
}

impl Status {
    pub(crate) fn quote_view(&self) -> Option<(&str, Option<&Status>)> {
        if let Some(quote) = &self.quote {
            Some((&quote.state, quote.quoted_status.as_deref()))
        } else if self.pleroma.quote_id.is_some() {
            Some(("accepted", self.pleroma.quote.as_deref()))
        } else {
            None
        }
    }
    /// What a reader sees: the status a boost boosts, else the status.
    pub(crate) fn shown(&self) -> &Status {
        self.reblog.as_deref().unwrap_or(self)
    }

    /// The files of the status shown, each with the status it is attached
    /// to and its place there from 1.
    pub(crate) fn attached(&self) -> Vec<(&Status, usize, &Media)> {
        let s = self.shown();
        s.media_attachments
            .iter()
            .enumerate()
            .map(|(i, m)| (s, i + 1, m))
            .collect()
    }
}

/// A notification.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub created_at: String,
    pub account: Option<Account>,
    pub status: Option<Status>,
}

/// The context of a status: what it answers, and the replies under it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Context {
    pub ancestors: Vec<Status>,
    pub descendants: Vec<Status>,
}

/// What `/api/v2/search` answers.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Found {
    pub accounts: Vec<Account>,
    pub statuses: Vec<Status>,
}

/// What `POST /api/v1/apps` answers.
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Registered {
    pub client_id: String,
    pub client_secret: String,
}

/// What `POST /oauth/token` answers. No `Debug`: it holds the token.
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Token {
    pub access_token: String,
    pub scope: String,
}

/// The limits `/api/v2/instance` names for a post, each with Mastodon's
/// default for a server that does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    pub max_characters: usize,
    pub characters_reserved_per_url: usize,
    pub max_media_attachments: usize,
    pub description_limit: usize,
    pub image_size_limit: Option<u64>,
    pub video_size_limit: Option<u64>,
    /// Pleroma-compatible uploads bound the entire multipart request.
    pub media_body_limit: Option<u64>,
    pub extension_quotes: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_characters: 500,
            characters_reserved_per_url: 23,
            max_media_attachments: 4,
            description_limit: 1500,
            image_size_limit: None,
            video_size_limit: None,
            media_body_limit: None,
            extension_quotes: false,
        }
    }
}

impl Limits {
    /// The limits in an instance's `configuration`, the defaults for what
    /// it leaves out.
    pub(crate) fn from_instance(v: &serde_json::Value) -> Limits {
        let d = Limits::default();
        let c = &v["configuration"];
        let metadata = &v["pleroma"]["metadata"];
        let pleroma_api = metadata["features"]
            .as_array()
            .is_some_and(|features| features.iter().any(|f| f == "pleroma_api"));
        let n = |v: &serde_json::Value| {
            v.as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n > 0)
        };
        Limits {
            extension_quotes: metadata["features"]
                .as_array()
                .is_some_and(|features| features.iter().any(|f| f == "quote_posting")),
            max_characters: n(&c["statuses"]["max_characters"])
                // An old version or a fork says it at the top.
                .or_else(|| n(&v["max_toot_chars"]))
                .unwrap_or(d.max_characters),
            characters_reserved_per_url: c["statuses"]["characters_reserved_per_url"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(if pleroma_api {
                    0
                } else {
                    d.characters_reserved_per_url
                }),
            max_media_attachments: c["statuses"]["max_media_attachments"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .or_else(|| {
                    v["max_media_attachments"]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                })
                .unwrap_or(d.max_media_attachments),
            description_limit: n(&c["media_attachments"]["description_limit"])
                .or_else(|| n(&v["description_limit"]))
                .or_else(|| n(&metadata["description_limit"]))
                .unwrap_or(d.description_limit),
            image_size_limit: c["media_attachments"]["image_size_limit"]
                .as_u64()
                .or_else(|| v["upload_limit"].as_u64())
                .filter(|n| *n > 0),
            video_size_limit: c["media_attachments"]["video_size_limit"]
                .as_u64()
                .or_else(|| v["upload_limit"].as_u64())
                .filter(|n| *n > 0),
            // The advertised Pleroma API extension uses Plug's multipart
            // request budget. Neither a Mastodon-looking version string nor
            // an absent feature in the extension list decides this behavior.
            media_body_limit: pleroma_api
                .then(|| {
                    v["upload_limit"]
                        .as_u64()
                        .or_else(|| c["media_attachments"]["video_size_limit"].as_u64())
                        .filter(|n| *n > 0)
                })
                .flatten(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // An answer with only an ID still reads, and nulls read as empty.
    #[test]
    fn a_status_with_little_in_it_still_reads() {
        let s: Status =
            serde_json::from_value(json!({"id": "1", "url": null, "reblog": null})).unwrap();
        assert_eq!(s.id, "1");
        assert!(s.shown().media_attachments.is_empty());
    }

    #[test]
    fn the_host_of_an_account_is_after_the_at() {
        let a = Account {
            acct: "bob@other.test".into(),
            ..Default::default()
        };
        assert_eq!(a.host("here.test"), "other.test");
        let a = Account {
            acct: "alice".into(),
            ..Default::default()
        };
        assert_eq!(a.host("here.test"), "here.test");
    }

    // A boost's files are those of the status it boosts.
    #[test]
    fn a_boost_shows_the_boosted_status_and_its_files() {
        let s: Status = serde_json::from_value(json!({
            "id": "b", "reblog": {"id": "o", "media_attachments": [
                {"id": "m1", "type": "image", "url": "https://x.test/1.png"},
                {"id": "m2", "type": "video", "url": null, "remote_url": "https://y.test/2.mp4"}
            ]}
        }))
        .unwrap();
        let files = s.attached();
        assert_eq!(files.len(), 2);
        assert_eq!((files[0].0.id.as_str(), files[0].1), ("o", 1));
        assert_eq!(files[1].2.source(), Some("https://y.test/2.mp4"));
    }

    #[test]
    fn the_limits_are_read_from_the_configuration_with_defaults() {
        let l = Limits::from_instance(&json!({"configuration": {
            "statuses": {"max_characters": 1000, "characters_reserved_per_url": 30},
            "media_attachments": {"image_size_limit": 16777216, "description_limit": 0}
        }}));
        assert_eq!(l.max_characters, 1000);
        assert_eq!(l.characters_reserved_per_url, 30);
        assert_eq!(l.max_media_attachments, 4);
        assert_eq!(l.description_limit, 1500);
        assert_eq!(l.image_size_limit, Some(16_777_216));
        assert_eq!(l.video_size_limit, None);
        assert_eq!(
            Limits::from_instance(&json!({"max_toot_chars": 5000})).max_characters,
            5000
        );
        assert_eq!(Limits::from_instance(&json!(null)), Limits::default());
    }
}

/// The current account's relationship with another account.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub(crate) struct Relationship {
    pub id: String,
    #[serde(default)]
    pub following: bool,
    #[serde(default)]
    pub requested: bool,
    #[serde(default)]
    pub muting: bool,
    #[serde(default)]
    pub blocking: bool,
}

/// A conversation and its latest direct status.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Conversation {
    pub id: String,
    pub unread: bool,
    pub accounts: Vec<Account>,
    pub last_status: Option<Status>,
}
