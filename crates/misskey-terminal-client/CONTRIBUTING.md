# Contributing to msky

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
just deny        # dependency advisories and licenses (needs cargo-deny)
just bench       # performance with himorime (see bench/README.md)
just coverage    # line coverage of the unit tests and the E2E suite together (needs cargo-llvm-cov)
just fuzz <target> -max_total_time=60   # one fuzz target (needs nightly and cargo-fuzz)
just release-test                       # the release scripts, without a tag
```

`just ci` runs the first five.

## End-to-end tests

The suite lives in `e2e/atago/*.atago.yaml` and runs with `e2e/run.sh`, which builds the release binary and puts it first on `PATH`. It needs no account and no network:

- The Misskey server is an atago mock server. Each scenario declares the API routes it needs (`POST /api/<endpoint>`), points msky at it with `--host ${api.url}` or `MSKY_HOST`, and asserts what msky sent (`mock:` with `body:` and `count:`).
- The files a note carries are served by the same mock from `e2e/atago/testdata`, and `dir:` and `file:` assert what msky saved.

The suite runs on Linux, macOS, and Windows. Skip a platform only for a reason written in a comment on the scenario.

Write a scenario for every behavior you add or change, including how it fails. Do not point a scenario at a real server.

## Benchmarks

`bench/himorime.yaml` measures start-up, the commands, and the client from start to its first note, against a stand-in server. Every pull request is compared with its base by `.github/workflows/bench.yml`; see [bench/README.md](bench/README.md).

## Fuzzing

`fuzz/` holds cargo-fuzz targets for the code that reads untrusted input: `mfm_links` (the links in a note's MFM, its plain text, and silencing its links), `metadata_strip` (taking the metadata off a file before it is uploaded), `mp4_track` (the video track of an MP4 a note carries), and `note_json` (a server's notes, accounts and notifications). Each checks properties, not only that nothing panics: links fall on character boundaries without overlapping and a silenced note gets no preview, a stripped video keeps its length and a second pass changes nothing, every sample of a track is Annex B, and an answer reads the same from its bytes as from its JSON value.

msky has no library target, so a target includes the file it tests with `#[path = "../../src/..."] mod ...;`. Only a file that uses nothing else from the crate can be included that way (`src/metadata.rs` uses `crate::error`, which the target includes beside it); keep `src/mfm.rs`, `src/metadata.rs`, `src/error.rs`, `src/tui/mp4.rs` and `src/api/types.rs` free of other `use crate::`.

Fuzzing needs nightly and cargo-fuzz (`rustup toolchain install nightly`, `cargo install --locked cargo-fuzz`). `just fuzz mp4_track -max_total_time=60` runs one target; new inputs go to the ignored `fuzz/work/`, and `fuzz/corpus/<target>/` holds the committed seeds. Pull requests that touch those files or `fuzz/` build the targets and run each over its seeds; `.github/workflows/fuzz.yml` explores every night. When it finds a crash, add the input to `fuzz/corpus/<target>/` and to a unit test before fixing it.

## Properties

`src/properties.rs` checks properties on generated text with proptest: wrapped lines fit their width and keep every character, a cut is the start of the text, the text field stays whole whatever is typed or pasted, and silencing a note's links leaves none with a preview and the same text to read. `cargo test` runs them with 256 cases each; `.github/workflows/properties.yml` runs them each night with 500,000 (`PROPTEST_CASES=500000 cargo test --release --bin msky properties` does the same here). When one fails, add the smallest input it prints as a unit test next to the code it breaks before fixing it.

## Mutation testing

`.github/workflows/mutation.yml` runs [cargo-mutants](https://mutants.rs) over the files `.cargo/mutants.toml` names: on a pull request over the lines it changes, and every week over all of them in eight shards. A mutant changes one operator, constant or return value; it is caught when a unit test fails. The run fails when the share caught falls below `MUTATION_MIN_SCORE`, the score of the last full run, so a change cannot leave the tests weaker. The job summary lists the mutants missed: add the test that catches one, and raise `MUTATION_MIN_SCORE` when the score goes up. `cargo mutants --in-diff <(git diff main -- src)` checks your own change before you push.

## Code style

- Comments and documentation in English, explaining why rather than what.
- Errors reach the user as `error:` plus, when there is a next step, `hint:`, with the exit code of their class (see `src/error.rs`).
- Each command lives in `src/cli`, calls the API through `src/api`, supports `--json` with the one note shape of `src/cli/format.rs`, and comes with an E2E scenario in `e2e/atago/`.
- Text others wrote goes through `src/text.rs` before it reaches the terminal.

## Commits and pull requests

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/). Pull requests describe the change and how it was tested.

## Releasing

A pushed tag `vX.Y.Z` runs `.github/workflows/release.yml`: it checks the tag, builds every target in `.github/release-targets.json` with the pinned `RUST_TOOLCHAIN` and no build cache, packs reproducible archives, writes the third-party license texts (`cargo about`, from `about.toml` and `about.hbs`), `checksums.txt` and a CycloneDX SBOM, attests the build provenance of the archives and the SBOM, signs `checksums.txt` with cosign, creates the GitHub release with the CHANGELOG section as its text, adds SLSA provenance, verifies the published release, and then publishes the crate and updates the formula in nao1215/homebrew-tap. Nothing else starts it: no branch push, pull request or schedule publishes anything.

Before a tag, two workflows run the same steps on pull requests and on main. `.github/workflows/release-smoke.yml` builds and packs every target with the pinned compiler, checks the glibc ceiling, then downloads each archive on a fresh runner and runs `msky --version`. `.github/workflows/licenses.yml` checks the dependency licenses with cargo-deny and writes the third-party license texts with cargo-about, so a crate the release could not ship fails review instead of the release.

1. In a pull request, set `version` in `Cargo.toml` to `X.Y.Z`, rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD`, and add an empty `## [Unreleased]` above it. `just release-test` checks the release scripts.
2. Merge it, then tag the merge commit and push the tag: `git tag vX.Y.Z` and `git push origin vX.Y.Z`.

The workflow stops before building if the tag is not `v` plus the `Cargo.toml` version, the pinned compiler is older than `rust-version`, or the CHANGELOG has no section for the version. A tag with a hyphen (`v0.2.0-rc.1`) makes a pre-release and skips crates.io and Homebrew. Publishing to crates.io uses `CARGO_REGISTRY_TOKEN` when it is set, else trusted publishing (the first publish needs the token). The formula is updated only when `HOMEBREW_TAP_GITHUB_TOKEN` is set.
