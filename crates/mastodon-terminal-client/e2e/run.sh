#!/usr/bin/env bash
#
# run.sh builds mstdn from this checkout (cargo, the workspace's e2e profile:
# the release optimizations without LTO) and runs the atago end-to-end suite
# (e2e/atago/*.atago.yaml) against the real binary.
#
# The test definitions are atago YAML; this script only bootstraps the
# environment. The suite stubs the Mastodon API with atago mock servers and
# points mstdn at them with --host or MSTDN_HOST, so it needs no network and
# no account: every login, upload and post goes to a mock.
#
# Environment contract used by the specs:
#   PATH          mstdn resolves here (target/e2e of the workspace)
#   LANG, LC_ALL  C.UTF-8, so mstdn speaks English whatever locale the
#                 caller runs in; a spec that checks another language sets
#                 LC_ALL in its own env
#
# Usage: crates/mastodon-terminal-client/e2e/run.sh [atago args...]
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

echo "e2e: building mstdn (cargo build --profile e2e --locked -p mastodon-terminal-client)..."
cargo build --profile e2e --locked -p mastodon-terminal-client

export PATH="$WORKSPACE_ROOT/target/e2e:$PATH"
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
unset LC_MESSAGES

echo "e2e: $(mstdn --version)"
echo "e2e: $(atago --version)"
# Extra args (e.g. --filter X) go before the path so the flag parser sees them.
atago run "$@" e2e/atago
