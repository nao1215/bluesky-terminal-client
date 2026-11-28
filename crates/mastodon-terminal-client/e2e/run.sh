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
exec "$CRATE_DIR/../../scripts/e2e/run.sh" mastodon-terminal-client mstdn "$CRATE_DIR" "$@"
