# Contributing to bsky

## Prerequisites

- Rust stable (`rustup default stable`); the minimum supported version is the `rust-version` in `Cargo.toml`
- [just](https://github.com/casey/just) (`cargo install just`)
- [atago](https://github.com/nao1215/atago) for the end-to-end suite (`go install github.com/nao1215/atago@latest`)
- [himorime](https://github.com/nao1215/himorime) and python3 for the benchmarks (`go install github.com/nao1215/himorime@latest`)

## Checks

```sh
just test        # unit tests
just lint        # clippy with warnings as errors
just fmt-check   # formatting
just doc         # rustdoc with warnings as errors
just e2e         # end-to-end suite
just bench       # performance with himorime (see bench/README.md)
just coverage    # line coverage of the unit tests and the E2E suite together (needs cargo-llvm-cov)
```

`just ci` runs all of them except the benchmarks and coverage.

## End-to-end tests

The suite lives in `e2e/atago/*.atago.yaml` and runs with `e2e/run.sh`, which builds the release binary and puts it first on `PATH`. It needs no account and no network:

- The Bluesky PDS is an atago mock server. Each scenario declares the XRPC routes it needs and asserts what bsky sent (`mock:` with `body:`, `header:`, and `query:`).
- The terminal is an atago pseudo-terminal. `graphics: kitty` makes it answer like a terminal that draws images, and `screen.images` asserts what bsky drew, down to the pixels. Leaving `graphics` out is how a terminal without image support is tested.
- The session file is written with a `fixture:` step, and `changes:` and `file:` assert what bsky wrote to disk.

The suite runs on Linux, macOS, and Windows (on Windows atago draws `graphics: kitty` through the OpenConsole it ships). Skip a platform only for a reason written in a comment on the scenario, such as a stand-in program that only runs there (write the other platform's version as a scenario with `only:`).

Write a scenario for every behavior you add or change, including how it fails. Wait for the screen with `expect_screen` before sending the next key, and send a key after `Esc` only once the screen has changed, or the terminal reads the pair as `Alt` plus the key.

## Benchmarks

`bench/himorime.yaml` measures start-up, the commands, and the client from start to its first post, against a stand-in server. Every pull request is compared with its base by `.github/workflows/bench.yml`; see [bench/README.md](bench/README.md).

## Screenshots

The pictures in the README are recorded with `doc/record-demo.sh` (kitty, xdotool and ffmpeg on X11). The chat, accounts and columns pictures use `doc/demo-server.py`, a stand-in server with made-up accounts, so no real account's messages are shown.

## Code style

- Comments and documentation in English, explaining why rather than what.
- Errors reach the user as `error:` plus, when there is a next step, `hint:`, with the exit code of their class (see `src/error.rs`).
- The client is what `bsky` opens, and its behavior belongs in keys, not flags. The commands (`bsky timeline`, `bsky post`...) are for scripts: each is one thing a script needs from Bluesky, lives in `src/cli`, calls the same API, account and post code as the client, supports `--json`, and comes with an E2E scenario in `e2e/atago/` (most are in `cli.atago.yaml`). A command that only repeats a key of the client without being useful to a script does not belong.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.

## Releasing

A pushed tag `vX.Y.Z` runs `.github/workflows/release.yml`: it checks the tag, builds every target in `.github/release-targets.json` with the pinned `RUST_TOOLCHAIN` and no build cache, packs reproducible archives, writes `checksums.txt` and a CycloneDX SBOM, attests the build provenance of the archives and the SBOM, creates the GitHub release with the CHANGELOG section as its text, publishes the crate, and updates the formula in nao1215/homebrew-tap.

1. In a pull request, set `version` in `Cargo.toml` to `X.Y.Z`, rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD`, and add an empty `## [Unreleased]` above it. `just release-test` checks the release scripts.
2. Merge it, then tag the merge commit and push the tag: `git tag vX.Y.Z` and `git push origin vX.Y.Z`.

The workflow stops before building if the tag is not `v` plus the `Cargo.toml` version, the pinned compiler is older than `rust-version`, or the CHANGELOG has no section for the version. A tag with a hyphen (`v0.2.0-rc.1`) makes a pre-release and skips crates.io and Homebrew. Publishing to crates.io uses `CARGO_REGISTRY_TOKEN` when it is set, else trusted publishing (the first publish needs the token). The formula is updated only when `HOMEBREW_TAP_GITHUB_TOKEN` is set.
