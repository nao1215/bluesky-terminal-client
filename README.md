[![Build](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/misskey-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# msky

A [Misskey](https://misskey-hub.net) client for the terminal. Pictures and videos are drawn in place.

![the timeline, a note's video and pictures, a thread, a profile and the notifications](doc/img/demo.gif)

msky has the same keys as [bsky](https://github.com/nao1215/bluesky-terminal-client), its Bluesky sibling. It is written for Misskey itself; forks such as Sharkey may work, but they are not tested.

## Install

```sh
git clone https://github.com/nao1215/misskey-terminal-client
cd misskey-terminal-client
cargo install --locked --path .
```

It needs Rust 1.90 or later.

## Use

Run `msky`, pick a server, and allow msky in the browser that opens. MiAuth logs you in, so msky never sees your password, and `esc` reads the server as a visitor. `?` lists every key, `.` what the keys do to the selected note or account.

| Pictures with | Terminals |
|---|---|
| kitty graphics | kitty, Ghostty |
| sixel | foot, mlterm, Windows Terminal 1.22+ |
| iTerm2 images | iTerm2, WezTerm |

Other terminals get text, and `space` opens a note in the browser. Custom emoji are drawn as pictures. MP4 videos with H.264 play without sound; `d` saves other formats and `o` opens them. MFM is shown as plain text with bold kept, and `w` opens a content warning. Photos and videos are posted without their location data.

msky speaks English and 日本語: the language of your system, or the one chosen in the settings (`s`).

`+` on the Home tab: your antennas, lists, channels, notifications or a search in columns beside it

![an antenna and the notifications added beside the home timeline, then scrolled](doc/img/columns.gif)

The demos show made-up notes served by [doc/demo-server.py](doc/demo-server.py), with the author's photos in [doc/demo](doc/demo).

## Keys

| Key | Action |
|---|---|
| `1`–`7` | Home, Local, Global, Notifications, Search, Profile, Chat |
| `+` `x` | Add a column, remove one |
| `j` `k` | Move |
| `n` `r` `Q` | Note, reply, quote |
| `l` `e` `b` | React ❤, choose a reaction, renote |
| `f` `M` `B` | Follow, mute, block |
| `D` `!` | Delete your note, report |
| `v` `space` `w` | Thread, pictures or video, content warning |
| `d` `o` `c` | Save files, open in the browser, copy the address |
| `/` | Search |
| `m` | Message the profile shown |
| `e` on your profile | Edit your profile |
| `A` `T` `s` | Accounts, themes, settings |
| `?` `q` | Help, quit |

The Chat tab is hidden on servers without chat, such as misskey.io.

## Commands

```sh
msky timeline -n 5
msky post "hello" --image cat.jpg
msky search Misskey --json | jq -r .url
```

`msky --help` lists them all. [doc/commands.md](doc/commands.md) covers logging in and its permissions, posting from CI, saving files, the JSON shape and the exit codes.

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
| `LANG` | Language, unless one is chosen in the settings |
| `NO_COLOR` | No color |

## Contributing

```sh
just test    # unit tests
just e2e     # end-to-end tests (atago)
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Not made by or affiliated with the Misskey project.
