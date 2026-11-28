[![Build](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)

# mstdn

A [Mastodon](https://joinmastodon.org) client for the terminal: a full-screen client with timelines, direct conversations, account management and media, and commands for reading, posting, favourites, boosts and moderation. It lives in the same workspace as [msky](https://github.com/nao1215/social-terminal-clients/tree/main/crates/misskey-terminal-client) (Misskey) and [bsky](https://github.com/nao1215/social-terminal-clients/tree/main/crates/bluesky-terminal-client) (Bluesky).

```sh
mstdn                       # the full-screen client
mstdn author Gargron@mastodon.social -n 5
mstdn status https://mastodon.social/@Gargron/109372
mstdn timeline local --host fosstodon.org
mstdn search '#rust' --json | jq -r .url
mstdn login
mstdn post "hello" --image cat.jpg --alt "a cat"
```

Without a command, mstdn opens the full-screen client; each command prints its result and exits, as plain lines or as JSON.

## Demo

![federated accounts, a protocol walkthrough, a content warning, a poll and a boost](doc/demo/demo.gif)

From the repository root, record the fictional timeline on an X11 desktop:

```sh
./crates/mastodon-terminal-client/doc/record-demo.sh
```

The script builds `mstdn` and needs Python 3, kitty, xdotool, xwininfo and ffmpeg. It writes `doc/demo/demo.gif` inside this crate. The client and kitty use a temporary HOME and configuration, and the mock listens only on `127.0.0.1`. All media is served locally; no real account or public server is used. `Space` opens the picture and plays the H.264 video. Media sources are listed in [doc/demo/SOURCES.md](doc/demo/SOURCES.md).

Verify the demo in a PTY without an X11 display or a recording, with atago installed:

```sh
cargo build --locked -p mastodon-terminal-client
PATH="$PWD/target/debug:$PATH" atago run crates/mastodon-terminal-client/doc/verify-demo.atago.yaml
```

With `CARGO_TARGET_DIR` set, put its `debug` directory on PATH instead.

## Install

```sh
cargo install --locked --git https://github.com/nao1215/social-terminal-clients mastodon-terminal-client
```

Building it needs Rust 1.90 or later. It is not on crates.io.

## Full-screen client

`mstdn` opens six tabs: Home, Local, Federated, Notifications, Profile and Chat, as the account in use or, without one, as a visitor. Visitors can read Local, Federated, hashtag searches and public profiles. `--host` picks the server as it does for the commands.

| Key | What it does |
|---|---|
| `1`-`6`, `tab` | The tabs |
| `j` `k`, `g` `G`, `pgdn` `pgup` | Move; the next page loads near the end |
| `v` | The thread of the post, with its replies |
| `enter` | The selected post author or search account in Profile; `esc` returns |
| `/` | Open the search box; `ctrl+t` switches posts and accounts, `enter` searches, `esc` cancels |
| `esc` on search results | Return to the previous timeline with its selection and scroll position |
| `5` | The account in use and its posts |
| `e` on your own Profile | Edit display name, bio, four profile fields, locked, bot and discoverable; `tab` moves, `space` toggles flags, `ctrl+o` chooses avatar or header, `ctrl+x` clears a chosen picture, `ctrl+s` saves, `esc` cancels |
| `Q` | Quote the selected post when the server exposes the quote API; `ctrl+s` sends |
| `ctrl+q` in the composer | Cycle quote approval between the account default, anyone, followers and only yourself when the server exposes the quote API |
| `+` on Home | Add a list, followed or typed hashtag, Notifications or search beside Home |
| `H` `L`, left/right | Focus the previous or next column; `<` `>` move the column |
| `x` then `y` | Close the focused column; another key keeps it |
| `6`, `m` on a profile | Direct conversations, or a new direct message to that account |
| `enter`, `i` in Chat | Open a conversation and mark it read, then `i` writes and `enter` sends; `esc` stops writing or returns to the conversation list |
| `space` | Pictures and videos full screen; `h` `l` or arrows change files, `r` replays video, `esc` closes |
| `d` | Save the post files in the download folder |
| `A` | Saved accounts; `j` `k` select, `enter` switches, `a` logs in through browser OAuth, `x` then `y` removes the selected login; `esc` cancels |
| `s` | Settings: theme, language, download folder and browser; `enter` edits or chooses, `x` resets, `esc` cancels an edit |
| `O` | Open the selected post's link card in the browser after its content warning is opened |
| `l`, `b` | Favourite, boost, or take it back |
| `f` | Follow or unfollow the selected post author or the profile shown; also cancels a pending follow request |
| `M`, `B` | Toggle mute or block for that account; `y` confirms, any other key cancels |
| `D` | Delete your own selected post; `y` confirms |
| `!` | Report the selected post or an account without a selected post; `tab` edits an optional comment, `enter` asks for confirmation, `y` sends |
| `ctrl+o` in the composer | Browse for pictures or a video; `space` marks files, `enter` chooses, `h` goes up, `.` shows hidden files, `esc` returns |
| `tab`, `shift+tab` in the composer | Move between text, content warning and attachment alt text; `ctrl+x` removes the focused attachment or the last |
| `n`, `r` | A new post, a reply; `ctrl+s` sends, `ctrl+w` adds a content warning, `ctrl+p` changes who may see it |
| `w` | Show the text behind a content warning |
| `o`, `c` | Open the post in the browser, copy its address |
| `.` | What the keys do to the post |
| `T` | The theme, from 42 choices; `j`/`k` and `pgdn`/`pgup` preview, `enter` saves, `esc` restores the original |
| `R` | Refresh |
| `?`, `q` | The keys, quit |

Pictures and video thumbnails are drawn in the lists with kitty graphics, sixel or iTerm2 inline images, detected from the terminal. `space` opens the original pictures and plays MP4/H.264 videos without sound. Other codecs show a reason; `d` saves the files and `o` opens the post. On a terminal without graphics, the files and their alt text appear as text. A content warning hides its pictures until `w` opens it. Custom emoji in posts, names and notifications use their supplied pictures, with `:shortcode:` as the fallback on terminals without graphics.

Search opens a bordered box over the current list and keeps the last query. Enter replaces the timeline area with results under `Search: <query>`; Esc returns to the previous list without fetching it again. Ctrl+T in the box switches posts and accounts. Enter or `v` on an account opens its profile; Esc returns to the results. Search reads a hashtag timeline for visitors and, with a login, searches posts or accounts. Profile shows the account header and its posts, loading more near the end. Switching accounts clears the previous account's views and uses the new server and token for subsequent actions; work already sent finishes as its original account. `q` waits up to 3 seconds for writes still on their way.

Favourites, boosts, follows, mutes and blocks show their new state immediately and restore it if the server refuses the write. Account actions read the relationship before the first toggle; the resulting state appears in the status line, the profile and the actions menu. Mute, block, deletion and reports require `y`; any other answer sends nothing. Deletion removes the post from the lists only after the server accepts it. Reports go to the current server's moderators and are not forwarded to another server.

The composer attaches up to four pictures or one video with alt text, checked against the same instance limits as `mstdn post`. Files upload only on `ctrl+s`, with their metadata stripped, and the post waits for processing to finish. Cancelling the browser or closing the draft uploads nothing. A refused post keeps the draft for editing.

The thread of a selection resting for 400 ms is read ahead; `v` uses a fresh result or joins an in-flight read. Notifications refresh every minute and show the number newer than the latest ID viewed during this run, among the loaded notifications; opening the tab clears the count. This count is local to the run and does not change the server notification marker.

Profile editing reads the account's unformatted source and sends only changed attributes in one `PATCH /api/v1/accounts/update_credentials`. Avatar and header pictures use the shared file browser and have their metadata stripped before saving. Cancelling sends nothing, and a refused save keeps the draft. Login already requests `read write`, which covers profile editing and conversations; no scope change or new environment variable is needed.

Quote creation is detected from the documented `quote_approval` field on a successfully read status, without comparing software versions. Before sending, mstdn probes the status again; a standalone approval policy probes `source.quote_policy` on the credential account. Missing capabilities name the feature and server; network and permission failures retain their original errors. Received quotes are drawn regardless of version, with a placeholder for quotes awaiting approval or unavailable to the reader. The server enforces the quoted post's policy; a known denial is refused locally.

Columns keep their own selection and pagination and are saved per account in `settings.json`. Chat reads `/api/v1/conversations` and the context of its latest direct status, refreshing the visible conversation or list every 15 seconds. Only direct statuses with the same participants appear in an open conversation, replies keep direct visibility, and the conversation is marked read after Enter opens it and the messages load. A refused read restores the unread flag; a refused send keeps the input.

Settings are kept in `settings.json`. The download folder and browser can be edited with `s`; nonempty `MSTDN_DOWNLOAD_DIR` and `MSTDN_BROWSER` override these saved choices for the run. The language setting takes precedence over `LC_ALL`, `LC_MESSAGES` and `LANG`, in that order, with English as the fallback. Unknown settings are preserved; an unreadable settings file is left untouched. Pictures are cached on disk up to 256 MB and in memory; `MSTDN_CACHE_DIR=off` disables the disk cache.

## Commands

`mstdn author alice` prints a post like this:

```
Alice @alice · 2026-11-26 19:54
山の写真 🏔️ #photo https://example.com/a/long/address
[2 pictures]
♡ 19  ⟳ 3  ↩ 1
https://mastodon.social/@alice/113562
```

| Command | What it does |
|---|---|
| `timeline [KIND]` (`tl`) | `home` (default), `local` or `public` (`federated`); `--reposts` keeps the boosts |
| `notifications` (`notif`) | Your notifications; `--seen` saves the newest displayed notification as read |
| `author ACTOR` | An account's posts; `--filter with-replies\|no-replies\|media\|video\|threads`, `--reposts`, `--download` |
| `status POST` (`note`), `thread POST` | One post; the post with what it answers and its replies |
| `search QUERY` | A hashtag's posts or full-text search results; `--accounts` finds accounts |
| `profile [ACTOR]`, `whoami` | An account; the one logged in |
| `post TEXT` | A post: `--image` (up to 4), `--video`, `--alt`, `--reply`, `--quote` (4.5+), `--cw`, `--sensitive`, `--visibility`, `--dry-run` |
| `download POST` | Save a post's files (`-o DIR`) |
| `like POST`, `unlike POST` (`favourite`, `unfavourite`) | Favourite a post or take it back |
| `repost POST`, `unrepost POST` (`boost`, `unboost`) | Boost a post or take it back |
| `delete POST` | Delete your own post |
| `likes POST`, `reposts POST` | Accounts that favourited or boosted a post (`-n`) |
| `follow ACTOR`, `unfollow ACTOR` | Follow, unfollow, or cancel a pending request |
| `followers [ACTOR]`, `follows [ACTOR]` (`following`) | Followers or following (`-n`); `followers --pending` lists incoming requests |
| `mute ACTOR`, `unmute ACTOR`, `mutes` | Mute, unmute, or list muted accounts (`-n`) |
| `block ACTOR`, `unblock ACTOR`, `blocks` | Block, unblock, or list blocked accounts (`-n`) |
| `favourites`, `bookmarks` | Your favourite or bookmarked posts (`-n`) |
| `bookmark POST`, `unbookmark POST` | Save or remove a bookmark |
| `lists`, `list ID` | Your list IDs and names, or a list's timeline (`-n`) |
| `report TARGET --reason REASON` | Report a post ID or account; `--comment`, `--yes/-y`; reasons: `spam`, `violation`, `misleading`, `sexual`, `rude`, `other` |
| `chat [ACTOR] [TEXT]` | Direct conversation summaries, available one-to-one history, or send text (`-` reads stdin); `-n` limits results |
| `login`, `logout`, `accounts` | Log in with OAuth (`--oob`), log out (`--all`), list or switch accounts (`--use HOST`) |

An account is `name`, `@name@host` or its address; a post is its ID or its address (`https://host/@name/ID`, `https://host/users/name/statuses/ID`). An address is read on its own server. `--host` (or `MSTDN_HOST`) picks the server, else the server of the account in use, else mastodon.social. `-n` sets how many to print (20 for posts and chat, 50 for account lists and saved posts); mstdn reads up to 40 a page, the next page where the server's `Link` header says.

Mastodon sends a post's text as HTML. mstdn prints the text it reads as: a blank line between paragraphs, a mention or a hashtag as written, a link's address whole, and `the article (https://example.com/a)` for a link whose text is not its address. Custom emoji stay `:shortcode:`.

Without a login, Mastodon's full-text search finds nothing, so `mstdn search` asks for one, except for a hashtag (`'#rust'`), which it reads from the hashtag's timeline. With a login, the server finds only what it lets you search (on mastodon.social: your own posts, those you interacted with, and those of accounts that opted in).

`author` asks the server for an account of another server it does not know yet only with a login, as Mastodon's search does; without one, read it on its own server with `--host`.

## Logging in

`mstdn login` registers mstdn on the server (once; the client ID and secret are kept in `apps/<host>.json`), prints the server's page that asks for the access, and opens it in the browser. Allow it there; the browser is sent back to mstdn on `http://127.0.0.1:<port>/callback`. With `--oob` the page shows a code to paste instead, for a browser on another machine. mstdn asks for `read write`, uses PKCE, and never sees the password.

The token is saved per server in `accounts/<host>.json` in the config folder, readable by you alone (mode 0600), and goes only to that server. `mstdn logout` removes it; the access stays listed in the server's settings (Authorized apps), where it can be revoked.

For a job with no one at the keyboard, set `MSTDN_TOKEN` to an access token made in Preferences, Development, with `read` and `write`. mstdn uses it on `MSTDN_HOST` only and never prints it.

```yaml
- name: Post
  env:
    MSTDN_HOST: mastodon.social
    MSTDN_TOKEN: ${{ secrets.MSTDN_TOKEN }}
  run: mstdn post "New article https://debimate.jp/post/a/"
```

## Posting

`mstdn post` uploads the files, waits until the server has processed them, makes the post, and prints its address. Before sending, it checks what it can against the server's own limits (`/api/v2/instance`): the length as Mastodon counts it (an address as 23 characters, `@name@host` as `@name`, the content warning with the text), four pictures or one video, the alt texts, and the file sizes. A post is sent once, with an `Idempotency-Key`. Photos and videos go up without their metadata (EXIF and GPS, XMP, a video's user data and location).

`--visibility` takes `public`, `unlisted`, `private` or `direct` (`home` and `followers`, Misskey's names for `unlisted` and `private`, too). `--reply` takes a post of another server by its address, and the server fetches it first. The server makes the link card itself, so there is no `--no-card`. `--dry-run` prints what would be sent and sends nothing.

## Saving files

`mstdn download POST` and `mstdn author ACTOR --download` save each file as `<post ID>-<n>.<ext>` in `-o`, `MSTDN_DOWNLOAD_DIR`, or `mstdn` in your Downloads folder. mstdn never writes over a file, names the extension after the type the file is served as, and skips files over 200 MB.

## JSON

`--json` prints one post a line, in the shape bsky and msky print too ([doc/json.md](https://github.com/nao1215/social-terminal-clients/blob/main/doc/json.md) lists every key):

```json
{"content":"<p>山の写真 🏔️</p>","createdAt":"2026-11-26T10:54:49.672Z","cw":null,"files":[{"description":"a mountain","id":"113","mime":null,"previewUrl":"https://files.example/small/a.jpg","type":"image","url":"https://files.example/original/a.jpg"}],"id":"113562","language":"ja","likeCount":19,"mentions":[],"poll":null,"quoteId":null,"repliesCount":1,"replyId":null,"repostCount":3,"repostOf":null,"sensitive":false,"tags":[],"text":"山の写真 🏔️","uri":"https://mastodon.social/users/alice/statuses/113562","url":"https://mastodon.social/@alice/113562","user":{"host":"mastodon.social","id":"1","name":"Alice","username":"alice"},"visibility":"public"}
```

`text` is the HTML read as text and `content` the HTML itself; `likeCount` counts the favourites and `repostCount` the boosts, and a boost holds the post it boosts in `repostOf`. `notifications` prints `{"id", "createdAt", "type", "user", "postId", "post"}`, a favourite as the `type` `like` and a boost as `repost`; `profile` and `whoami` an account; `post` `{"action", "target", "id", "url", "already", "uri"}`; and `download` one line a file, `{"postId", "type", "url", "path"}`. An error is `{"error", "hint", "status"}`, on stdout as well as stderr.

When a server says it is busy (429, 502, 503, 504), mstdn reads up to three more times, after its `Retry-After` or `X-RateLimit-Reset`, or after 1, 2 and 4 seconds. A write is never sent again.

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: a wrong flag or argument, no login where one is needed, a post the server would not take |
| 2 | The full-screen client was started without a terminal, or the terminal failed |
| 3 | A file could not be read or written |
| 4 | The server could not be reached or said no, including authentication and permission failures |
| 5 | The server does not offer the requested feature; the message names the feature and server |

HTTP 501 returns status 5. A 404 on a resource-free endpoint can establish that the feature is absent; optional routes that may hide resources additionally require evidence from an older Mastodon release. Compatibility version strings are not interpreted as Mastodon releases. Missing resources, authentication failures and uncertain checks remain status 4. Quote capability is checked before uploads or posting, including `--dry-run`.

## Environment

| Variable | Meaning |
|---|---|
| `MSTDN_HOST` | Server to read (also `--host`) |
| `MSTDN_TOKEN` | Access token in place of the saved login, on `MSTDN_HOST` only |
| `MSTDN_CONFIG_DIR` | Config folder |
| `MSTDN_DOWNLOAD_DIR` | Where `download` and `d` save |
| `MSTDN_CACHE_DIR` | Picture cache folder, else `mstdn` in the platform cache folder; `off` disables it |
| `MSTDN_GRAPHICS` | Force `kitty`, `sixel` or `iterm2` when the terminal supports it but reports another protocol |
| `MSTDN_BROWSER` | Program that opens login pages, posts and link cards; overrides the saved browser |
| `LC_ALL` `LC_MESSAGES` `LANG` | English or 日本語; the full-screen language when settings name none, and the login directions |
| `NO_COLOR` | The full-screen client in `monochrome` |
| `TZ` | Time zone of the times shown |

## Testing

| Technique | Coverage |
|---|---|
| Unit and property tests | HTML, pagination, output and full-screen state; nightly properties use 20,000 cases |
| Fuzzing | HTML-to-text, status JSON and Link headers |
| Mutation tests | HTML, single-line text and full-screen lists |
| Benchmarks | Start-up, 100-status commands and the first post on screen |

`link_header` builds the `Next`, `next_of`, `percent_decode` and `percent_encode` items directly from `src/api/mod.rs`; the harness keeps no second parser implementation.

From this crate's directory:

```sh
just bench                                  # himorime, Python 3, atago
just fuzz link_header -max_total_time=60
just properties                             # 20,000 cases per property
just mutants                                # changes since main
```

[Benchmarks](bench/README.md) run against local stand-ins. Fuzz targets run over their committed seeds on pull requests and explore nightly for 300 seconds each; local exploration writes new inputs to `fuzz/work/`. Mutation testing reports changed lines on pull requests and the selected source files weekly, without a score gate.

## Contributing

```sh
cargo test -p mastodon-terminal-client
just e2e-mstdn    # end-to-end tests (atago)
```

The unit and existing E2E suites send their writes to local stand-ins or atago mocks. The separate [live compatibility suite](../../doc/compat.md) starts disposable GoToSocial or Mastodon servers on `127.0.0.1`, seeds throwaway accounts and removes their data afterwards.

## License

[MIT](LICENSE). Not made by or affiliated with the Mastodon project.

Notification unread counts use the server's notification marker and capped unread count (Mastodon 4.3+). Viewing the Notifications tab saves its newest loaded ID. A refused save restores the unread count. Servers without the count endpoint use loaded notifications relative to the marker; if markers cannot be read, the client retains its local count. Link cards use metadata and thumbnails returned by Mastodon, and stay hidden behind content warnings.

Direct posts are addressed to mentioned accounts and are not end-to-end encrypted. `chat ACTOR` reads the available direct contexts for one-to-one conversations; group conversations remain in `chat` summaries. Sending text containing additional mentions can add recipients. CLI history reading leaves conversation read state unchanged. Reports are sent to the current server with `forward=false`; interactive confirmation or `--yes` is required. Logout removes the local saved login; revoke OAuth access in the server's authorized applications settings if needed.

Mastodon has no standard arbitrary emoji reactions, per-post link-card suppression, outgoing follow-request listing, Misskey social timeline or Bluesky feed-generator API. Those commands and flags are omitted. Incoming requests are available through `followers --pending`; `unfollow` cancels an outgoing request when its account is known. Quote posts require Mastodon 4.5 or later.
