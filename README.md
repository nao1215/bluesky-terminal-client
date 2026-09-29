[![Build](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml)
![Coverage](https://raw.githubusercontent.com/nao1215/octocovs-central-repo/main/badges/nao1215/bluesky-terminal-client/coverage.svg)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/bluesky-terminal-client)](https://crates.io/crates/bluesky-terminal-client)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/nao1215/bluesky-terminal-client/badge)](https://scorecard.dev/viewer/?uri=github.com/nao1215/bluesky-terminal-client)
<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

# bsky

A [Bluesky](https://bsky.app) client for the terminal. Pictures and videos are drawn in place.

![the timeline scrolled, then a search](doc/img/demo.gif)

## Install

```sh
brew install nao1215/tap/bsky
cargo install --locked bluesky-terminal-client
cargo binstall bluesky-terminal-client
```

Binaries for Linux, macOS and Windows are on the [releases page](https://github.com/nao1215/bluesky-terminal-client/releases), with the license texts of the crates they contain in `bsky-<version>-THIRD_PARTY_LICENSES.html`.

### Verifying a release

Each release from 0.13.0 on carries `checksums.txt`, a cosign signature of it, and SLSA provenance, and each archive has a GitHub build attestation. In a directory with the downloaded files (`<tag>` is the release, for example `v0.13.0`):

```sh
sha256sum --ignore-missing --check checksums.txt
cosign verify-blob --bundle checksums.txt.sigstore.json \
  --certificate-identity "https://github.com/nao1215/bluesky-terminal-client/.github/workflows/release.yml@refs/tags/<tag>" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  checksums.txt
slsa-verifier verify-artifact <archive> --provenance-path bsky-<tag>.intoto.jsonl \
  --source-uri github.com/nao1215/bluesky-terminal-client --source-tag <tag>
gh attestation verify <archive> --repo nao1215/bluesky-terminal-client
```

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

Chat, accounts and columns show made-up data served by [doc/demo-server.py](doc/demo-server.py); the composer browses the pictures in [doc/demo](doc/demo).

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
| `BSKY_SERVICE` | PDS to log in to (also `--service`) |
| `BSKY_VIDEO_SERVICE` | Video upload service |
| `LANG` | Language, unless one is chosen in the settings |
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

## Contributors

Everyone who reports a bug, suggests a feature, sends a pull request, or packages bsky is listed here ([emoji key](https://allcontributors.org/docs/en/emoji-key)).

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://debimate.jp/"><img src="https://avatars.githubusercontent.com/u/22737008?v=4?s=75" width="75px;" alt="CHIKAMATSU Naohiro"/><br /><sub><b>CHIKAMATSU Naohiro</b></sub></a><br /><a href="https://github.com/nao1215/bluesky-terminal-client/commits?author=nao1215" title="Code">💻</a> <a href="https://github.com/nao1215/bluesky-terminal-client/commits?author=nao1215" title="Documentation">📖</a></td>
    </tr>
  </tbody>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

[MIT](LICENSE). Not made by or affiliated with Bluesky Social PBC.
