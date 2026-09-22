# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `bs`, a Bluesky client for the terminal. The timeline shows posts written by accounts you follow and nothing else: reposts, your own posts, and posts by accounts you do not follow are left out. Avatars and attached photos are drawn inline with kitty graphics, sixel, or iTerm2 inline images, each at its own aspect ratio.
- Posting, replying (the reply keeps the thread's root and names the post it answers), and liking. New posts carry link, mention, and hashtag facets at UTF-8 byte offsets, and the composer enforces Bluesky's 300-character limit before sending.
- Search for posts and for accounts, following and unfollowing from the results, from a post's author, or from a profile, and opening any account's profile with its recent posts.
- A profile editor for the display name, description, and avatar. Saving keeps every other field of the profile record and is guarded with `swapRecord` against the version the editor loaded, so an edit made elsewhere in the meantime is refused rather than overwritten.
- Login with an app password. Only the session tokens are stored, in `session.json` with owner-only permissions; an expired access token is refreshed once and the new tokens are saved. `bs logout` removes the session.
- A terminal without an image protocol is refused at startup with exit status 2; `BS_GRAPHICS` forces a protocol for terminals that support one but do not answer the query. Usage errors exit 1 and a session file that cannot be read or parsed exits 3, each with an `error:` line and, where there is a next step, a `hint:`.
- Replies on the timeline are shown under the posts they answer: the parent, and the thread's first post with a gap marker when the reply sits deeper. `v` opens a post's thread: the posts above it, the post, and every reply expanded with indentation, with placeholders for deleted or blocked posts. Like, repost, reply, and follow work on any post in it.
- A Notifications tab (`4`) shows likes, reposts, follows, mentions, replies, and quotes with the posts they are about, counts the unread ones on the tab, and marks them seen as of the time they were loaded. Replies, mentions, and quotes can be answered and liked from it.
- Repost and remove a repost with `b`.
- Lists load their next page as the selection nears the end: the timeline, search results, a profile's posts, and notifications. A page is never loaded twice, repeated posts are skipped, and a page for a list that has since been refreshed is dropped.
- Nine color themes (`default`, `light`, `dracula`, `nord`, `gruvbox`, `solarized`, `catppuccin`, `tokyo-night`, `monochrome`) in a picker opened with `T`, previewed live and saved in `settings.json`. 24-bit themes are mapped to 256 colors on terminals without 24-bit color, and `NO_COLOR` turns color off.
- A hint row under every view with the keys that work there, and a `?` help grouped by view that scrolls and closes only with Esc, `q`, or `?`.
- Esc on a profile opened from the search results, the timeline, or the notifications goes back there, with the results and the selection as they were.

### Fixed

- Typing on the Search tab ran the letters as commands (`q` quit, `l` liked a result) unless `/` was pressed first. Arriving at an empty Search tab now puts the cursor in the box, and `Tab` leaves it.
- A status message such as "liked" replaced the key hints until the next message. Messages now have their own row and clear after a few seconds.
