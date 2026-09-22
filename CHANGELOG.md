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
