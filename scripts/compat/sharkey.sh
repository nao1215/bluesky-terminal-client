#!/usr/bin/env bash
# Sharkey 2025.4.3, using its official Compose example and release image.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 "$ROOT/crates/misskey-terminal-client/e2e/compat/server.py" sharkey "$@"
