#!/usr/bin/env bash
# Build a workspace client and run its atago suite from its crate directory.
# Usage: scripts/e2e/run.sh <package> <binary> <crate-dir> [atago args...]
set -euo pipefail

PACKAGE="$1"
BINARY="$2"
CRATE_DIR="$(cd -- "$3" && pwd)"
shift 3
WORKSPACE_ROOT="$(cd -- "$CRATE_DIR/../.." && pwd)"

if ! command -v atago >/dev/null 2>&1; then
    echo "e2e: atago is not installed. Install it from https://github.com/nao1215/atago" >&2
    echo "e2e: e.g. 'go install github.com/nao1215/atago@latest' (CI uses the release pinned in the workspace's .github/workflows/e2e.yml)" >&2
    exit 127
fi

cd "$CRATE_DIR"
echo "e2e: building $BINARY (cargo build --profile e2e --locked -p $PACKAGE)..."
cargo build --profile e2e --locked -p "$PACKAGE"

export PATH="$WORKSPACE_ROOT/target/e2e:$PATH"
export LANG=C.UTF-8 LC_ALL=C.UTF-8
unset LC_MESSAGES

echo "e2e: $("$BINARY" --version)"
echo "e2e: $(atago --version)"
# Flags precede the path so atago's flag parser sees them.
exec atago run "$@" e2e/atago
