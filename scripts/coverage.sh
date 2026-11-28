#!/usr/bin/env bash
#
# coverage.sh measures line coverage of the unit tests and the atago
# end-to-end suites (bsky's, msky's and mstdn's) together and writes it as LCOV
# (lcov.info) and a summary.
#
# Both run instrumented builds of the same code: `cargo test` for the unit
# tests, and a debug bsky, msky and mstdn on PATH for their E2E suites, so the
# drawing, the terminal setup and the worker that only the real binary
# reaches are counted too. The debug profile keeps the E2E binaries and the
# test binaries in one report.
#
# Needs cargo-llvm-cov (`cargo install cargo-llvm-cov`) and atago on PATH.
#
# Usage: scripts/coverage.sh [lcov output path]   (default lcov.info)
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
OUT="${1:-lcov.info}"
cd "$REPO_ROOT"

if ! command -v atago >/dev/null 2>&1; then
	echo "coverage: atago is not installed; see CONTRIBUTING.md" >&2
	exit 127
fi

# Every cargo command below builds with -C instrument-coverage and writes
# its profiles under target/coverage. A directory of its own: the wrapper
# that instruments the build is invisible to cargo's change detection, so a
# plain `cargo test` in target/ would otherwise reuse instrumented builds
# made with cfg(coverage).
export CARGO_TARGET_DIR="$REPO_ROOT/target/coverage"
eval "$(cargo llvm-cov show-env --sh)"
cargo llvm-cov clean --workspace

# Dependencies are built optimized: only the workspace's own code (bsky, msky,
# mstdn and terminal-kit) is instrumented, and
# an unoptimized video decoder is too slow for the E2E scenarios that play a
# video in real time (they failed on CI runners).
deps_optimized=(--config 'profile.dev.package."*".opt-level=3')

echo "coverage: unit tests"
cargo test "${deps_optimized[@]}" --locked --workspace --all-targets

echo "coverage: end-to-end suites"
cargo build "${deps_optimized[@]}" --locked -p bluesky-terminal-client -p misskey-terminal-client -p mastodon-terminal-client
# The same locale as each suite's e2e/run.sh: the specs read the clients in
# English and set the locale themselves where they read another one.
(
	cd crates/bluesky-terminal-client
	PATH="$CARGO_TARGET_DIR/debug:$PATH" LC_ALL=C.UTF-8 LANG=C.UTF-8 atago run e2e/atago
)
(
	cd crates/misskey-terminal-client
	PATH="$CARGO_TARGET_DIR/debug:$PATH" LC_ALL=C.UTF-8 LANG=C.UTF-8 LC_MESSAGES='' atago run e2e/atago
)
(
	cd crates/mastodon-terminal-client
	PATH="$CARGO_TARGET_DIR/debug:$PATH" LC_ALL=C.UTF-8 LANG=C.UTF-8 LC_MESSAGES='' atago run e2e/atago
)

cargo llvm-cov report --lcov --output-path "$OUT"
cargo llvm-cov report --summary-only
echo "coverage: wrote $OUT"
