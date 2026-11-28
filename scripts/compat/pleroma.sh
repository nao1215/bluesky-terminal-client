#!/usr/bin/env bash
# Pleroma v2.11.0, using the project's official release image.
set -euo pipefail
exec "$(dirname -- "${BASH_SOURCE[0]}")/pleroma-family.sh" pleroma
