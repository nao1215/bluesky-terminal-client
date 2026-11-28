#!/usr/bin/env bash
# GoToSocial's public OAuth forms, on a disposable loopback-only instance.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
umask 077
work="$(mktemp -d "${TMPDIR:-/tmp}/stc-gts.XXXXXXXX")"
name="$(basename "$work" | tr '[:upper:]' '[:lower:]')"
image='superseriousbusiness/gotosocial:0.19.0@sha256:6de5714314ead3468f5514b8957b400260a743b4035a167cfd3922d86e85f792'
cleanup() {
    docker rm -fv "$name" >/dev/null 2>&1 || true
    docker network rm "$name" >/dev/null 2>&1 || true
    python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$work"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export MSTDN_HOST=http://127.0.0.1:18080
docker pull "$image" >/dev/null
docker network create "$name" >/dev/null
docker run -d --name "$name" --network "$name" --user 0:0 \
    -p 127.0.0.1:18080:8080 --mount type=volume,destination=/gotosocial/storage \
    -e GTS_HOST=127.0.0.1:18080 -e GTS_PROTOCOL=http -e GTS_BIND_ADDRESS=0.0.0.0 \
    -e GTS_PORT=8080 -e GTS_DB_TYPE=sqlite -e GTS_DB_ADDRESS=/gotosocial/storage/sqlite.db \
    -e GTS_STORAGE_LOCAL_BASE_PATH=/gotosocial/storage -e GTS_INSTANCE_FEDERATION_MODE=allowlist \
    -e GTS_ADVANCED_RATE_LIMIT_REQUESTS=0 "$image" >/dev/null
python3 "$ROOT/scripts/compat/server.py" wait "$MSTDN_HOST"
python3 "$ROOT/scripts/compat/server.py" users-gts "$name" "$work"
docker restart "$name" >/dev/null
python3 "$ROOT/scripts/compat/server.py" wait "$MSTDN_HOST"
python3 "$ROOT/scripts/compat/server.py" authorize "$MSTDN_HOST" gotosocial "$work"
export MSTDN_COMPAT_CONTAINER="$name"
python3 "$ROOT/scripts/compat/seed.py" "$work" gotosocial
