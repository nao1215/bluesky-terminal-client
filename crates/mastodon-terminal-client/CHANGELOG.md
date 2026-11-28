# Changelog

All notable changes to mstdn (the mastodon-terminal-client crate) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `mstdn`, a Mastodon client for the terminal made of commands that read timelines, posts and accounts, save their files, and post.
- `mstdn login` logs in with OAuth: it registers mstdn on the server once (`POST /api/v1/apps`, kept in `apps/<host>.json`), opens the authorization page, and takes the code the browser brings back to a listener on 127.0.0.1, or, with `--oob`, the code pasted from the page. The code is traded with a PKCE verifier and the token is saved per server in `accounts/<host>.json` (mode 0600), never printed. `mstdn accounts` lists and switches the accounts, `mstdn logout` removes one or `--all`, and `MSTDN_TOKEN` takes a token made in the server's settings for CI jobs.
- `mstdn author`, `status` (`note`), `thread`, `profile` and `whoami` read accounts and posts, as a visitor where the server allows it; `author` takes bsky's `--filter` names and `--reposts`, and finds an account of another server with a search when logged in.
- `mstdn timeline` reads the home, local and public (federated) timelines, a page of up to 40 at a time where the `Link` header says the next one starts, and `mstdn notifications` the account's notifications.
- `mstdn search` reads a hashtag's timeline as a visitor too, and, logged in, the server's full-text search; without a login a word search is refused before anything is asked, since Mastodon answers it with nothing.
- `mstdn download` and `author --download` save a post's files (the boosted post's for a boost) as `<post ID>-<n>.<ext>`, the extension named after the type the file is served as, never over a file already there.
- `mstdn post` posts text with up to four pictures or a video, alt text, a content warning, `--sensitive`, `--visibility` and `--reply` (a post of another server is fetched first), checked against the server's own limits from `/api/v2/instance` and counted as Mastodon counts (an address as 23 characters, a remote mention as its name). Files go up without their metadata (msky's EXIF, XMP and MP4 stripping), the post waits for them to be processed, and the status is sent once with an `Idempotency-Key`. `--dry-run` sends nothing.
- A status's HTML is read back into its text for the terminal and for `--json`: paragraphs, line breaks, mentions and hashtags as written, links whole (the address after the text when the text is not it), custom emoji shortcodes, quotes, lists and preformatted text, with unit and property tests.
- A read turned away for being asked too often waits as `Retry-After` or `X-RateLimit-Reset` says, up to three times; a write is never sent again.
- The login's directions and the page the browser comes back to are in English or Japanese, by the locale.
- `--json` prints one object a line in the shape bsky and msky print (`doc/json.md`): a post's favourites are `likeCount` and its boosts `repostCount`, a boost holds the post it boosts in `repostOf`, a file's `type` is `image`, `video`, `audio` or `file`, a notification names its post in `postId` and `post`, and a write prints `{"action", "target", "id", "url", "already"}`.
- `-n 0` is a usage error (exit 1), as in bsky and msky.
