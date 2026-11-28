#!/bin/sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
exec python3 "$here/../../../scripts/bench/runner.py" "$1" "$here/mock.py" "$2"
