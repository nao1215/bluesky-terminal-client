# mstdn

A [Mastodon](https://joinmastodon.org) client for the terminal: commands that read timelines, posts and accounts, save their files, and post with pictures. It lives in the same workspace as [msky](https://github.com/nao1215/social-terminal-clients/tree/main/crates/misskey-terminal-client) (Misskey) and [bsky](https://github.com/nao1215/social-terminal-clients/tree/main/crates/bluesky-terminal-client) (Bluesky).

```sh
mstdn author Gargron@mastodon.social -n 5
mstdn status https://mastodon.social/@Gargron/109372
mstdn timeline local --host fosstodon.org
mstdn search '#rust' --json | jq -r .url
mstdn login
mstdn post "hello" --image cat.jpg --alt "a cat"
```

mstdn is commands only: each one prints its result and exits, as plain lines or as JSON.

## Install

```sh
cargo install --locked --git https://github.com/nao1215/social-terminal-clients mastodon-terminal-client
```

Building it needs Rust 1.90 or later. It is not on crates.io.

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
| `notifications` (`notif`) | Your notifications |
| `author ACTOR` | An account's posts; `--filter with-replies\|no-replies\|media\|video\|threads`, `--reposts`, `--download` |
| `status POST` (`note`), `thread POST` | One post; the post with what it answers and its replies |
| `search QUERY` | A hashtag's posts, or the posts the server's full-text search finds |
| `profile [ACTOR]`, `whoami` | An account; the one logged in |
| `post TEXT` | A post: `--image` (up to 4), `--video`, `--alt`, `--reply`, `--cw`, `--sensitive`, `--visibility`, `--dry-run` |
| `download POST` | Save a post's files (`-o DIR`) |
| `login`, `logout`, `accounts` | Log in with OAuth (`--oob`), log out (`--all`), list or switch accounts (`--use HOST`) |

An account is `name`, `@name@host` or its address; a post is its ID or its address (`https://host/@name/ID`, `https://host/users/name/statuses/ID`). An address is read on its own server. `--host` (or `MSTDN_HOST`) picks the server, else the server of the account in use, else mastodon.social. `-n` sets how many to print (20 by default); mstdn reads up to 40 a page, the next page where the server's `Link` header says.

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
| 3 | A file could not be read or written |
| 4 | The server could not be reached or said no |

## Environment

| Variable | Meaning |
|---|---|
| `MSTDN_HOST` | Server to read (also `--host`) |
| `MSTDN_TOKEN` | Access token in place of the saved login, on `MSTDN_HOST` only |
| `MSTDN_CONFIG_DIR` | Config folder |
| `MSTDN_DOWNLOAD_DIR` | Where `download` saves |
| `MSTDN_BROWSER` | Program that opens the login page |
| `LC_ALL` `LC_MESSAGES` `LANG` | Language of the login's directions: English or 日本語 |
| `TZ` | Time zone of the times shown |

## Contributing

```sh
cargo test -p mastodon-terminal-client
just e2e-mstdn    # end-to-end tests (atago)
```

Every write the tests make (the login's token exchange, uploads, posts) goes to a stand-in server or an atago mock, never to a real one.

## License

[MIT](LICENSE). Not made by or affiliated with the Mastodon project.
