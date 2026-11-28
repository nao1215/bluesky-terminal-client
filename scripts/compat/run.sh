#!/usr/bin/env bash
# A destructive suite for throwaway accounts on a loopback server only.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${MSTDN_HOST:?set the disposable loopback origin}"
if [[ ! "$MSTDN_HOST" =~ ^http://127\.0\.0\.1:[0-9]+$ ]]; then
    echo 'compat: only http://127.0.0.1:PORT is permitted' >&2
    exit 1
fi
umask 077
export MSTDN_COMPAT_WORK
MSTDN_COMPAT_WORK="$(mktemp -d "${TMPDIR:-/tmp}/mstdn-compat.XXXXXXXX")"
cleanup() { python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$MSTDN_COMPAT_WORK"; }
trap cleanup EXIT
export MSTDN_CONFIG_DIR="$MSTDN_COMPAT_WORK/config"
export LANG=C.UTF-8 LC_ALL=C.UTF-8
export NO_PROXY=127.0.0.1 no_proxy=127.0.0.1
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy
: "${MSTDN_COMPAT_SEED:?run a server bootstrap script to seed test data}"
: "${MSTDN_TOKEN_SECOND:?set a second throwaway account token for notifications}"
command -v atago >/dev/null
cd "$ROOT"
cargo build --profile e2e --locked -p mastodon-terminal-client
export PATH="$ROOT/target/e2e:$PATH"
selection=()
if [[ -n "${MSTDN_COMPAT_FILTER:-}" ]]; then
    selection=(--filter "$MSTDN_COMPAT_FILTER")
fi
set +e
atago run --ci --parallel 1 --report json "${selection[@]}" "$ROOT/crates/mastodon-terminal-client/e2e/compat" > "$MSTDN_COMPAT_WORK/atago.json"
result=$?
set -e
cat "$MSTDN_COMPAT_WORK/atago.json"
report_dir="${MSTDN_COMPAT_REPORT_DIR:-$ROOT/target/compat/$MSTDN_COMPAT_SYSTEM}"
mkdir -p -- "$report_dir"
cp -- "$MSTDN_COMPAT_WORK/atago.json" "$report_dir/atago.json"
cp -- "$MSTDN_COMPAT_SEED/seed.json" "$report_dir/seed.json"
exit "$result"
