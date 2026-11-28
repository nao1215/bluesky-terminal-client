# Contributing

This repository is a Cargo workspace. Each client is a crate in `crates/` with its own CONTRIBUTING.md, which covers its tests, benchmarks, fuzz targets and releases:

- bsky: [crates/bluesky-terminal-client/CONTRIBUTING.md](crates/bluesky-terminal-client/CONTRIBUTING.md)
- msky: [crates/misskey-terminal-client/CONTRIBUTING.md](crates/misskey-terminal-client/CONTRIBUTING.md)

`crates/terminal-kit` holds the terminal drawing code both clients use. The workflows, `deny.toml`, `about.toml`, `clippy.toml`, `.cargo/mutants.toml` and `scripts/` are shared by the workspace and stay at the repository root.

## Prerequisites

- Rust stable (`rustup default stable`); the minimum supported version is the `rust-version` in each crate's `Cargo.toml`
- [just](https://github.com/casey/just) (`cargo install just`)
- [atago](https://github.com/nao1215/atago) for the end-to-end suites (`go install github.com/nao1215/atago@latest`)
- [himorime](https://github.com/nao1215/himorime) and python3 for the benchmarks (`go install github.com/nao1215/himorime@latest`)

## Checks

At the repository root, these act on the whole workspace:

```sh
just test        # unit tests of every crate
just lint        # clippy with warnings as errors
just fmt-check   # formatting
just doc         # rustdoc with warnings as errors
just e2e         # every end-to-end suite (just e2e-bsky, just e2e-msky, just e2e-mstdn for one)
just deny        # dependency advisories and licenses (needs cargo-deny)
just coverage    # line coverage of the unit tests and every E2E suite together (needs cargo-llvm-cov)
just release-test  # the release scripts, without a tag
```

`just ci` runs the first five.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.

## Releasing

Each client is released on its own tag: bsky on `vX.Y.Z` (`.github/workflows/release.yml`) and msky on `msky-vX.Y.Z` (`.github/workflows/release-msky.yml`). Neither workflow starts on the other's tags. The steps are in each client's CONTRIBUTING.md. terminal-kit is published to crates.io by whichever release first needs a version of it that is not there yet.
