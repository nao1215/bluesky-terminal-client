#!/usr/bin/env bash
#
# run.sh builds msky from this checkout (cargo, the workspace's e2e profile:
# the release optimizations without LTO, so it builds in a fraction of the
# time) and runs the atago end-to-end suite (e2e/atago/*.atago.yaml) against
# the real binary.
#
# The test definitions are atago YAML; this script only bootstraps the
# environment. The suite stubs the Misskey API with atago mock servers and
# points msky at them with --host or MSKY_HOST, so it needs no network and
# no account.
#
# Environment contract used by the specs:
#   PATH   msky resolves here (target/e2e of the workspace)
#   LC_ALL C.UTF-8, so the client is in English unless a spec says otherwise
#
# Usage: crates/misskey-terminal-client/e2e/run.sh [atago args...]
#        (e.g. crates/misskey-terminal-client/e2e/run.sh --filter search)
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

echo "e2e: building msky (cargo build --profile e2e --locked -p misskey-terminal-client)..."
cargo build --profile e2e --locked -p misskey-terminal-client

export PATH="$WORKSPACE_ROOT/target/e2e:$PATH"
# The client speaks the language the locale asks for; the specs read it in
# English, and set the locale themselves where they read another one.
export LC_ALL=C.UTF-8 LANG=C.UTF-8
unset LC_MESSAGES

echo "e2e: $(msky --version)"
echo "e2e: $(atago --version)"
# Extra args (e.g. --filter X) go before the path so the flag parser sees them.
atago run "$@" e2e/atago
