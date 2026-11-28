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
just e2e         # every end-to-end suite (just e2e-bsky, just e2e-msky, just e2e-mstdn, just e2e-nstr for one)
just deny        # dependency advisories and licenses (needs cargo-deny)
just coverage    # line coverage of the unit tests and every E2E suite together (needs cargo-llvm-cov)
just release-test  # the release scripts, without a tag
```

`just ci` runs the first five and `just rules`.

## Rules checked by machine

`just rules` runs what `.github/workflows/rules.yml` checks on every pull request. Each script is in `scripts/rules/` and says at its top what it checks and why.

| Rule | Script | When it fails | How to make an exception |
|---|---|---|---|
| No bold, one line per English paragraph, in READMEs, CHANGELOGs and doc/ | `docs.py` | `**` outside code, or a paragraph broken over lines | none |
| A README says what exists now | `docs.py` | "coming soon", "planned", "not yet", "same as" and the like in a README (a CHANGELOG's Unreleased is left alone) | none |
| The root README is one page | `docs.py` | a link from it to a crate's README or to doc/ | none |
| No co-author or session trailers | `trailers.py` | `Co-Authored-By` (a bot's own excepted), `Claude-Session` or a claude.ai/code link in a commit or the PR description | none |
| No lint switched off | `source.py` | `allow` or `expect` of dead_code, unused, unreachable_pub or missing_docs in crates/*/src | none: remove the code, or keep test-only code under `#[cfg(test)]` |
| No dependency left unused | cargo-machete | a dependency no code uses | none |
| No new copy of code | `duplicates.py` | a copy of 10 lines and 100 tokens or more in crates/*/src that `scripts/rules/jscpd-baseline.json` does not list | `python3 scripts/rules/duplicates.py --update`, with the reason in the PR |
| terminal-kit's API changes only on purpose | `public_api.sh` | what terminal-kit lets other crates reach is not `crates/terminal-kit/public-api.txt` | `scripts/rules/public_api.sh --update`, with the reason a client needs it in the PR |
| Every command and flag is listed | `cli.py` | `doc/cli-surface/<client>.txt` is not what `--help` says, or the source hides a command or flag from `--help` | `python3 scripts/rules/cli.py --update surface` |
| A command or flag is added for a use | `cli.py` | an entry added to `doc/cli-surface/<client>.txt` without its `usecase` and `without` in `doc/cli-surface/usecases.toml` | none: say who uses it and what happens without it, or leave it out |
| Every command has an E2E spec | `cli.py` | no atago spec in the crate's `e2e/atago/` runs a command, or a client with a full-screen client has no pty spec for it | an entry in `scripts/rules/e2e-allow.toml` with the reason |

`terminal-kit` also denies `missing_docs`, so clippy fails on an undocumented public item. `just rules-update` writes the three lists again, and the pull request explains each line it changes. `scripts/rules/fast.sh [COMMIT_MESSAGE_FILE]` runs the rules that need no build, for a hook before a commit.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.

## Releasing

Each client has a release workflow: bsky uses `vX.Y.Z` (`release.yml`), msky uses `msky-vX.Y.Z` (`release-msky.yml`), mstdn uses `mstdn-vX.Y.Z` (`release-mstdn.yml`), and nstr uses `nstr-vX.Y.Z` (`release-nstr.yml`), all under `.github/workflows/`. Each workflow accepts only its client's tags. terminal-kit is published to crates.io by whichever release first needs a version of it that is not there yet.

mstdn and nstr remain at 0.1.0 with Unreleased changelogs while their release machinery is prepared. Before either first release, add a non-empty `## [0.1.0] - YYYY-MM-DD` section for the release notes; the tag check rejects an Unreleased-only changelog. Their workflows use msky's compiler pin, target matrix, signing and verification steps, crates.io authentication, and Homebrew tap token. The first publication uses `CARGO_REGISTRY_TOKEN`; later publications can use trusted publishing when configured.
