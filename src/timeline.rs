//! The "following only" rule for the home timeline.
//!
//! `app.bsky.feed.getTimeline` mixes in reposts, which bring in posts by
//! accounts the viewer does not follow. bsky shows a post only when it is in
//! the feed because its author wrote it, not because somebody reposted it,
//! and its author is someone the viewer follows or the viewer.

use crate::api::types::{FeedItem, Post, ReplyContext};

/// Keep only posts written by `viewer_did` or by accounts it follows.
///
/// A reply keeps the thread above it ([`Post::context`]), whoever wrote those
/// posts: the rule is about who wrote the item, and the context is what makes
/// a followed account's reply readable.
pub fn followed_posts(feed: Vec<FeedItem>, viewer_did: &str) -> Vec<Post> {
    feed.into_iter()
        .filter(|item| item.reason.is_none())
        .filter(|item| {
            item.post.author.did == viewer_did || item.post.author.following_uri().is_some()
        })
        .map(|item| {
            let mut post = item.post;
            post.context = item.reply.map(|r| Box::new(ReplyContext::from_reply(r)));
            post
        })
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
    fn keeps_only_original_posts_by_followed_accounts_and_you() {
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
        assert_eq!(authors, ["friend", "me", "friend2"]);
    }

    #[test]
    fn your_own_repost_of_a_stranger_is_still_left_out() {
        assert!(followed_posts(vec![item("stranger", false, true)], "me").is_empty());
        assert_eq!(
            followed_posts(vec![item("me", false, false)], "me").len(),
            1
        );
    }
}
