# Testing

Each technique here is used because it catches a kind of bug the others miss. This page says what each one runs on, where its tests live, and how CI runs it. The commands for the checks a pull request needs are in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Unit tests

The unit tests sit beside the code they test, in a `#[cfg(test)] mod tests` at the end of each file (or a `tests/` folder next to it for `src/tui/app` and `src/tui/view`). `App` holds no terminal and no network, so a key press, a page of posts arriving late or a failed write is a function call; what the screen shows is drawn into ratatui's `TestBackend`. Sets that have to be covered case by case, such as the punctuation that ends a link, are `rstest` tables. CI runs them with `cargo test --locked --all-targets` on Linux, macOS and Windows.

## State-transition tests

[src/tui/view/state_fuzz.rs](../src/tui/view/state_fuzz.rs) drives `App` with random keys and with answers from a stand-in worker that arrive late, out of order or not at all, thousands of steps per seed. After every step it checks that nothing panicked, that every selection is inside its list, that one key never sent two writes, and that the screen draws at any terminal size. A failure prints the seed and the step, which replay it. `cargo test` runs 12 seeds; the `Nightly properties` workflow runs 2,000.

## End-to-end tests

`e2e/atago/` holds [atago](https://github.com/nao1215/atago) specs that run the real binary in a pseudo-terminal that answers like a kitty-graphics terminal, against atago mock servers in place of Bluesky. They check what bsky drew, down to the pixels of a picture, what it sent to the server, and what it wrote to disk. `e2e/run.sh` builds the `e2e` profile and runs every spec; the `E2E` workflow runs the suite on Linux, macOS and Windows.

## Property-based tests

[proptest](https://github.com/proptest-rs/proptest) generates the inputs and checks a property every answer has to satisfy. The text is drawn from pieces that are hard to lay out or to count: ZWJ families, flags, keycaps, emoji with VS16 or a skin tone, combining marks, CJK and full-width spaces, half-width kana with sound marks, Arabic ligatures, tabs and control characters.

- `mod properties` at the end of [src/tui/text.rs](../src/tui/text.rs) covers the layout of text in cells: the clusters are the ones unicode-segmentation finds, a wrapped line is never wider than its column (unless one cluster is), wrapping drops nothing but spaces, and a cut keeps a prefix or a suffix of whole clusters within the width.
- `mod properties` at the end of [src/api/facets.rs](../src/api/facets.rs) covers the facets of a new post: every range is a separate slice of the text on character boundaries, and the bytes it covers are what it points at (the URL, `@` and the handle, `#` and the tag).

## Metamorphic tests

The same modules check how two runs relate, which can be said exactly where the right answer for one input cannot: two paragraphs wrap as each does on its own, a line that already fits wraps to itself, one more column never keeps less of a cut, and text joined by a space has the facets of each part, the second part's moved by the bytes before it.

Property and metamorphic tests run in every `cargo test` with proptest's default of 256 cases each and a fresh seed each run; a failure prints the minimal input it shrank to. The `Nightly properties` workflow runs them in release mode with 20,000 cases each. To run them that way locally:

```sh
PROPTEST_CASES=20000 cargo test --release --bins properties
BSKY_FUZZ_SEEDS=2000 cargo test --release --bins random_keys_and_late_answers_keep_the_client_sound
```

A failure becomes a plain unit test with the input proptest printed, fixed on its own branch.

## Mutation tests

[cargo-mutants](https://mutants.rs) rewrites one operator, constant or return value at a time in the code that computes and decides on its own, which [.cargo/mutants.toml](../.cargo/mutants.toml) lists: text layout, facets, HLS and MPEG-TS, link cards, languages, picture scaling, columns and the text field. It runs the unit tests against each rewrite. A rewrite no test notices, such as `<` turned into `<=` in a width check, is a missed mutant: the line runs under the tests but nothing checks its answer. The rest of the crate mostly connects that code to the terminal, the network and the worker, which the state-transition and end-to-end suites cover.

The `Mutation` workflow runs the whole set every Saturday in eight shards and, on a pull request that touches `src/`, only the mutants on the lines it changes (`cargo mutants --in-diff`). Both fail when the score, caught / (caught + missed) with timeouts counted as caught and mutants that do not compile left out, falls below the recorded score in `MUTATION_MIN_SCORE`. The missed mutants are listed in the job summary. Some are equivalent to the original and no test can catch them, so the recorded score is what the suite reaches today rather than 100%, and it is raised when a test closes a gap.

| Date | Mutants | Score |
|---|---|---|
| 2026-11-19 | 936 generated: 774 caught, 20 timed out, 110 missed, 32 unviable | 87.83% |

A run only writes its score to the job summary. Recording it here and raising `MUTATION_MIN_SCORE` in `.github/workflows/mutation.yml` to match is a change of its own, made after the first full run and again whenever a test raises the score.

To run it locally (a full run takes hours; keep its temporary copies off a small tmpfs):

```sh
cargo install cargo-mutants --locked
TMPDIR=$HOME/.cache/mutants RUST_TEST_THREADS=4 MALLOC_ARENA_MAX=2 cargo mutants -j 4
python3 scripts/mutation_score.py mutants.out
```

## Fuzzing

`fuzz/` holds cargo-fuzz targets for the parsers that read untrusted input: the MPEG-TS demuxer, HLS playlists, the facets of a post and a page's link card. Each checks properties, not only that nothing panics. The `Fuzz` workflow explores each target every night; [CONTRIBUTING.md](../CONTRIBUTING.md#fuzzing) has the local commands.

## Differential checks

The facet rules follow Bluesky's app. The ignored test `api::facets::differential_dump::dump` writes bsky's facets for a file of posts in the shape a dump from the official `@atproto/api` package has, so the two can be compared line by line by hand; the cases where they differed became the `rstest` tables in `src/api/facets.rs`.

## Benchmarks

`bench/himorime.yaml` measures start-up, the commands and the client from start to its first post with [himorime](https://github.com/nao1215/himorime), and the `Benchmark` workflow compares a pull request with its base in the same job. [bench/README.md](../bench/README.md) has the commands.

## Coverage

`scripts/coverage.sh` measures the line coverage of the unit tests and the end-to-end suite together with cargo-llvm-cov, and the `Coverage` workflow publishes it as the README badge.
