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
exec "$CRATE_DIR/../../scripts/e2e/run.sh" nostr-terminal-client nstr "$CRATE_DIR" "$@"
