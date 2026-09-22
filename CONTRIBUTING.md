# Contributing to bsky

## Prerequisites

- Rust stable (`rustup default stable`); the minimum supported version is the `rust-version` in `Cargo.toml`
- [just](https://github.com/casey/just) (`cargo install just`)
- [atago](https://github.com/nao1215/atago) for the end-to-end suite (`go install github.com/nao1215/atago@latest`)

## Checks

```sh
just test        # unit tests
just lint        # clippy with warnings as errors
just fmt-check   # formatting
just doc         # rustdoc with warnings as errors
just e2e         # end-to-end suite
```

`just ci` runs all of them.

## End-to-end tests

The suite lives in `e2e/atago/*.atago.yaml` and runs with `e2e/run.sh`, which builds the release binary and puts it first on `PATH`. It needs no account and no network:

- The Bluesky PDS is an atago mock server. Each scenario declares the XRPC routes it needs and asserts what bsky sent (`mock:` with `body:`, `header:`, and `query:`).
- The terminal is an atago pseudo-terminal. `graphics: kitty` makes it answer like a terminal that draws images, and `screen.images` asserts what bsky drew, down to the pixels. Leaving `graphics` out is how a terminal without image support is tested.
- The session file is written with a `fixture:` step, and `changes:` and `file:` assert what bsky wrote to disk.

The suite runs on Linux, macOS, and Windows. On Windows the pty is a ConPTY, which does not pass kitty graphics through (nao1215/atago#676), so the scenarios that need `graphics: kitty` skip Windows with a comment saying so; the terminal checks and the rest run there. Add a Windows skip only for that reason, with the same comment.

Write a scenario for every behavior you add or change, including how it fails. Wait for the screen with `expect_screen` before sending the next key, and send a key after `Esc` only once the screen has changed, or the terminal reads the pair as `Alt` plus the key.

## Code style

- Comments and documentation in English, explaining why rather than what.
- Errors reach the user as `error:` plus, when there is a next step, `hint:`, with the exit code of their class (see `src/error.rs`).
- Keep the command-line surface small: behavior belongs in the client, not in new subcommands or flags.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.
