# Contributing to msky

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
just deny        # dependency advisories and licenses (needs cargo-deny)
```

`just ci` runs all of them but `deny`.

## End-to-end tests

The suite lives in `e2e/atago/*.atago.yaml` and runs with `e2e/run.sh`, which builds the release binary and puts it first on `PATH`. It needs no account and no network:

- The Misskey server is an atago mock server. Each scenario declares the API routes it needs (`POST /api/<endpoint>`), points msky at it with `--host ${api.url}` or `MSKY_HOST`, and asserts what msky sent (`mock:` with `body:` and `count:`).
- The files a note carries are served by the same mock from `e2e/atago/testdata`, and `dir:` and `file:` assert what msky saved.

The suite runs on Linux, macOS, and Windows. Skip a platform only for a reason written in a comment on the scenario.

Write a scenario for every behavior you add or change, including how it fails. Do not point a scenario at a real server.

## Code style

- Comments and documentation in English, explaining why rather than what.
- Errors reach the user as `error:` plus, when there is a next step, `hint:`, with the exit code of their class (see `src/error.rs`).
- Each command lives in `src/cli`, calls the API through `src/api`, supports `--json` with the one note shape of `src/cli/format.rs`, and comes with an E2E scenario in `e2e/atago/`.
- Text others wrote goes through `src/text.rs` before it reaches the terminal.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.
