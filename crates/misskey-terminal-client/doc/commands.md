# msky commands

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

`msky author alice` prints a note like this:

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

An account is `name`, `@name@host` or its address; a note is its ID or its address. `--host` (or `MSKY_HOST`) picks the server, else the server of the account in use, else misskey.io. `-n` sets how many to print (20 by default); msky reads up to 100 a page. `msky <command> --help` has the rest.

## Logging in

`msky login` lists the Misskey servers from [Misskey Hub](https://misskey-hub.net/servers/), or takes any host. It prints the MiAuth address, opens it, and saves the token once you allow it. msky asks for these permissions and no others:

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

If a login from an earlier msky lacks one of them, msky says so before it sends anything and asks you to log in again. The token is saved per server in `accounts/<host>.json` in the config folder, readable by you alone (mode 0600), and goes only to that server.

## Posting

`msky post` uploads the files to your drive, makes the note, and prints its address. It sends once and never retries. Before sending, it checks what it can: the server's length limit, four pictures or one video, the alt texts, and the file size the account may upload.

Each viewer's server draws a link's preview, so msky sends the text as written. `--no-card` turns every link into a silent link, which gets no preview.

`--dry-run` sends nothing. It checks the note and the login, prints what it would send, and asks the server for the first link's preview. It exits 4 when the server cannot make one, so a job stops before it posts.

For a job with no one at the keyboard, set `MSKY_TOKEN` to an access token. msky uses it on `MSKY_HOST` only and never prints it. Make it in Settings, API, with `read:account` and `write:notes`, plus `write:drive` to attach files and `write:following` to follow.

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

## Saving files

`msky download NOTE` and `msky author ACTOR --download` save each file as `<note ID>-<n>.<ext>` in `-o`, `MSKY_DOWNLOAD_DIR`, or `msky` in your Downloads folder. msky never writes over a file, names the extension after the file's real type, and skips files over 200 MB.

## JSON

`--json` prints one note a line, in the shape bsky and mstdn print too ([doc/json.md](https://github.com/nao1215/social-terminal-clients/blob/main/doc/json.md) lists every key):

```json
{"createdAt":"2026-11-26T10:54:49.672Z","cw":null,"files":[{"description":null,"id":"9xyz0f0001","isSensitive":false,"mime":"image/jpeg","name":"mountain.jpg","previewUrl":"https://media.misskey.example/files/thumbnail-mountain.webp","size":204800,"type":"image","url":"https://media.misskey.example/files/mountain.jpg"}],"id":"9xyz0abc12","likeCount":19,"quoteId":null,"reactions":{":blobcat@misskey.design:":2,":igyo@.:":12,"👍":5},"repliesCount":1,"replyId":null,"repostCount":3,"repostOf":null,"text":"山の写真 🏔️","uri":null,"url":"https://misskey.io/notes/9xyz0abc12","user":{"host":"misskey.io","id":"9u0000abcd","name":"Alice","username":"alice"},"visibility":"public"}
```

`likeCount` counts the reactions of every kind, and `reactions` each of them. A renote with nothing of its own is a repost: its note is in `repostOf`, whole; one with words quotes the note named by `quoteId`. `notifications` prints `{"id", "createdAt", "type", "user", "postId", "post", "reaction"}`, a reaction as the `type` `like`. A write prints `{"action", "target", "id", "url", "already"}`: `post` the note it made, `follow` the account followed, with `requested` when the account has to accept. `download` prints one line a file, `{"postId", "type", "url", "path"}`. An error is `{"error", "hint", "status"}`, on stdout as well as stderr.

When a server says it is busy (429, 502, 503, 504), msky asks up to three more times, after its `Retry-After` or after 1, 2 and 4 seconds.

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: a wrong flag or argument, no login where one is needed, a note that cannot be sent as written |
| 2 | `msky` without a command outside an interactive terminal |
| 3 | A file could not be read or written |
| 4 | The server could not be reached or said no, or made no preview for `--dry-run` |

## In the client

In the composer, `ctrl+s` sends, `ctrl+o` attaches up to four pictures or one video (up to 100 MB), `ctrl+w` adds a content warning, and `ctrl+p` chooses who sees the note (public, home, followers). A write is sent once; `D`, `B`, `x`, `!` and logging out wait for `y`. A thread is read ahead when the selection rests on a note, and the Notifications tab counts what is unread. MFM shows its text: `$[x2 大きい]` shows `大きい`, `$[ruby 漢字 かんじ]` shows `漢字(かんじ)`, and a file marked sensitive has a `⚠ sensitive` mark. `s` sets the theme (bsky's themes, `T` previews them), the language, the download folder and the browser in `settings.json`.

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
