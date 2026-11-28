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
exec "$CRATE_DIR/../../scripts/e2e/run.sh" misskey-terminal-client msky "$CRATE_DIR" "$@"
