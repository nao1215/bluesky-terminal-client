#!/usr/bin/env bash
#
# public_api.sh lists what terminal-kit, the one library of the workspace,
# lets another crate reach (cargo-public-api, read from rustdoc's JSON),
# and fails when it is not crates/terminal-kit/public-api.txt. An item
# added or taken away then shows in the pull request's diff, to be argued
# for there; `--update` writes the list again.
#
# The clients are binaries: `unreachable_pub` keeps their items private to
# the crate, so they have no list.
#
# rustdoc's JSON is a nightly feature, and its format changes between
# nightlies, so the toolchain is pinned with the cargo-public-api that reads
# it (CI installs both; see .github/workflows/rules.yml).
#
# Usage: scripts/rules/public_api.sh [--update]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
TOOLCHAIN="${PUBLIC_API_TOOLCHAIN:-nightly-2026-09-28}"
LIST="$ROOT/crates/terminal-kit/public-api.txt"

if ! command -v cargo-public-api >/dev/null 2>&1; then
	echo "public-api: cargo-public-api is not installed (cargo install cargo-public-api --locked --version 0.52.0)" >&2
	exit 127
fi

now="$(cargo "+$TOOLCHAIN" public-api --manifest-path "$ROOT/crates/terminal-kit/Cargo.toml" -ss 2>/dev/null)"
if [ "${1:-}" = "--update" ]; then
	printf '%s\n' "$now" >"$LIST"
	exit 0
fi
if ! diff -u "$LIST" <(printf '%s\n' "$now") >&2; then
	echo "error: public-api: what terminal-kit lets other crates reach changed (above); if the change is wanted, run scripts/rules/public_api.sh --update and say in the pull request why a client needs it" >&2
	exit 1
fi
