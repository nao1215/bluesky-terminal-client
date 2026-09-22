# bs

[![Build](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/bluesky-terminal-client/actions/workflows/e2e.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

bs is a [Bluesky](https://bsky.app) client for the terminal that draws pictures inline, in the terminal's own graphics protocol, next to the posts they belong to.

- The timeline shows posts written by the accounts you follow, and nothing else: no reposts, no posts of your own, no one you do not follow.
- Avatars and attached photos are drawn at their own shape with kitty graphics, sixel, or iTerm2 inline images.
- Post, reply, and like; search posts and accounts; follow and unfollow; edit your display name, description, and avatar.
- A terminal that cannot draw images is refused at startup instead of giving you a client that silently drops every picture.

## Requirements

A terminal with an image protocol:

| Protocol | Terminals |
|----------|-----------|
| kitty graphics (unicode placeholders) | kitty, Ghostty |
| sixel | foot, mlterm, Windows Terminal 1.22+, xterm started with `-ti vt340` |
| iTerm2 inline images | iTerm2, WezTerm |

bs asks the terminal which protocols it speaks. When the terminal answers none of them, bs exits with status 2:

```console
$ bs
error: this terminal cannot display images
hint: use a terminal with kitty graphics, sixel, or iTerm2 inline images (kitty, Ghostty, WezTerm, foot, iTerm2, ...), or set BS_GRAPHICS when yours supports one but does not answer the query
```

Some multiplexers pass images through but do not pass the question along. For those, name the protocol yourself with `BS_GRAPHICS=kitty`, `sixel`, or `iterm2`.

## Installation

```sh
cargo install --locked --git https://github.com/nao1215/bluesky-terminal-client
```

The package is `bluesky-terminal-client`; the command it installs is `bs`. Rust 1.90 or later is required.

## Getting started

Create an app password in the Bluesky app (Settings, Privacy and security, App passwords), then run:

```sh
bs
```

The first start asks for your handle (or email) and the app password. bs never stores the password: it keeps the session tokens the server returns, in `session.json` in the config directory, readable only by you on Unix. `bs logout` removes it.

To use a PDS other than bsky.social:

```sh
bs --service https://pds.example.com
```

## Keys

| Key | Action |
|-----|--------|
| `1` `2` `3`, `Tab` `Shift+Tab` | Timeline, Search, Profile |
| `j` `k`, `↓` `↑` | Move the selection (`g` / `G` for top and bottom) |
| `n` | New post |
| `r` | Reply to the selected post |
| `l` | Like, or remove your like |
| `f` | Follow or unfollow the selected account (the author of the selected post, or the profile shown) |
| `Enter` | Open the profile of the selected account or post author |
| `/` | Search; `Ctrl+T` in the box, or `t` on the Search tab, switches between posts and accounts |
| `e` | Edit your profile (Profile tab) |
| `R`, `F5` | Refresh the current view |
| `Esc` | Leave the search box, close a window, return to your own profile |
| `Ctrl+S` | Send the post, save the profile |
| `?` | Help |
| `q`, `Ctrl+C` | Quit |

Links, mentions, and hashtags in posts you write become clickable: bs finds them in the text and sends the rich-text facets Bluesky needs. A mention whose handle does not resolve stays plain text. The composer counts characters the way Bluesky does and refuses to send more than 300.

## Configuration

| Variable | Meaning |
|----------|---------|
| `BS_SERVICE` | PDS to log in to, like `--service` (default `https://bsky.social`) |
| `BS_CONFIG_DIR` | Directory for `session.json` (default: `bs` in the platform config directory, `$XDG_CONFIG_HOME/bs` on Linux) |
| `BS_GRAPHICS` | Force an image protocol: `kitty`, `sixel`, or `iterm2` |

## Exit status

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Usage error: unknown flag, malformed `--service`, unknown `BS_GRAPHICS` |
| 2 | The terminal is unsupported: stdin or stdout is not a terminal, or there is no image protocol |
| 3 | A local file could not be read or written, such as a corrupt `session.json` |

Errors are printed as `error:` and, where there is a next step, `hint:`. Network and server errors do not end the client: they are shown in it, and the action can be retried.

## Development

```sh
just test      # unit tests
just lint      # clippy
just e2e       # end-to-end suite (needs atago)
```

The end-to-end suite in [e2e/atago](e2e/atago) runs the real binary with [atago](https://github.com/nao1215/atago). It drives bs in a pseudo-terminal that answers like a kitty-graphics terminal and stubs the Bluesky API with mock servers, so it asserts, offline, what is drawn on the screen (including the images, compared pixel for pixel), what bs sends to the server, and what it writes to disk. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
