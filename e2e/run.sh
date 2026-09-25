#!/usr/bin/env bash
#
# run.sh builds bsky from this checkout (cargo, release profile) and runs the
# atago end-to-end suite (e2e/atago/*.atago.yaml) against the real binary.
#
# The test definitions are atago YAML; this script only bootstraps the
# environment. The suite drives bsky in a pseudo-terminal that answers like a
# kitty-graphics terminal (`pty.graphics: kitty`) and stubs the Bluesky API
# with atago mock servers, so it needs no network and no account.
#
# Environment contract used by the specs:
#   PATH   bsky resolves here (target/release from this checkout)
#
# Usage: e2e/run.sh [atago args...]        (e.g. e2e/run.sh --filter search)
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

if ! command -v atago >/dev/null 2>&1; then
	echo "e2e: atago is not installed. Install it from https://github.com/nao1215/atago" >&2
	echo "e2e: e.g. 'go install github.com/nao1215/atago@latest' (CI uses the release pinned in .github/workflows/e2e.yml)" >&2
	exit 127
fi

cd "$REPO_ROOT"

echo "e2e: building bsky (cargo build --release --locked)..."
cargo build --release --locked

export PATH="$REPO_ROOT/target/release:$PATH"

echo "e2e: $(bsky --version)"
echo "e2e: $(atago --version)"
# Extra args (e.g. --filter X) go before the path so the flag parser sees them.
atago run "$@" e2e/atago
