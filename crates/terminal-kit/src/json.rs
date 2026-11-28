//! The keys every client prints with `--json`, as `doc/json.md` lists them.
//!
//! A client may print more keys beside these (what only its network has),
//! never fewer: each client's tests check what it prints against these
//! lists, so the three stay one shape.

/// A post.
pub const POST: &[&str] = &[
    "id",
    "url",
    "uri",
    "createdAt",
    "user",
    "text",
    "cw",
    "likeCount",
    "repostCount",
    "repliesCount",
    "files",
    "replyId",
    "quoteId",
    "repostOf",
];

/// The `user` of a post, a notification or a message.
pub const USER: &[&str] = &["id", "username", "host", "name"];

/// An account: a [`USER`] and these.
pub const ACCOUNT: &[&str] = &[
    "id",
    "username",
    "host",
    "name",
    "url",
    "description",
    "followersCount",
    "followingCount",
    "postsCount",
    "following",
];

/// A file of a post.
pub const FILE: &[&str] = &["id", "type", "mime", "url", "previewUrl", "description"];

/// What a file's `type` can be.
pub const FILE_TYPES: &[&str] = &["image", "video", "audio", "file"];

/// A notification.
pub const NOTIFICATION: &[&str] = &["id", "createdAt", "type", "user", "postId", "post"];

/// The `type`s of a notification every network has; another is the
/// server's own word.
pub const NOTIFICATION_TYPES: &[&str] = &["like", "repost", "follow", "mention", "reply", "quote"];

/// A direct message.
pub const MESSAGE: &[&str] = &["id", "createdAt", "user", "text"];

/// An account logged in, as `accounts` and `login` print it.
pub const LOGIN: &[&str] = &["id", "username", "host", "service"];

/// A file `download` saved.
pub const SAVED: &[&str] = &["postId", "type", "url", "path"];

/// What a write did.
pub const WROTE: &[&str] = &["action", "target", "id", "url", "already"];

/// The keys of `keys` that `have` lacks: what a printed object must still
/// gain to be the shape. `have` is the object's keys.
pub fn missing<'a>(keys: &[&'a str], have: &[&str]) -> Vec<&'a str> {
    keys.iter().copied().filter(|k| !have.contains(k)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_names_the_keys_an_object_lacks() {
        assert_eq!(
            missing(USER, &["id", "name", "extra"]),
            ["username", "host"]
        );
        assert!(missing(USER, USER).is_empty());
    }

    // An account is a user and more: a script that reads a post's `user`
    // reads an account the same way, and a login names its account by the
    // same keys.
    #[test]
    fn an_account_is_a_user_and_more() {
        assert!(missing(USER, ACCOUNT).is_empty());
        assert!(missing(&["id", "username", "host"], LOGIN).is_empty());
    }

    // No list names a key twice, and every key is camelCase.
    #[test]
    fn the_keys_are_camel_case_and_each_once() {
        for list in [
            POST,
            USER,
            ACCOUNT,
            FILE,
            NOTIFICATION,
            MESSAGE,
            LOGIN,
            SAVED,
            WROTE,
        ] {
            for (i, k) in list.iter().enumerate() {
                assert!(!list[..i].contains(k), "{k} twice");
                assert!(
                    k.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        && k.chars().all(|c| c.is_ascii_alphanumeric()),
                    "{k}"
                );
            }
        }
    }
}
