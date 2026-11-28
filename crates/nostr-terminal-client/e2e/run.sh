#!/usr/bin/env bash
#
# run.sh builds nstr from this checkout (cargo, the workspace's e2e profile:
# the release optimizations without LTO) and runs the atago end-to-end suite
# (e2e/atago/*.atago.yaml) against the real binary.
#
# The test definitions are atago YAML; this script only bootstraps the
# environment. The relays are e2e/atago/testdata/relay.py, a stand-in on
# 127.0.0.1 that writes the config naming it, so the suite needs no network
# and no key: the events are signed with throwaway keys, and every post goes
# to the stand-in. It needs python3 on PATH.
#
# Environment contract used by the specs:
#   PATH          nstr resolves here (target/e2e of the workspace)
#   LANG, LC_ALL  C.UTF-8, as for the other clients' suites
#
# Usage: crates/nostr-terminal-client/e2e/run.sh [atago args...]
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
WORKSPACE_ROOT="$(cd -- "$CRATE_DIR/../.." && pwd)"

if ! command -v atago >/dev/null 2>&1; then
	echo "e2e: atago is not installed. Install it from https://github.com/nao1215/atago" >&2
	echo "e2e: e.g. 'go install github.com/nao1215/atago@latest' (CI uses the release pinned in the workspace's .github/workflows/e2e.yml)" >&2
	exit 127
fi

cd "$CRATE_DIR"

echo "e2e: building nstr (cargo build --profile e2e --locked -p nostr-terminal-client)..."
cargo build --profile e2e --locked -p nostr-terminal-client

export PATH="$WORKSPACE_ROOT/target/e2e:$PATH"
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
unset LC_MESSAGES

echo "e2e: $(nstr --version)"
echo "e2e: $(atago --version)"
# Extra args (e.g. --filter X) go before the path so the flag parser sees them.
atago run "$@" e2e/atago
