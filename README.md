[![Build](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml)
![Coverage](https://raw.githubusercontent.com/nao1215/octocovs-central-repo/main/badges/nao1215/bluesky-terminal-client/coverage.svg)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

# bsky

A [Bluesky](https://bsky.app) client for the terminal. Pictures and videos are drawn in place.

![the timeline scrolled, then a search](doc/img/demo.gif)

## Install

```sh
brew install nao1215/tap/bsky
cargo install --locked bluesky-terminal-client
```

Binaries for Linux, macOS and Windows are on the [releases page](https://github.com/nao1215/bluesky-terminal-client/releases).

## Use

Run `bsky` and log in. An [app password](https://bsky.app/settings/app-passwords) is safer than your password. `?` lists every key, `.` what the keys do to the selected post.

| Pictures with | Terminals |
|---|---|
| kitty graphics | kitty, Ghostty |
| sixel | foot, mlterm, Windows Terminal 1.22+ |
| iTerm2 images | iTerm2, WezTerm |

Other terminals get text, and `Space` opens a post in the browser. Photos and videos are posted without their location data.

bsky speaks English, 日本語, 简体中文, 한국어, Русский, Español, Français, Deutsch and Português: the language of your system, or the one chosen in the settings (`s` on your profile).

## Screens

| `Space`: pictures and videos | `n` `Ctrl+O`: attach pictures or a video |
|:--:|:--:|
| ![a picture and a video, full screen](doc/img/viewer.gif) | ![the picture browser of the composer](doc/img/compose.png) |
| `v`: thread | `/`: search |
| ![a thread](doc/img/thread.png) | ![accounts found for a search](doc/img/search.png) |
| `.`: actions | `e`: edit your profile |
| ![the list of what the keys do to a post](doc/img/actions.png) | ![the profile editor](doc/img/editor.png) |
| `2`: chat | `A`: accounts |
| ![a conversation](doc/img/chat.png) | ![the account list](doc/img/accounts.png) |
| `s`: settings | text only |
| ![the settings screen](doc/img/settings.png) | ![the timeline without pictures](doc/img/text.png) |

`+` on the timeline: your pinned feeds, notifications or a search in columns beside it

![a feed and notifications added beside the timeline, then scrolled](doc/img/columns.gif)

`T`: 42 themes

| | | |
|:--:|:--:|:--:|
| ![bluesky](doc/img/theme-bluesky.png) | ![bluesky-light](doc/img/theme-bluesky-light.png) | ![dracula](doc/img/theme-dracula.png) |
| ![nord](doc/img/theme-nord.png) | ![gruvbox](doc/img/theme-gruvbox.png) | ![catppuccin-latte](doc/img/theme-catppuccin-latte.png) |

Chat, accounts, columns and the composer show made-up data and the pictures of [doc/demo](doc/demo), served by [doc/demo-server.py](doc/demo-server.py).

## Keys

| Key | Action |
|---|---|
| `1`–`5` | Timeline, Chat, Search, Notifications, Profile |
| `+` `x` | Add a column (a pinned feed, notifications, a search), remove one |
| `j` `k` | Move |
| `n` `r` `Q` | Post, reply, quote |
| `l` `b` `f` | Like, repost, follow |
| `M` `B` | Mute, block |
| `D` | Delete your post |
| `v` `Space` `o` | Thread, pictures or video, link |
| `/` | Search |
| `m` | Message the profile shown |
| `A` `T` `s` | Accounts, themes, settings |
| `?` `q` | Help, quit |

## Commands

```sh
bsky tl -n 5
bsky post "hello" --image cat.jpg
bsky search rust --json | jq -r .uri
```

`bsky --help` lists them all.

## Environment

| Variable | Meaning |
|---|---|
| `BSKY_ACCOUNT` | Account for this run (also `-a`) |
| `BSKY_CONFIG_DIR` | Config folder |
| `BSKY_CACHE_DIR` | Picture cache; `off` for none |
| `BSKY_DOWNLOAD_DIR` | Where `d` saves |
| `BSKY_GRAPHICS` | `kitty`, `sixel` or `iterm2` |
| `BSKY_BROWSER` | Program that opens links |
| `BSKY_SERVICE` | PDS (also `--service`) |
| `BSKY_VIDEO_SERVICE` | Video upload service |
| `NO_COLOR` | No color |

## Other terminal clients

From their READMEs and source, September 2026.

| | bsky | [tuisky](https://github.com/sugyan/tuisky) | [mattn/bsky](https://github.com/mattn/bsky) |
|---|---|---|---|
| Written in | Rust | Rust | Go |
| Interface | Full screen, and commands | Full screen | Commands |
| Pictures and videos | Drawn in the terminal | Links to the browser | Not shown |
| Several columns | Yes | Yes | Not applicable |
| Several accounts | Yes | Yes | Yes |
| Refreshes by itself | Chat only | Yes | `stream` |
| Notifications | Yes | No | Yes |
| Direct messages | Yes | No | Yes |
| Posting | Text, pictures, video, reply, quote | Text, pictures, quote | Text, pictures, video, reply, quote |
| Mute and block | Yes | No | Yes |
| Lists, report, app passwords | Yes | No | Yes |
| Invite codes | No | No | Yes |
| JSON output | Every command | No | Most commands |
| Key bindings | Fixed | Set in TOML | Not applicable |
| MCP server | No | No | Yes |

## Contributing

```sh
just test    # unit tests
just e2e     # end-to-end tests (atago)
just bench   # performance (himorime)
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Not made by or affiliated with Bluesky Social PBC.
