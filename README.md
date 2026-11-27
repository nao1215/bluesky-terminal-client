[![Build](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# msky

An unofficial [Misskey](https://misskey-hub.net) client for the terminal. It reads what a Misskey server shows to visitors: an account's notes, one note, a search, and the files attached to notes.

```sh
msky author syuilo -n 5
msky note https://misskey.io/notes/au102erst5tf04vs
msky search Misskey --json | jq -r .url
msky download https://misskey.io/notes/au102erst5tf04vs -o pictures
msky author syuilo --with-files -n 50 --download
```

A note as `msky author alice --download` prints it:

```
Alice @alice · 2026-11-26 19:54
山の写真 🏔️
[2 pictures]
:igyo: 12  👍 5  :blobcat@misskey.design: 2
♡ 19  ⟳ 3  ↩ 1
https://misskey.io/notes/9xyz0abc12
saved /home/alice/Downloads/msky/9xyz0abc12-1.jpg
saved /home/alice/Downloads/msky/9xyz0abc12-2.png
```

## Install

```sh
git clone https://github.com/nao1215/misskey-terminal-client
cd misskey-terminal-client
cargo install --locked --path .
```

It needs Rust 1.90 or later.

## Use

msky asks misskey.io unless `--host` or `MSKY_HOST` names another server (`--host misskey.design`, `--host https://misskey.example`). It does not log in, so it shows what the server shows to visitors. A note or an account given by its address is read on the server in the address, whatever `--host` says.

| Command | What it does |
|---|---|
| `msky author USER` | The account's notes, newest first |
| `msky note NOTE` | One note |
| `msky search QUERY` | The notes that match, newest first |
| `msky download NOTE` | Saves the files attached to the note |

An account is `name`, `@name`, `@name@host`, or its address (`https://misskey.io/@name`). A note is its ID or its address (`https://misskey.io/notes/ID`). Either address works without `https://`.

`-n` sets how many notes `author` and `search` print (20 by default). They read them a page of up to 100 at a time, each page older than the last note of the one before, until there are enough or no more.

`author` leaves out the account's replies and renotes. `--replies` and `--renotes` put them in, and `--with-files` keeps only the notes with files (Misskey does not take it with `--replies`). A renote shows the note it renotes, under a line that says who renoted it.

Text that others wrote is printed without its control characters, so an escape sequence in a note or a name cannot set the clipboard or the window title, or clear the screen. A note's reactions are listed ten kinds at most, the most given first; `--json` has them all.

### Saving files

`msky download NOTE` saves each file attached to the note and prints where. `msky author USER --download` saves the files of every note it lists, and `-o DIR` saves them there. The folder is `-o`, else `MSKY_DOWNLOAD_DIR`, else `msky` in your Downloads folder.

A file is named after its note, its place among the note's files, and its type: `9xyz0abc12-1.jpg`. The extension is the one the file was uploaded with when it agrees with the type the server detected, else the usual one for that type, and `.bin` for a type msky does not know, so a picture uploaded as `cat.exe` is not saved as a program. A file already there is never written over: the new one is saved as `9xyz0abc12-1 (1).jpg`. msky saves files up to 200 MB, fetches only http and https addresses, and removes a file it could not finish.

### JSON

`--json` prints one note per line, in the same shape for every command:

```json
{"id":"9xyz0abc12","createdAt":"2026-11-26T10:54:49.672Z","url":"https://misskey.io/notes/9xyz0abc12","uri":null,"user":{"id":"9u0000abcd","username":"alice","host":"misskey.io","name":"Alice"},"text":"山の写真 🏔️","cw":null,"reactionCount":19,"reactions":{":blobcat@misskey.design:":2,":igyo@.:":12,"👍":5},"renoteCount":3,"repliesCount":1,"files":[{"id":"9xyz0f0001","name":"mountain.jpg","url":"https://media.misskey.example/files/mountain.jpg","type":"image/jpeg","size":204800,"isSensitive":false}],"replyId":null,"renoteId":null,"renote":null}
```

`url` is the note on the server asked, `uri` where a note of another server was first posted. `host` is the server the account is on, filled in where Misskey writes null for an account of the server asked. `reactions` keeps the server's keys: `:name@.:` is a custom emoji of the server asked. `renote` is the note a renote or a quote carries, in the same shape. With `--download`, `author` adds `saved`, the paths of the files it saved; `download --json` prints one line per file with `noteId`, `url`, `type`, `size` and `path`. An error is printed as `{"error": ..., "hint": ..., "status": ...}` on stdout as well as on stderr.

### Rate limits

A server that answers it is asked too often (HTTP 429, `RATE_LIMIT_EXCEEDED`) is waited for as its `Retry-After` says, a minute at most, or 1, 2 and then 4 seconds when it says nothing, and asked again, three times at most. msky says on stderr how long it waits. A busy gateway (502, 503, 504) is asked again the same way; any other refusal is reported at once.

## Environment

| Variable | Meaning |
|---|---|
| `MSKY_HOST` | Server to read (also `--host`); misskey.io by default |
| `MSKY_DOWNLOAD_DIR` | Where files are saved when `-o` is not given |
| `TZ` | Time zone the times are shown in |

## Exit status

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Usage error: an unknown flag, a malformed `--host`, something that is not a note or an account |
| 3 | A file could not be written: the output, or a download |
| 4 | The server could not be reached or said no |

## License

[MIT](LICENSE)
