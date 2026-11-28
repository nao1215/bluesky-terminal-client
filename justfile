# The workspace of the terminal clients in crates/. Each client has a justfile
# of its own in its crate directory; these recipes act on the whole workspace.
#
# Run `just` with no arguments to see available recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Show available tasks
default:
    @just --list

# Run the unit tests of every crate
test:
    cargo test --workspace --all-targets

# Run clippy with warnings as errors
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Check formatting (does not modify files)
fmt-check:
    cargo fmt --all -- --check

# Format all source files
fmt:
    cargo fmt --all

# Build documentation with warnings as errors
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Run security audit (requires cargo-audit)
audit:
    cargo audit

# Check dependency licenses and advisories (requires cargo-deny)
deny:
    cargo deny --locked --all-features check

# Run bsky's end-to-end suite with atago (builds bsky, needs atago on PATH)
e2e-bsky *ARGS:
    ./crates/bluesky-terminal-client/e2e/run.sh {{ARGS}}

# Run msky's end-to-end suite with atago (builds msky, needs atago on PATH)
e2e-msky *ARGS:
    ./crates/misskey-terminal-client/e2e/run.sh {{ARGS}}

# Run mstdn's end-to-end suite with atago (builds mstdn, needs atago on PATH)
e2e-mstdn *ARGS:
    ./crates/mastodon-terminal-client/e2e/run.sh {{ARGS}}

# Run every end-to-end suite
e2e: e2e-bsky e2e-msky e2e-mstdn

# Run every CI check locally
ci: test lint fmt-check doc e2e

# Build a release binary of every client
build-release:
    cargo build --release --locked --workspace

# Line coverage of the unit tests and both E2E suites together, as lcov.info (needs cargo-llvm-cov and atago)
coverage:
    ./scripts/coverage.sh

# Test the release scripts (packing, checks, notes, Homebrew formula) without a tag
release-test:
    ./scripts/release/test.sh

# Remove build artifacts
clean:
    cargo clean
