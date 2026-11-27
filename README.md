[![Build](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# msky

An unofficial [Misskey](https://misskey-hub.net) client for the terminal. Pictures and videos are drawn in place.

![the timeline, a note's pictures, a thread and the notifications](doc/img/demo.gif)

msky is built as [bsky](https://github.com/nao1215/bluesky-terminal-client) is for Bluesky, with the same keys. It is for Misskey itself; a fork such as Sharkey may work, but it is not looked after.

## Install

```sh
git clone https://github.com/nao1215/misskey-terminal-client
cd misskey-terminal-client
cargo install --locked --path .
```

It needs Rust 1.90 or later.

## Use

Run `msky`, choose a server and allow msky in the browser that opens (MiAuth: msky never sees your password). `esc` reads the server as a visitor instead. `?` lists every key, `.` what the keys do to the selected note or account.

| Pictures with | Terminals |
|---|---|
| kitty graphics | kitty, Ghostty |
| sixel | foot, mlterm, Windows Terminal 1.22+ |
| iTerm2 images | iTerm2, WezTerm |

Other terminals get text, and `space` opens a note in the browser. `MSKY_GRAPHICS` names the protocol when the terminal does not answer. Custom emoji are drawn as pictures, in notes, names and reactions. A video in MP4 with H.264 plays in the viewer without sound, up to 100 MB; WebM and other codecs are saved with `d` or opened with `o`.

MFM is drawn as plain text: `$[x2 大きい]` shows `大きい`, `$[ruby 漢字 かんじ]` shows `漢字(かんじ)`, `**bold**` is bold. A content warning is folded until `w` opens it, and a file marked sensitive has a `⚠ sensitive` mark.

Files go up without their metadata: a photo's EXIF (its GPS position among it), XMP, IPTC and comments, a video's user data and location. This holds for `msky post`, the composer and the profile editor.

msky speaks English and 日本語: the language of your system, or the one chosen in the settings (`s`). The commands are in English.

## Keys

| Key | Action |
|---|---|
| `1`–`7` `tab` | Home, Local, Global, Notifications, Search, Profile, Chat (the `tabs` setting) |
| `+` `x` | Add a tab (one not shown, an antenna, a list, a channel), close the one shown |
| `j` `k` `g` `G` | Move; more loads by itself near the end |
| `n` `r` `Q` | Note, reply, quote |
| `l` `e` `b` | React ❤, choose a reaction, renote |
| `f` `M` `B` | Follow, mute, block the account shown or found |
| `m` | Message the profile shown |
| `D` `!` | Delete your note, report a note or an account |
| `v` `space` `w` | Thread, files, the text behind a content warning |
| `d` `o` `c` | Save the files, open in the browser, copy the address |
| `enter` `esc` | The author's profile, back |
| `/` `t` | Search, notes or accounts |
| `e` on your profile | Edit your profile |
| `R` | Load again |
| `A` `T` `s` | Accounts, themes, settings |
| `?` `q` | Help, quit |

In the composer, `ctrl+s` sends, `ctrl+o` attaches up to four pictures or one video, `ctrl+w` adds a content warning, `ctrl+p` chooses who sees the note (public, home, followers). A write is sent once; `D`, `B`, `x`, `!` and logging out wait for `y`.

The Chat tab is Misskey's chat (from Misskey 2025.4). On a server without it, misskey.io among them, the tab is not shown. A thread is read ahead when the selection rests on a note, and the Notifications tab counts what is unread.

## Settings

`s` changes `settings.json` in the config folder: the theme (bsky's themes, `T` previews them), the language, the download folder, the browser and the tabs. `NO_COLOR` draws in monochrome.

```json
{"tabs": ["home", "social", "local", "global", "notifications", "search", "profile", "chat"]}
```

A tab is one of those names, or an antenna, a list or a channel as `+` writes it (`antenna/HOST/ID/NAME`, `list/...`, `channel/...`). One of another server shows only while its account is in use.

## Commands

```sh
msky timeline -n 10
msky notifications --json | jq -r .type
msky author syuilo -n 50 --filter media --download
msky note https://misskey.io/notes/au102erst5tf04vs
msky search Misskey --json | jq -r .url
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

| Command | What it does |
|---|---|
| `timeline [KIND]` | `home` (default), `local`, `social` or `global` |
| `notifications` | Your notifications; they stay unread |
| `author ACTOR` | An account's notes; `--filter with-replies\|no-replies\|media\|video\|threads`, `--reposts` |
| `note NOTE`, `thread NOTE` | One note; the note with what it answers and its replies |
| `search QUERY` | Notes that match |
| `profile [ACTOR]`, `whoami` | An account; the one logged in |
| `reactions NOTE` (`likes`), `renotes NOTE` (`reposts`) | Who reacted or renoted |
| `post TEXT` | A note: `--image` (up to 4), `--video`, `--alt`, `--reply`, `--quote`, `--cw`, `--visibility`, `--no-card`, `--dry-run` |
| `like NOTE` (`react`), `unlike` | React ❤ or `--reaction`; take it back |
| `repost NOTE` (`renote`), `unrepost` | Renote; take it back |
| `delete NOTE` | Delete your note |
| `follow ACTOR`, `unfollow` | Follow; unfollow or take back a request |
| `follows [ACTOR]`, `followers [ACTOR]` | Who follows whom; `--pending` for requests |
| `mute`, `unmute`, `mutes` | Mute an account; list them |
| `block`, `unblock`, `blocks` | Block an account; list them |
| `report TARGET --reason R` | Report to the moderators: `spam`, `violation`, `misleading`, `sexual`, `rude`, `other`; asks unless `--yes` |
| `chat [ACTOR] [TEXT]` | Conversations, the messages with ACTOR, or send TEXT |
| `download NOTE` | Save a note's files (`-o DIR`) |
| `login`, `logout`, `accounts`, `servers` | Log in with MiAuth, log out (`--all`), list or switch accounts (`--use HOST`), list servers |

An account is `name`, `@name@host` or its address; a note is its ID or its address. `--host` (or `MSKY_HOST`) picks the server, else the account in use's, else misskey.io. `-n` sets how many to print (20 by default), read a page of up to 100 at a time. `msky --help` and `msky <command> --help` say the rest.

### Logging in

`msky login` lists the servers in [Misskey Hub](https://misskey-hub.net/servers/)'s list that run Misskey, or takes any host. It prints the MiAuth address, opens it, and saves the token once you allow it.

msky asks for these permissions, and nothing else:

| Permission | For |
|---|---|
| `read:account`, `read:notifications`, `read:following`, `read:reactions` | Reading |
| `write:notes`, `write:drive` | Notes, renotes, deleting, files |
| `write:following` | Follow and unfollow |
| `write:reactions`, `write:notifications` | Reactions, marking notifications read |
| `read:mutes`, `write:mutes`, `read:blocks`, `write:blocks` | Mute and block |
| `read:chat`, `write:chat` | Chat |
| `write:account` | The profile editor |
| `write:report-abuse` | Reports |

A login made by an earlier msky lacks some of them: msky says so before it sends anything, and asks you to log in again. The token is saved per server in `accounts/<host>.json` in the config folder, readable by you alone (mode 0600), and is sent only to that server.

### Posting

`msky post` uploads the files to your drive, then makes the note, and prints its address. It is sent once and never tried again. What can be checked first is checked before anything is sent: the server's length limit, four pictures or one video, the alt texts, the file size the account may upload.

Misskey draws a link's preview on each viewer's server, so msky sends the text as it is. `--no-card` writes every link as a silent link, which gets no preview.

`--dry-run` sends nothing: it checks the note and the login, prints what it would send, and asks the server for the preview of the first link. It exits 4 when the server cannot make one, so a job stops before it posts.

`MSKY_TOKEN` is an access token for a job with no one at the keyboard, used on `MSKY_HOST` only and never printed. Make it in Settings, API, with `read:account` and `write:notes` (and `write:drive`, `write:following` to attach files and follow).

```yaml
- name: Post
  env:
    MSKY_HOST: misskey.io
    MSKY_TOKEN: ${{ secrets.MSKY_TOKEN }}
    URL: https://debimate.jp/post/a/
  run: |
    msky post --dry-run --json "New article $URL" | jq -e '.preview.title'
    msky post "New article $URL"
```

### Saving files

`msky download NOTE` and `msky author ACTOR --download` save each file as `<note ID>-<n>.<ext>` in `-o`, `MSKY_DOWNLOAD_DIR`, or `msky` in your Downloads folder. A file is never written over, and the extension follows the file's real type. Files over 200 MB are not saved.

### JSON

`--json` prints one note a line, in the same shape for every command:

```json
{"id":"9xyz0abc12","createdAt":"2026-11-26T10:54:49.672Z","url":"https://misskey.io/notes/9xyz0abc12","uri":null,"user":{"id":"9u0000abcd","username":"alice","host":"misskey.io","name":"Alice"},"text":"山の写真 🏔️","cw":null,"reactionCount":19,"reactions":{":blobcat@misskey.design:":2,":igyo@.:":12,"👍":5},"renoteCount":3,"repliesCount":1,"files":[{"id":"9xyz0f0001","name":"mountain.jpg","url":"https://media.misskey.example/files/mountain.jpg","type":"image/jpeg","size":204800,"isSensitive":false}],"replyId":null,"renoteId":null,"renote":null}
```

`notifications` prints `{"id", "createdAt", "type", "user", "reaction", "note"}`. `post` prints `{"uri", "id"}`, `follow` `{"id"}`, `unfollow` `{"deleted"}`, and `download` one line a file, `{"note", "kind", "url", "path"}`. An error is `{"error", "hint", "status"}` on stdout as well as stderr.

A server that says it is busy (429, 502, 503, 504) is asked again up to three times, after its `Retry-After` or 1, 2 and 4 seconds.

## Environment

| Variable | Meaning |
|---|---|
| `MSKY_HOST` | Server to read (also `--host`) |
| `MSKY_TOKEN` | Access token in place of the saved login, on `MSKY_HOST` only |
| `MSKY_CONFIG_DIR` | Config folder |
| `MSKY_CACHE_DIR` | Picture cache; `off` for none |
| `MSKY_DOWNLOAD_DIR` | Where `d` and `download` save |
| `MSKY_GRAPHICS` | `kitty`, `sixel` or `iterm2` |
| `MSKY_BROWSER` | Program that opens addresses |
| `MSKY_SERVERS_URL` | Another server list, or `off`; for tests |
| `MSKY_MIAUTH_SESSION` | A fixed MiAuth session; for tests |
| `LC_ALL` `LC_MESSAGES` `LANG` | Language, unless one is chosen in the settings |
| `NO_COLOR` | No color |
| `TZ` | Time zone of the times shown |

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: a wrong flag or argument, no login where one is needed, a note that cannot be sent as written |
| 2 | `msky` without a command outside an interactive terminal |
| 3 | A file could not be read or written |
| 4 | The server could not be reached or said no, or made no preview for `--dry-run` |

## Contributing

```sh
just test    # unit tests
just e2e     # end-to-end tests (atago)
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Not made by or affiliated with the Misskey project.
