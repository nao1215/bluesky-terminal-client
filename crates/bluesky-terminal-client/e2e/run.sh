#!/usr/bin/env bash
#
# run.sh builds bsky from this checkout (cargo, the workspace's e2e profile:
# the release optimizations without LTO, so it builds in a fraction of the
# time) and runs the atago end-to-end suite (e2e/atago/*.atago.yaml) against
# the real binary.
# The concurrent token refresh scenario also needs python3 for its mock server's request barrier.
#
# The test definitions are atago YAML; this script only bootstraps the
# environment. The suite drives bsky in a pseudo-terminal that answers like a
# kitty-graphics terminal (`pty.graphics: kitty`) and stubs the Bluesky API
# with atago mock servers, so it needs no network and no account.
#
# Environment contract used by the specs:
#   PATH          bsky resolves here (target/e2e of the workspace)
#   LANG, LC_ALL  C.UTF-8, so bsky speaks English whatever locale the caller
#                 runs in; a spec that checks another language sets LC_ALL in
#                 its own env
#
# Usage: crates/bluesky-terminal-client/e2e/run.sh [atago args...]
#        (e.g. crates/bluesky-terminal-client/e2e/run.sh --filter search)
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
exec "$CRATE_DIR/../../scripts/e2e/run.sh" bluesky-terminal-client bsky "$CRATE_DIR" "$@"
