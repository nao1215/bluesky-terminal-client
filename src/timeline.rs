//! The "following only" rule for the home timeline.
//!
//! `app.bsky.feed.getTimeline` mixes in the viewer's own posts and reposts,
//! which bring in posts by accounts the viewer does not follow. bs shows a
//! post only when its author is someone the viewer follows and it is in the
//! feed because that author wrote it, not because somebody reposted it.

use crate::api::types::{FeedItem, Post};

/// Keep only posts written by accounts `viewer_did` follows.
pub fn followed_posts(feed: Vec<FeedItem>, viewer_did: &str) -> Vec<Post> {
    feed.into_iter()
        .filter(|item| item.reason.is_none())
        .map(|item| item.post)
        .filter(|post| post.author.did != viewer_did && post.author.following_uri().is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(author: &str, following: bool, repost: bool) -> FeedItem {
        let mut v = json!({
            "post": {
                "uri": format!("at://{author}/app.bsky.feed.post/1"),
                "cid": "c",
                "author": {"did": author, "handle": format!("{author}.test")},
                "record": {"text": format!("by {author}")},
            }
        });
        if following {
            v["post"]["author"]["viewer"] = json!({"following": "at://me/app.bsky.graph.follow/x"});
        }
        if repost {
            v["reason"] =
                json!({"$type": "app.bsky.feed.defs#reasonRepost", "by": {"did": "friend"}});
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn keeps_only_original_posts_by_followed_accounts() {
        let feed = vec![
            item("friend", true, false),
            item("stranger", false, false),
            item("stranger", false, true),
            item("friend2", true, true),
            item("me", false, false),
            item("friend2", true, false),
        ];
        let authors: Vec<String> = followed_posts(feed, "me")
            .into_iter()
            .map(|p| p.author.did)
            .collect();
        assert_eq!(authors, ["friend", "friend2"]);
    }

    #[test]
    fn own_posts_are_dropped_even_with_a_following_viewer_state() {
        assert!(followed_posts(vec![item("me", true, false)], "me").is_empty());
    }
}
