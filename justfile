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
    cargo deny check

# Run the end-to-end suite with atago (builds bsky, needs atago on PATH)
e2e *ARGS:
    ./e2e/run.sh {{ARGS}}

# Run every CI check locally
ci: test lint fmt-check doc e2e

# Build a debug binary
build:
    cargo build

# Build a release binary
build-release:
    cargo build --release --locked

# Run code coverage (requires cargo-llvm-cov)
coverage:
    cargo llvm-cov --all-targets --summary-only

# Remove build artifacts
clean:
    cargo clean
