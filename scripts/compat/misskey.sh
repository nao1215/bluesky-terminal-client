#!/usr/bin/env bash
# Misskey 2025.12.0, using its official Compose example and release image.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
exec python3 "$ROOT/crates/misskey-terminal-client/e2e/compat/server.py" misskey "$@"
