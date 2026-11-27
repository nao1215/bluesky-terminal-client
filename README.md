[![Build](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# msky

An unofficial [Misskey](https://misskey-hub.net) client for the terminal, built as [bsky](https://github.com/nao1215/bluesky-terminal-client) is for Bluesky. `msky` alone opens a full-screen client: the home, local, social and global timelines, threads and notifications, with avatars and pictures drawn in the terminal (kitty graphics, sixel, or iTerm2 inline images). The commands read what a Misskey server shows to visitors: an account's notes, one note, a search, and the files attached to notes. Logged in with MiAuth, or given an access token in `MSKY_TOKEN`, they read your timelines and notifications, post notes with pictures and videos, and follow and unfollow accounts.

msky is for Misskey itself; a fork (Sharkey and the like) may work, but it is not looked after.

```sh
msky
msky login
msky timeline -n 10
msky notifications --json | jq -r .type
msky author syuilo -n 5
msky note https://misskey.io/notes/au102erst5tf04vs
msky search Misskey --json | jq -r .url
msky download https://misskey.io/notes/au102erst5tf04vs -o pictures
msky author syuilo -n 50 --filter media --download
msky post "hello" --image cat.jpg --alt "a cat"
msky post --dry-run "New article https://debimate.jp/post/a/"
msky follow @syuilo@misskey.io
```

A note as `msky author alice` prints it:

```
Alice @alice · 2026-11-26 19:54
山の写真 🏔️
[2 pictures]
:igyo: 12  👍 5  :blobcat@misskey.design: 2
♡ 19  ⟳ 3  ↩ 1
https://misskey.io/notes/9xyz0abc12
```

## Install

```sh
git clone https://github.com/nao1215/misskey-terminal-client
cd misskey-terminal-client
cargo install --locked --path .
```

It needs Rust 1.90 or later.

## The client

`msky` with no command opens the client on the server of the account in use (or the one `--host` names), in bsky's layout: the tabs on the top row, the notes, the keys that work in the view, and a status row that names the picture protocol at its right. The keys are bsky's where bsky has the action; `?` lists them all, and `.` lists what the keys do to the selected note.

| Key | What it does |
|---|---|
| `1` `2` `3` …, `tab` `shift+tab` | The tabs, numbered in the order shown: by default 1 Home, 2 Local, 3 Global, 4 Notifications, 5 Search, 6 Profile (see the `tabs` setting below); `tab` and `shift+tab` go round them, as in bsky |
| `/` `i`, `t` | Type in the search box (it has focus when you arrive at an empty Search tab), and switch between searching notes and accounts |
| `enter` | The profile of the selected note's author, account found, or notification's sender; `esc` goes back to where it was opened from |
| `f` | Follows or unfollows the account shown on the Profile tab or found by a search, or takes back a request not yet accepted |
| `j` `k` `↓` `↑`, `pgdn` `pgup`, `g` `G` | Move; the next page loads by itself near the end (`untilId`) |
| `v` | The thread: what the note answers, the note, and two levels of replies; `esc` closes it |
| `space` | The note's files full screen (`←` `→` between them, `d` saves the one shown, `o` opens it in the browser); without pictures, the note in the browser |
| `w` | Shows or hides the text behind a content warning, which is folded until then |
| `d` | Saves every file of the note, as `msky download` names them |
| `o`, `c` | Opens the note in the browser, copies its address (OSC 52) |
| `n`, `r`, `Q` | A new note, a reply to the note shown, a quote of it (see below) |
| `l`, `e` | Reacts ❤ (bsky's like), or takes your reaction back; `e` chooses one of Misskey's default reactions (`←` `→` or `1`-`9` `0`, `enter`) |
| `b` | Renotes the note shown, or takes your renote of it back |
| `D` | Deletes your own note: `y` confirms, any other key keeps it |
| `R` `F5` | Loads the view again |
| `A` | The accounts: `enter` uses one (and its server), `x` then `y` logs one out, `a` logs another in |
| `?`, `q` | The keys, quit |

A renote is shown as the note it renotes under `⟳ renoted by`, a quote with a `❝` line for the note it quotes, a reply under a dim line for the note it answers. A file the uploader marked sensitive is drawn as any other, with a `⚠ sensitive` mark. MFM is drawn as plain text: `$[x2 大きい]` shows `大きい`, `$[ruby 漢字 かんじ]` shows `漢字(かんじ)`, `<center>` and `<small>` show what they hold, `**bold**` is bold, and `?[label](address)` shows its label. A custom emoji is shown by its name, `:blobcat:`. Without a picture protocol (or through a pipe of a multiplexer that answers none) the client runs as text and names the files instead; `MSKY_GRAPHICS=kitty|sixel|iterm2` names the protocol when the terminal does not answer. Pictures are kept in `MSKY_CACHE_DIR` (`off` for none), else `msky/images` in the platform's cache folder.

Without an account the client opens on the login form, as bsky's does: choose a server from [Misskey Hub](https://misskey-hub.net/servers/)'s list (the one `msky servers` prints), or press `/` and type any server's host, and allow msky in the browser that opens; the login is saved as `msky login` saves it. `esc` reads the server as a visitor instead: the Home, Notifications and Profile tabs say how to log in, and the Local and Global tabs show the server where it shows them to visitors.

The client reads one server at a time: switching accounts in `A` switches to that account's server.

Each timeline is a tab of its own, and keeps its place, the pages it loaded and its next page while another tab is shown; a tab is loaded the first time it is shown, and `R` loads again the one shown only. Social, Misskey's home and local mixed, is not a tab by default. `settings.json` in the config directory chooses the tabs and their order, and the numbers follow it:

```json
{"tabs": ["home", "social", "local", "global", "notifications", "search", "profile"]}
```

The names are `home`, `local`, `social`, `global`, `notifications`, `search` and `profile`; a name that is not one is said when the client starts and left out, and a list naming none is the default. `msky timeline social` reads Social whatever the setting says.

The Profile tab shows the account's name and address, how you and it follow each other, its counts and what it says about itself (as plain text), over its notes, paged as the timelines are. The Search tab searches notes (`notes/search`, paged by `untilId`) or accounts (`users/search`, paged by offset).

The composer (`n`, `r`, `Q`) takes bsky's keys: `ctrl+s` sends, `ctrl+o` opens a file browser to attach up to four pictures or one video (`space` marks several, `enter` chooses), `tab` moves to a file's alt text, `ctrl+x` removes a file, `esc` closes without sending. Misskey's own are `ctrl+w`, which adds or takes off a content warning, and `ctrl+p`, which steps through who may see the note (public, home, followers); a reply starts as visible as the note it answers. The length is checked against the server's own limit (its `meta`) when the note is sent. A write is sent once, and a key pressed again while one is on its way is refused. The notifications are marked read when their tab shows them.

Reacting, deleting and marking notifications read need `write:reactions` and `write:notifications`, which `msky login` asks for from this version: a login made before says to log in again (`A` then `a`, or `msky login`) when one of them is pressed, and the notifications are left unread.

## Use

msky asks the server `--host` or `MSKY_HOST` names (`--host misskey.design`, `--host https://misskey.example`), else the server of the account in use (see Logging in), else misskey.io. Without a login it shows what the server shows to visitors. A note or an account given by its address is read on the server in the address, whatever `--host` says.

| Command | What it does |
|---|---|
| `msky author ACTOR` | The account's notes, newest first |
| `msky note NOTE` | One note |
| `msky search QUERY` | The notes that match, newest first |
| `msky download NOTE` | Saves the files attached to the note |
| `msky timeline [KIND]` | A timeline, newest first: `home` (default), `local`, `social` or `global` |
| `msky notifications` | Your notifications, newest first |
| `msky whoami` | The account logged in on the server |
| `msky post TEXT` | Posts a note (see Posting) |
| `msky follow ACTOR` | Follows the account |
| `msky unfollow ACTOR` | Unfollows the account, or takes back a request to follow it |
| `msky follows [ACTOR]` | The accounts ACTOR (you by default) follows; `--pending`: your requests not yet accepted |
| `msky followers [ACTOR]` | The accounts that follow ACTOR (you by default); `--pending`: the requests waiting for your approval |
| `msky login` | Logs in with MiAuth |
| `msky logout` | Removes the saved token of the account in use (`--all`: of every account) |
| `msky accounts` | The accounts logged in, the one in use marked; `--use HOST` makes another the one in use |
| `msky servers` | Servers to log in to, the most accounts first |

An account is `name`, `@name`, `@name@host`, or its address (`https://misskey.io/@name`). A note is its ID or its address (`https://misskey.io/notes/ID`). Either address works without `https://`.

`-n` sets how many notes `author`, `search` and `timeline` print, and how many notifications `notifications` prints (20 by default). They read them a page of up to 100 at a time, each page older than the last note of the one before, until there are enough or no more. A note is printed once, and a page that starts from a note seen before ends the reading.

`author` takes the options of bsky's `author`. `--filter` chooses which notes; Misskey's `users/notes` filters only by replies and by files, so two of the five are told by msky from the notes it gets, reading full pages until there are enough:

| `--filter` | Notes listed |
|---|---|
| `with-replies` (default) | Notes and replies |
| `no-replies` | Notes, not replies |
| `media` | Notes with files attached, of any type (Misskey's `withFiles`, which it takes without replies) |
| `video` | Notes with a video attached, its own or the renoted or quoted note's |
| `threads` | Notes, and the account's replies to its own notes (a reply whose note the server does not send along is left out) |

Any other value is a usage error (exit 1). `--reposts` keeps the account's renotes too, each under a `⟳ renoted by @name` line and shown as the note it renotes; a quote is the account's own note and is always listed.

### Logging in

`msky login` without `--host` (and without `MSKY_HOST`), in a terminal, lists servers to choose from and takes a number, or any server's host; elsewhere it logs in on the server of the account in use, else misskey.io. The list is the one [Misskey Hub](https://misskey-hub.net/servers/) publishes (`https://instanceapp.misskey.page/instances.json`), the servers whose nodeinfo says `misskey`, the most accounts first; when it cannot be read, msky offers the few it knows (misskey.io, misskey-square.net, takusuki.com, oekakiskey.com, misskey.day) and says why. `msky servers` prints the list (`-n`, 20 by default; `--json`: `{"host", "name", "users", "langs", "openRegistrations"}` a line).

`msky login` logs in on the server with [MiAuth](https://misskey-hub.net/en/docs/for-developers/api/token/miauth/). It prints an address on the server, `https://misskey.io/miauth/<session>?name=msky&permission=...`, on stderr, and opens it in the web browser when stderr is a terminal (with `MSKY_BROWSER` when set, else `xdg-open`, `open`, or the Windows URL handler). Open it where you are signed in to the server and allow the access; msky asks the server every two seconds whether it was allowed, and saves the token it gets. msky never sees your password.

msky asks for `read:account`, `read:notifications`, `read:following` and `read:reactions` to read, `write:notes` and `write:drive` for `post` and the client's notes (and to delete your own and renote), `write:following` for `follow` and `unfollow`, and `write:reactions` and `write:notifications` for the client's reactions and the notifications' read mark, and for nothing else: it changes no setting. The permissions given are saved with the token. A login saved by an earlier msky, which asked to read only, cannot post or follow: `post`, `follow` and `unfollow` say so before anything is sent (exit 1) and ask you to run `msky login` again. Access not allowed within `--timeout` seconds (300 by default), or denied, is a usage error (exit 1) and nothing is saved. Logged in, `msky login` prints `logged in as @name@host` (`--json`: `{"id", "username", "host"}`).

The token is saved per server, as bsky saves its sessions: in `accounts/<host>.json` in the config directory (`MSKY_CONFIG_DIR`, else `msky` in the platform's config folder: `~/.config/msky` on Linux, `~/Library/Application Support/msky` on macOS, `%APPDATA%\msky` on Windows), readable and writable by you alone on Unix (mode 0600). `accounts.json` beside it names the server of the account in use, the one logged in last; it holds no secret. Logging in on the same server again replaces its account.

With a token saved for the server, every command sends it to that server, and only to that one: `timeline`, `notifications` and `whoami` need it, `author`, `note`, `search` and `download` use it when it is there and read as a visitor when it is not. `timeline home` and `timeline social` and `notifications` without a login are a usage error (exit 1); `timeline local` and `timeline global` are read as a visitor, where the server shows them to visitors. A token the server no longer takes is an error that says to log in again.

`timeline` reads pages as `author` does, and leaves the renotes out unless `--reposts` asks for them. `notifications` reads them the same way, and leaves them unread on the server. `msky logout` removes the token of the account in use, or of the one on `--host`, and prints `logged out @name@host`; `--all` removes every one. The access stays listed in the server's settings, where you can revoke it.

`msky accounts` lists the accounts logged in as bsky's `accounts` does, one a line with the one in use marked `*` (`--json`: `{"id", "username", "host", "service", "current"}`). `msky accounts --use HOST` makes the account on HOST the one in use; a server with no account logged in is a usage error that names those that have one.

Text that others wrote is printed without its control characters, so an escape sequence in a note or a name cannot set the clipboard or the window title, or clear the screen. A note's reactions are listed ten kinds at most, the most given first; `--json` has them all.

### Posting

`msky post TEXT` posts a note as the account logged in on the server and prints its address; TEXT `-` reads the text from stdin. It takes the options of bsky's `post`: `--image PATH` up to four times, or one `--video PATH`, and `--alt TEXT` for each of them in the same order; `--reply NOTE` for the note it answers; `--quote NOTE` for the note it quotes (Misskey's quote renote); and `--no-card` (below). Misskey's own are `--cw TEXT`, a content warning shown in place of the text until it is opened, and `--visibility public|home|followers` (`public` by default). The files are uploaded to your drive first (`drive/files/create`, with the alt text as their caption) and the note is made with them (`notes/create`). A note given to `--reply` or `--quote` is read first, and has to be one on the server posted to; a note of another server is given by its address there.

What can be told before anything is written is a usage error (exit 1) and nothing is sent: a text longer than the server takes (the `maxNoteTextLength` its `meta` gives, else Misskey's 3000; counted in code points as its API counts them, after the text is trimmed as the server trims it), a content warning over 100 or an alt text over 512, more than four pictures, pictures and a video together, more `--alt` than files, a note with no text, file or quote, a visibility Misskey does not have, a note to reply to or quote on another server, a file larger than the account may upload (the `maxFileSizeMb` that `i` gives), and a login that may not write. A file that cannot be read is exit 3. A post is sent once and never tried again; when the note cannot be made after its files were uploaded, the files stay in your drive.

#### Link previews

Bluesky and Misskey show a link's page differently. On Bluesky the client that posts reads the page and sends the card with the post, which is what bsky does. On Misskey nothing of the kind is sent: the note is its text, and each client that shows it draws a preview under the addresses it finds there. Misskey's web client parses the text as MFM, takes every address written as it is (`https://...`), in angle brackets (`<https://...>`) or as a link with a label (`[label](https://...)`), leaves out silent links (`?[label](https://...)`) and the address of the note quoted, and for each one asks its own server `GET /url?url=<address>&lang=<language>`. The server reads the page with [summaly](https://github.com/misskey-dev/summaly) (its Open Graph, Twitter card and other tags) and answers the title, description, thumbnail, icon and site name; the thumbnail and icon come through the server's media proxy. The server keeps an answer for an hour and marks it to be cached for a day, a failure too, and an administrator can turn previews off (`URL_PREVIEW_DISABLED`). The sources are [extract-url-from-mfm.ts](https://github.com/misskey-dev/misskey/blob/develop/packages/frontend/src/utility/extract-url-from-mfm.ts), [use-note.ts](https://github.com/misskey-dev/misskey/blob/develop/packages/frontend/src/composables/use-note.ts), [MkUrlPreview.vue](https://github.com/misskey-dev/misskey/blob/develop/packages/frontend/src/components/MkUrlPreview.vue), [UrlPreviewService.ts](https://github.com/misskey-dev/misskey/blob/develop/packages/backend/src/server/web/UrlPreviewService.ts) and the [MFM syntax](https://github.com/misskey-dev/mfm.js/blob/develop/docs/syntax.md).

So msky sends the text as it is, and the preview is whatever the server of each viewer makes of the page. Every address in the text gets one, not only the first, and a note with pictures gets them as well. A server of another kind (Mastodon, for one) that receives the note makes its own preview in its own way.

`--no-card` writes every link in the text as a silent link, which Misskey's clients show no preview of: `https://example.com/a` is sent as `?[<plain>https://example.com/a</plain>](https://example.com/a)`, which reads as the address it was (the `<plain>` keeps the address in the label from being read as MFM), and `[label](https://...)` as `?[label](https://...)`. The text is counted for the limit as it is sent. A server of another kind gets the link without the silent mark and may still show a preview.

#### Checking the preview before posting

`msky post --dry-run` does everything `post` does except write: it checks the note, reads the files, asks the server who the account is and reads the notes to reply to and quote, then prints what it would send and asks the server for the preview of the first address Misskey would show one under, as Misskey's web client asks for it (without the part after `#`, with `lang=ja-JP`). Nothing is uploaded or posted. It needs a login, or `MSKY_TOKEN`, as `post` does, so a job that dry-runs also finds a token that is missing or turned down.

```
$ msky post --dry-run "New article https://debimate.jp/post/a/"
dry run: nothing was sent to misskey.io
as @nchika@misskey.io, public
text:
New article https://debimate.jp/post/a/
preview of https://debimate.jp/post/a/:
  title: The article's title
  description: What the page says it is about
  image: <the server's media proxy>/preview.webp?url=...&preview=1
  site: debimate
```

With `--json` it prints one line: `{"dryRun": true, "host", "username", "note": {"text", "cw", "visibility", "replyId", "renoteId", "files": [{"path", "name", "size", "alt"}]}, "previewed": [every address that gets a preview], "preview": {"url", "title", "description", "thumbnail", "icon", "sitename", "sensitive"}}`, `preview` null when no address gets one. When the server cannot make the preview (the page is down, not yet published, or the server makes no previews), it prints the same with `"preview": null` and `"previewError"`, and exits 4, so a job stops before it posts a note without one. As the server keeps a failure for a day, ask for the preview only once the page is up.

#### Posting from CI

`MSKY_TOKEN` holds an access token to use in place of the saved login, for a job with no one at the keyboard; it goes with `MSKY_HOST` (misskey.io when not set, and not the server of a saved login). It is used on that server only, for every command, and it is never printed. Make the token on the server in Settings, API, Generate access token, with "View your account information" (`read:account`) and "Compose or delete notes" (`write:notes`), and, to attach files and to follow, "Edit or delete your Drive files and folders" (`write:drive`) and "Follow or unfollow other accounts" (`write:following`). A token the server turns down says what it needs (exit 4).

A GitHub Actions step, with the token in the repository's secrets as `MSKY_TOKEN`:

```yaml
- name: Install msky
  run: cargo install --locked --git https://github.com/nao1215/misskey-terminal-client
- name: Check the preview
  env:
    MSKY_HOST: misskey.io
    MSKY_TOKEN: ${{ secrets.MSKY_TOKEN }}
    URL: https://debimate.jp/post/a/
  run: msky post --dry-run --json "New article $URL" | jq -e '.preview.title'
- name: Post
  env:
    MSKY_HOST: misskey.io
    MSKY_TOKEN: ${{ secrets.MSKY_TOKEN }}
    URL: https://debimate.jp/post/a/
  run: msky post "New article $URL"
```

The job's log masks the secret, and msky does not print it. The dry run exits 4 when the server makes no preview, and `jq -e` fails the step when the preview has no title. While this repository is private, `cargo install --git` needs credentials that can read it.

### Following

`msky follow ACTOR` follows the account (`following/create`) and prints `following @name@host`; one followed already is said so and nothing is sent. An account that approves its followers gets a request, `asked to follow @name@host; they approve their followers`. An account of another server is followed once its server accepts: msky asks again after the follow, and until then says `asked to follow @name@host; pending until their server accepts`.

`msky follows [ACTOR]` and `msky followers [ACTOR]` list the accounts on each side of an account's follows (`users/following`, `users/followers`), yours without ACTOR, with the options of bsky's: `-n` (50 by default), paged by `untilId`. A line is `name @name@host  (following)  address`; `--json` prints an account a line in the shape `whoami --json` prints. `--pending` lists your own requests instead: `follows --pending` the accounts you asked to follow that have not accepted (`following/requests/sent`), `followers --pending` the accounts waiting for your approval (`following/requests/list`). `msky unfollow ACTOR` unfollows (`following/delete`), or takes back a request not yet approved (`following/requests/cancel`); an account you do not follow, and yourself, are a usage error (exit 1). ACTOR is written as for `author`; an address on another server is a usage error, as the account is followed from the server logged in on (give it as `@name@host`).

### Saving files

`msky download NOTE` saves each file attached to the note, and those of the note it renotes or quotes, and prints each saved path on a line. A note without files is a usage error (exit 1) and nothing is written. `msky author ACTOR --download` saves the files of the notes it read instead of printing them, in the same way, skipping the notes without files; `-o DIR` saves them there and implies `--download`. The folder is `-o`, else `MSKY_DOWNLOAD_DIR`, else `msky` in your Downloads folder.

A file is named after the note it is attached to, its place among that note's files, and its type: `9xyz0abc12-1.jpg`. The extension is the one the file was uploaded with when it agrees with the type the server detected, else the usual one for that type, and `.bin` for a type msky does not know, so a picture uploaded as `cat.exe` is not saved as a program. A file already there is never written over: the new one is saved as `9xyz0abc12-1 (1).jpg`. msky saves files up to 200 MB, fetches only http and https addresses, and removes a file it could not finish.

### JSON

`--json` prints one note per line, in the same shape for every command:

```json
{"id":"9xyz0abc12","createdAt":"2026-11-26T10:54:49.672Z","url":"https://misskey.io/notes/9xyz0abc12","uri":null,"user":{"id":"9u0000abcd","username":"alice","host":"misskey.io","name":"Alice"},"text":"山の写真 🏔️","cw":null,"reactionCount":19,"reactions":{":blobcat@misskey.design:":2,":igyo@.:":12,"👍":5},"renoteCount":3,"repliesCount":1,"files":[{"id":"9xyz0f0001","name":"mountain.jpg","url":"https://media.misskey.example/files/mountain.jpg","type":"image/jpeg","size":204800,"isSensitive":false}],"replyId":null,"renoteId":null,"renote":null}
```

`url` is the note on the server asked, `uri` where a note of another server was first posted. `host` is the server the account is on, filled in where Misskey writes null for an account of the server asked. `reactions` keeps the server's keys: `:name@.:` is a custom emoji of the server asked. `renote` is the note a renote or a quote carries, in the same shape. `timeline --json` prints notes in that shape. `notifications --json` prints one line per notification: `{"id", "createdAt", "type", "user", "reaction", "note"}`, where `type` is Misskey's (`follow`, `mention`, `reply`, `renote`, `quote`, `reaction`, ...), `user` is in the shape of a note's `user` (null for what the server itself says), and `note` is the note it is about in the shape above (or null). `whoami --json` prints the account as a note's `user` with `url`, `description`, `followersCount`, `followingCount` and `notesCount`. `logout --json` prints `{"loggedOut": [{"id", "username", "host"}]}`. `post --json` prints the note made as bsky's `post` prints its post, `{"uri": "<the note's address>", "id": "<its ID>"}` (a note of the server is known to others by that address); `--dry-run` prints as above. `follow --json` prints `{"id": "<the account's ID>"}`, with `"already": true` when it was followed already and `"requested": true` for a request; `unfollow --json` prints `{"deleted": "<the account's ID>"}`, as bsky's do with the follow's record. `download --json` and `author --download --json` print one line per saved file instead, as bsky's `download` does with `post`: `{"note": "<the note's address>", "kind": "image" | "video" | "audio" | "file", "url": "<where it was fetched>", "path": "<where it was saved>"}`. An error is printed as `{"error": ..., "hint": ..., "status": ...}` on stdout as well as on stderr.

### Rate limits

A server that answers it is asked too often (HTTP 429, `RATE_LIMIT_EXCEEDED`) is waited for as its `Retry-After` says, a minute at most, or 1, 2 and then 4 seconds when it says nothing, and asked again, three times at most. msky says on stderr how long it waits. A busy gateway (502, 503, 504) is asked again the same way; any other refusal is reported at once.

## Environment

| Variable | Meaning |
|---|---|
| `MSKY_HOST` | Server to read (also `--host`); the account in use's, else misskey.io, by default |
| `MSKY_TOKEN` | Access token to use in place of the saved login, on `MSKY_HOST` (or `--host`, else misskey.io) only |
| `MSKY_CONFIG_DIR` | Where the logins are saved |
| `MSKY_BROWSER` | The program `msky login` opens the address with |
| `MSKY_MIAUTH_SESSION` | The MiAuth session ID `msky login` uses instead of a random one; for tests against a stand-in server |
| `MSKY_DOWNLOAD_DIR` | Where files are saved when `-o` is not given |
| `MSKY_GRAPHICS` | The picture protocol the client uses, `kitty`, `sixel` or `iterm2`, for a terminal that does not answer the query |
| `MSKY_CACHE_DIR` | Where the client keeps the pictures it downloaded; `off` keeps none |
| `MSKY_SERVERS_URL` | Another address for the server list, or `off` for the servers msky knows; for tests against a stand-in server |
| `NO_COLOR` | Draws the client without color |
| `TZ` | Time zone the times are shown in |

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: an unknown flag, a malformed `--host`, something that is not a note or an account, no login where one is needed, a login not allowed in time or denied, a note `post` can tell is wrong before it is sent, a login that may not post or follow |
| 2 | `msky` without a command not run in an interactive terminal |
| 3 | A file could not be read or written: the output, a download, a login file |
| 4 | The server could not be reached or said no, or could not make the preview `post --dry-run` asked for |

## License

[MIT](LICENSE)
