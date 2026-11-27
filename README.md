[![Build](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# msky

An unofficial [Misskey](https://misskey-hub.net) client for the terminal. It reads what a Misskey server shows to visitors: an account's notes, one note, a search, and the files attached to notes. Logged in with MiAuth, with read-only access, it reads your timelines and notifications too.

```sh
msky login
msky timeline -n 10
msky notifications --json | jq -r .type
msky author syuilo -n 5
msky note https://misskey.io/notes/au102erst5tf04vs
msky search Misskey --json | jq -r .url
msky download https://misskey.io/notes/au102erst5tf04vs -o pictures
msky author syuilo -n 50 --filter media --download
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
| `msky login` | Logs in with MiAuth, with read-only access |
| `msky logout` | Removes the saved token of the account in use (`--all`: of every account) |

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

`msky login` logs in on the server with [MiAuth](https://misskey-hub.net/en/docs/for-developers/api/token/miauth/). It prints an address on the server, `https://misskey.io/miauth/<session>?name=msky&permission=...`, on stderr, and opens it in the web browser when stderr is a terminal (with `MSKY_BROWSER` when set, else `xdg-open`, `open`, or the Windows URL handler). Open it where you are signed in to the server and allow the access; msky asks the server every two seconds whether it was allowed, and saves the token it gets. msky never sees your password.

The access is read-only: msky asks for `read:account`, `read:notifications`, `read:following` and `read:reactions`, and for nothing that writes. Access not allowed within `--timeout` seconds (300 by default), or denied, is a usage error (exit 1) and nothing is saved. Logged in, `msky login` prints `logged in as @name@host` (`--json`: `{"id", "username", "host"}`).

The token is saved per server, as bsky saves its sessions: in `accounts/<host>.json` in the config directory (`MSKY_CONFIG_DIR`, else `msky` in the platform's config folder: `~/.config/msky` on Linux, `~/Library/Application Support/msky` on macOS, `%APPDATA%\msky` on Windows), readable and writable by you alone on Unix (mode 0600). `accounts.json` beside it names the server of the account in use, the one logged in last; it holds no secret. Logging in on the same server again replaces its account.

With a token saved for the server, every command sends it to that server, and only to that one: `timeline`, `notifications` and `whoami` need it, `author`, `note`, `search` and `download` use it when it is there and read as a visitor when it is not. `timeline home` and `timeline social` and `notifications` without a login are a usage error (exit 1); `timeline local` and `timeline global` are read as a visitor, where the server shows them to visitors. A token the server no longer takes is an error that says to log in again.

`timeline` reads pages as `author` does, and leaves the renotes out unless `--reposts` asks for them. `notifications` reads them the same way, and leaves them unread on the server. `msky logout` removes the token of the account in use, or of the one on `--host`, and prints `logged out @name@host`; `--all` removes every one. The access stays listed in the server's settings, where you can revoke it.

Text that others wrote is printed without its control characters, so an escape sequence in a note or a name cannot set the clipboard or the window title, or clear the screen. A note's reactions are listed ten kinds at most, the most given first; `--json` has them all.

### Saving files

`msky download NOTE` saves each file attached to the note, and those of the note it renotes or quotes, and prints each saved path on a line. A note without files is a usage error (exit 1) and nothing is written. `msky author ACTOR --download` saves the files of the notes it read instead of printing them, in the same way, skipping the notes without files; `-o DIR` saves them there and implies `--download`. The folder is `-o`, else `MSKY_DOWNLOAD_DIR`, else `msky` in your Downloads folder.

A file is named after the note it is attached to, its place among that note's files, and its type: `9xyz0abc12-1.jpg`. The extension is the one the file was uploaded with when it agrees with the type the server detected, else the usual one for that type, and `.bin` for a type msky does not know, so a picture uploaded as `cat.exe` is not saved as a program. A file already there is never written over: the new one is saved as `9xyz0abc12-1 (1).jpg`. msky saves files up to 200 MB, fetches only http and https addresses, and removes a file it could not finish.

### JSON

`--json` prints one note per line, in the same shape for every command:

```json
{"id":"9xyz0abc12","createdAt":"2026-11-26T10:54:49.672Z","url":"https://misskey.io/notes/9xyz0abc12","uri":null,"user":{"id":"9u0000abcd","username":"alice","host":"misskey.io","name":"Alice"},"text":"山の写真 🏔️","cw":null,"reactionCount":19,"reactions":{":blobcat@misskey.design:":2,":igyo@.:":12,"👍":5},"renoteCount":3,"repliesCount":1,"files":[{"id":"9xyz0f0001","name":"mountain.jpg","url":"https://media.misskey.example/files/mountain.jpg","type":"image/jpeg","size":204800,"isSensitive":false}],"replyId":null,"renoteId":null,"renote":null}
```

`url` is the note on the server asked, `uri` where a note of another server was first posted. `host` is the server the account is on, filled in where Misskey writes null for an account of the server asked. `reactions` keeps the server's keys: `:name@.:` is a custom emoji of the server asked. `renote` is the note a renote or a quote carries, in the same shape. `timeline --json` prints notes in that shape. `notifications --json` prints one line per notification: `{"id", "createdAt", "type", "user", "reaction", "note"}`, where `type` is Misskey's (`follow`, `mention`, `reply`, `renote`, `quote`, `reaction`, ...), `user` is in the shape of a note's `user` (null for what the server itself says), and `note` is the note it is about in the shape above (or null). `whoami --json` prints the account as a note's `user` with `url`, `description`, `followersCount`, `followingCount` and `notesCount`. `logout --json` prints `{"loggedOut": [{"id", "username", "host"}]}`. `download --json` and `author --download --json` print one line per saved file instead, as bsky's `download` does with `post`: `{"note": "<the note's address>", "kind": "image" | "video" | "audio" | "file", "url": "<where it was fetched>", "path": "<where it was saved>"}`. An error is printed as `{"error": ..., "hint": ..., "status": ...}` on stdout as well as on stderr.

### Rate limits

A server that answers it is asked too often (HTTP 429, `RATE_LIMIT_EXCEEDED`) is waited for as its `Retry-After` says, a minute at most, or 1, 2 and then 4 seconds when it says nothing, and asked again, three times at most. msky says on stderr how long it waits. A busy gateway (502, 503, 504) is asked again the same way; any other refusal is reported at once.

## Environment

| Variable | Meaning |
|---|---|
| `MSKY_HOST` | Server to read (also `--host`); the account in use's, else misskey.io, by default |
| `MSKY_CONFIG_DIR` | Where the logins are saved |
| `MSKY_BROWSER` | The program `msky login` opens the address with |
| `MSKY_MIAUTH_SESSION` | The MiAuth session ID `msky login` uses instead of a random one; for tests against a stand-in server |
| `MSKY_DOWNLOAD_DIR` | Where files are saved when `-o` is not given |
| `TZ` | Time zone the times are shown in |

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: an unknown flag, a malformed `--host`, something that is not a note or an account, no login where one is needed, a login not allowed in time or denied |
| 3 | A file could not be read or written: the output, a download, a login file |
| 4 | The server could not be reached or said no |

## License

[MIT](LICENSE)
