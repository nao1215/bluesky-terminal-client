# bsky — Bluesky client for the terminal
#
# Run `just` with no arguments to see available recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Show available tasks
default:
    @just --list

# Run unit tests
test:
    cargo test --all-targets

# Run clippy with warnings as errors
lint:
    cargo clippy --all-targets -- -D warnings

# Check formatting (does not modify files)
fmt-check:
    cargo fmt --all -- --check

# Format all source files
fmt:
    cargo fmt --all

# Build documentation with warnings as errors
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

# Run security audit (requires cargo-audit)
audit:
    cargo audit

# Check dependency licenses and advisories (requires cargo-deny)
deny:
    cargo deny --locked --all-features check

# Measure performance with himorime (needs himorime and python3)
bench *ARGS:
    himorime run {{ARGS}} bench

# Run the end-to-end suite with atago (builds bsky, needs atago on PATH)
e2e *ARGS:
    ./e2e/run.sh {{ARGS}}

# Fuzz one target, e.g. `just fuzz hls_demux -max_total_time=60` (needs nightly and cargo-fuzz).
# New inputs go to fuzz/work/, so the committed seeds in fuzz/corpus/ stay as they are.
fuzz TARGET *ARGS:
    mkdir -p fuzz/work/{{TARGET}}
    cargo +nightly fuzz run {{TARGET}} fuzz/work/{{TARGET}} fuzz/corpus/{{TARGET}} -- {{ARGS}}

# Run every CI check locally
ci: test lint fmt-check doc e2e

# Build a debug binary
build:
    cargo build

# Build a release binary
build-release:
    cargo build --release --locked

# Line coverage of the unit tests and the E2E suite together, as lcov.info (needs cargo-llvm-cov and atago)
coverage:
    ./scripts/coverage.sh

# Test the release scripts (packing, checks, notes, Homebrew formula) without a tag
release-test:
    ./scripts/release/test.sh

# Remove build artifacts
clean:
    cargo clean
