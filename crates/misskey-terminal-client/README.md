[![Build](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml)
[![Trivy](https://github.com/nao1215/social-terminal-clients/actions/workflows/trivy.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/trivy.yml)
![Coverage](https://raw.githubusercontent.com/nao1215/octocovs-central-repo/main/badges/nao1215/social-terminal-clients/coverage.svg)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/nao1215/social-terminal-clients/badge)](https://scorecard.dev/viewer/?uri=github.com/nao1215/social-terminal-clients)
<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

# msky

A [Misskey](https://misskey-hub.net) client for the terminal. Pictures and videos are drawn in place.

![the timeline, a note's video and pictures, a thread, a profile and the notifications](doc/img/demo.gif)

It is written for Misskey itself; forks such as Sharkey may work, but they are not tested.

## Install

```sh
brew install nao1215/tap/msky
cargo install --locked misskey-terminal-client
cargo binstall misskey-terminal-client
```

Binaries for Linux, macOS and Windows of 0.1.1 and earlier are on the [releases page of nao1215/misskey-terminal-client](https://github.com/nao1215/misskey-terminal-client/releases), tagged `v<version>`. Releases made from this repository are tagged `msky-v<version>` on its [releases page](https://github.com/nao1215/social-terminal-clients/releases). Each carries the license texts of the crates its binaries contain in `msky-v<version>-THIRD_PARTY_LICENSES.html`. Building it needs Rust 1.90 or later.

### Verifying a release

Each release carries `checksums.txt`, a cosign signature of it, and SLSA provenance, and each archive has a GitHub build attestation. In a directory with the downloaded files of a release made from this repository (`<version>` is its version, `<archive>` one of its archives):

```sh
sha256sum --ignore-missing --check checksums.txt
cosign verify-blob --bundle checksums.txt.sigstore.json \
  --certificate-identity "https://github.com/nao1215/social-terminal-clients/.github/workflows/release-msky.yml@refs/tags/msky-v<version>" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  checksums.txt
slsa-verifier verify-artifact <archive> --provenance-path msky-v<version>.intoto.jsonl \
  --source-uri github.com/nao1215/social-terminal-clients --source-tag msky-v<version>
gh attestation verify <archive> --repo nao1215/social-terminal-clients
```

A release of 0.1.1 or earlier is checked the same way with `nao1215/misskey-terminal-client` in place of `nao1215/social-terminal-clients`, `release.yml` in place of `release-msky.yml`, and the tag `v<version>` in place of `msky-v<version>`; its provenance is in `msky-v<version>.intoto.jsonl` too.

## Use

Run `msky`, pick a server, and allow msky in the browser that opens. MiAuth logs you in, so msky never sees your password, and `esc` reads the server as a visitor. `?` lists every key, `.` what the keys do to the selected note or account.

| Pictures with | Terminals |
|---|---|
| kitty graphics | kitty, Ghostty |
| sixel | foot, mlterm, Windows Terminal 1.22+ |
| iTerm2 images | iTerm2, WezTerm |

Other terminals get text, and `space` opens a note in the browser. Custom emoji are drawn as pictures. MP4 videos with H.264 play without sound; `d` saves other formats and `o` opens them. MFM is shown as plain text with bold kept, and `w` opens a content warning. Photos and videos are posted without their location data.

msky speaks English and 日本語: the language of your system, or the one chosen in the settings (`s`).

## Screens

| `space`: pictures and videos | `n` `ctrl+o`: attach pictures or a video |
|:--:|:--:|
| ![a note's pictures and a video, full screen](doc/img/viewer.gif) | ![the picture browser of the composer](doc/img/compose.png) |
| `v`: thread | `/`: search |
| ![a thread](doc/img/thread.png) | ![accounts found for a search](doc/img/search.png) |
| `.`: actions | `e`: edit your profile |
| ![the list of what the keys do to a note](doc/img/actions.png) | ![the profile editor](doc/img/editor.png) |
| `7`: chat | `A`: accounts |
| ![a conversation](doc/img/chat.png) | ![the account list](doc/img/accounts.png) |
| `s`: settings | |
| ![the settings screen](doc/img/settings.png) | |

`+` on the Home tab: your antennas, lists, channels, notifications or a search in columns beside it

![an antenna added beside the home timeline, then scrolled](doc/img/columns.gif)

`T`: 42 themes

| | | |
|:--:|:--:|:--:|
| ![dracula](doc/img/theme-dracula.png) | ![nord](doc/img/theme-nord.png) | ![gruvbox](doc/img/theme-gruvbox.png) |
| ![catppuccin-latte](doc/img/theme-catppuccin-latte.png) | ![tokyo-night](doc/img/theme-tokyo-night.png) | ![solarized-light](doc/img/theme-solarized-light.png) |

The screens show made-up notes served by [doc/demo-server.py](doc/demo-server.py), with the author's photos in [doc/demo](doc/demo).

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

## Other terminal clients

From their READMEs, November 2026.

| | msky | [misskey-cli](https://github.com/mikuta0407/misskey-cli) | [Misskey-TUI](https://github.com/yulog/misskey-tui) |
|---|---|---|---|
| Written in | Rust | Go | Go |
| Interface | Full screen, and commands | Commands | Full screen |
| Pictures and videos | Drawn in the terminal | Marked as attached | Not shown |
| Timelines | Home, local, global, social (commands) | Home, local, global, streaming | Home, local, social, global |
| Several columns | Yes | Not applicable | No |
| Several servers | Yes | Yes | No |
| Notifications | Yes | No | No |
| Chat | Yes | No | No |
| Posting | Text, pictures, video, reply, quote, content warning | Text, reply, renote, delete | Text, reply, renote |
| Follow, mute and block | Yes | No | No |
| Search | Notes and accounts | No | No |
| JSON output | Every command | No | Not applicable |
| Login | MiAuth in the browser | Access token in a TOML file | Access token in a JSON file |

## Related projects

- [bsky](../bluesky-terminal-client/README.md): a Bluesky client for the terminal, in the same repository.

## Contributing

```sh
just test    # unit tests
just e2e     # end-to-end tests (atago)
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Contributors

Everyone who reports a bug, suggests a feature, sends a pull request, or packages msky is listed here ([emoji key](https://allcontributors.org/docs/en/emoji-key)).

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://debimate.jp/"><img src="https://avatars.githubusercontent.com/u/22737008?v=4?s=75" width="75px;" alt="CHIKAMATSU Naohiro"/><br /><sub><b>CHIKAMATSU Naohiro</b></sub></a><br /><a href="https://github.com/nao1215/misskey-terminal-client/commits?author=nao1215" title="Code">💻</a> <a href="https://github.com/nao1215/misskey-terminal-client/commits?author=nao1215" title="Documentation">📖</a></td>
    </tr>
  </tbody>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

[MIT](LICENSE). Not made by or affiliated with the Misskey project.
