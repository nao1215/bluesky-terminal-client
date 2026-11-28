#!/usr/bin/env bash
# Akkoma v3.20.0 OTP release in the project's official CI image.
set -euo pipefail
exec "$(dirname -- "${BASH_SOURCE[0]}")/pleroma-family.sh" akkoma
