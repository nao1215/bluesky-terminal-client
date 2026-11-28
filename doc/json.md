# The JSON the commands print

With `--json`, bsky, msky and mstdn print one JSON object per line: one per post, account or notification a command lists, or one for what a command did. The three print the same keys for the same things, so a script written for one reads the others. A key is always there; one with nothing to say is `null` (an empty list for `files`). What only one network has comes as more keys beside the shared ones.

Every key is camelCase. Times are the server's own ISO 8601 strings. A list is read page by page until `-n` items have come; no cursor is printed, so every line is one item.

`doc/json_keys.rs` holds the key lists below, and each client's tests include it and check what they print against them.

## Post

| Key | What it is |
|---|---|
| `id` | What the other commands take for this post: the at:// URI on Bluesky, the note ID on Misskey, the status ID on Mastodon |
| `url` | The post's page on the web, on the server asked |
| `uri` | The post's own address: the at:// URI on Bluesky, the ActivityPub URI on Misskey (`null` for a note of the server asked) and Mastodon |
| `createdAt` | When it was written |
| `user` | Who wrote it, a [user](#user) |
| `text` | Its text, as text (Mastodon's HTML is read out to text) |
| `cw` | Its content warning, or `null` |
| `likeCount` | Likes; Misskey's reactions of every kind, Mastodon's favourites |
| `repostCount` | Reposts; Misskey's renotes, Mastodon's boosts |
| `repliesCount` | Replies |
| `files` | Its pictures, videos and other files, each a [file](#file) |
| `replyId` | The `id` of the post it answers, or `null` |
| `quoteId` | The `id` of the post it quotes, or `null` |
| `repostOf` | For a repost, the post reposted, whole; else `null`. A repost has no text or files of its own |

More keys:

- bsky: `cid`, `quoteCount`, `langs`.
- msky: `reactions` (each reaction with how many gave it), `visibility`.
- mstdn: `content` (the HTML), `sensitive`, `visibility`, `language`, `mentions`, `tags`, `poll`.

## User

The `user` of a post or a notification.

| Key | What it is |
|---|---|
| `id` | What the other commands take for the account: the DID on Bluesky, the user ID on Misskey and Mastodon |
| `username` | The handle without `@`: `alice.bsky.social`, `alice` |
| `host` | The server the account is on, the server asked included; `null` on Bluesky, where the handle is a whole domain |
| `name` | The display name, or `null` |

## Account

What `whoami`, `profile` and the commands that list accounts (`followers`, `follows`, a search for accounts...) print: a [user](#user) and

| Key | What it is |
|---|---|
| `url` | The account's page on the web |
| `description` | What it says about itself, as text |
| `followersCount`, `followingCount`, `postsCount` | Its counts, or `null` when the server does not say |
| `following` | Whether you follow it, or `null` when the server does not say |

More keys: msky and mstdn `locked`, mstdn `bot`.

The commands that list who liked or reposted a post, or who is on a list, print accounts too, with bsky's `likedAt` and msky's `reaction` and `reactedAt` beside.

## File

| Key | What it is |
|---|---|
| `id` | The file's ID on the server, or `null` |
| `type` | `image`, `video`, `audio` or `file`; a GIF that Mastodon made a video is `video` |
| `mime` | Its media type, or `null` when the server does not say (Bluesky and Mastodon do not) |
| `url` | Where it is; a Bluesky video's HLS playlist |
| `previewUrl` | A smaller picture of it, or `null` |
| `description` | Its alt text, or `null` |

More keys: msky `name`, `size`, `isSensitive`.

## Notification

| Key | What it is |
|---|---|
| `id` | The notification's ID; the at:// URI of what caused it on Bluesky |
| `createdAt` | When it came |
| `type` | `like`, `repost`, `follow`, `mention`, `reply`, `quote`, or the server's own word for anything else |
| `user` | Who did it, a [user](#user), or `null` |
| `postId` | The `id` of the post it is about, or `null` |
| `post` | That post, whole, when the server sent it; else `null` |

A Misskey reaction is a `like`, with the reaction in `reaction`. Mastodon's `favourite` is a `like`, its `reblog` a `repost`. A Bluesky notification carries the post of a reply, a mention or a quote without its counts, so they are `null` there; a like or a repost names the post in `postId` alone. More keys: bsky `isRead`, msky `reaction`.

## Thread

`thread` prints a post per line: the posts above the one asked for, oldest first, then it, then its replies, each before its own replies.

## Message

A direct message (bsky and msky `chat`):

| Key | What it is |
|---|---|
| `id` | The message's ID |
| `createdAt` | When it was sent |
| `user` | Who sent it, a [user](#user), or `null` |
| `text` | Its text, or `null` for one deleted |

More keys: bsky `convoId`; msky `to`, `room`, `file`. `chat ACTOR TEXT` prints the message it sent, and bsky's `chat` without ACTOR prints each conversation: `id`, `members` (users), `unreadCount`, `muted` and `lastMessage`.

## Logins

`accounts` prints each account logged in, `login` the one it logged in:

| Key | What it is |
|---|---|
| `id`, `username`, `host` | As a [user](#user) |
| `service` | The server it logs in to |
| `current` | Whether it is the one in use (`accounts` only) |

`logout` prints `{"loggedOut": [...]}`, each account with `id`, `username` and `host`.

## Saved files

`download`, and `author --download`, print each file saved:

| Key | What it is |
|---|---|
| `postId` | The `id` of the post it is from |
| `type` | As a [file](#file)'s |
| `url` | Where it was read from |
| `path` | Where it was saved |

## What a command did

A write prints one object:

| Key | What it is |
|---|---|
| `action` | The command: `post`, `like`, `unlike`, `repost`, `unrepost`, `follow`, `unfollow`, `mute`, `unmute`, `block`, `unblock`, `delete`, `report`... |
| `target` | The `id` of the post or account it was done to; `null` for a new post |
| `id` | The `id` of what it made (the new post; on Bluesky the like, repost, follow or block record), or `null` |
| `url` | The web page of what it made, or `null` |
| `already` | `true` when there was nothing to do: liked already, following already |

More keys: bsky `cid`, and for `app-passwords add` the `password`; msky `reaction`, `requested` (a follow the account has to accept), and a report's `comment`; mstdn `uri`.

`post --dry-run --json` prints what would be sent and sends nothing: `{"dryRun": true, "host", "username", "post": {"text", "cw", "visibility", "replyId", "files"}, ...}`; msky adds the quote's `quoteId` and the link preview the server would make.

## Errors

A command that fails prints `{"error", "hint", "status"}` to stdout as well as the error to stderr, and exits with `status`: 1 a usage error, 2 no terminal to draw in, 3 a local file, 4 the network or the server.

## Before this shape

bsky 0.15.0 and earlier printed the server's own objects. `bsky --raw` still prints them for what it reads, in place of the shape above (a write prints the object above with `--raw` too), and `thread --raw` the thread as one object. msky 0.1.1 and earlier printed this shape with Misskey's names: `reactionCount` is `likeCount` now, `renoteCount` `repostCount`, `renoteId` and `renote` are `quoteId` for a quote and `repostOf` for a renote; a file's `type` is its `mime`; a notification's `note` is `post`, with `postId`; `notesCount` is `postsCount`. A write printed `{"uri", "id"}`, `{"deleted"}` and the like, and prints the object above now. `download` printed `{"note", "kind", "url", "path"}`, and a dry run's `note` is `post`. A chat message's sender was `from`; it is `user`.
