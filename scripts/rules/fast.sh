#!/usr/bin/env bash
#
# fast.sh runs the rules that need no build, in about a second: how the
# READMEs, CHANGELOGs and doc/ are written (docs.py), no lint switched off
# in the crates' source (source.py), and, given a commit message, that it
# carries no co-author or session trailer (trailers.py).
# A hook can call it before every commit; CI runs the same scripts.
#
# Usage: scripts/rules/fast.sh [COMMIT_MESSAGE_FILE]
# Exit status 0 when every rule holds; otherwise 1, with each broken rule
# on its own line on stderr.
set -uo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
status=0

python3 "$SCRIPT_DIR/docs.py" || status=1
python3 "$SCRIPT_DIR/source.py" || status=1
if [ $# -ge 1 ]; then
	python3 "$SCRIPT_DIR/trailers.py" "$1" || status=1
fi
exit "$status"
