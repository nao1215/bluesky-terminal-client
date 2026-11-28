#!/usr/bin/env bash
# Shared provisioning; OAuth, seed data and assertions use the existing helpers.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
system="${1:?expected akkoma or pleroma}"
case "$system" in
    akkoma) port=14000 ;;
    pleroma) port=14001 ;;
    *) echo 'expected akkoma or pleroma' >&2; exit 1 ;;
esac
umask 077
work="$(mktemp -d "${TMPDIR:-/tmp}/stc-$system.XXXXXXXX")"
export DOCKER_CONFIG="$work/docker"
mkdir "$DOCKER_CONFIG"
name="$(basename "$work" | tr '[:upper:].' '[:lower:]-')"
image="stc-compat-akkoma:3.20.0"
cleanup() {
    result=$?
    if [[ "$result" != 0 ]]; then
        docker logs --tail 60 "$name" >&2 2>/dev/null || true
    fi
    docker rm -fv "$name-proxy" "$name" "$name-db" >/dev/null 2>&1 || true
    docker network rm "$name" "$name-edge" >/dev/null 2>&1 || true
    python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$work"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export MSTDN_HOST="http://127.0.0.1:$port"
if [[ "$system" == akkoma ]]; then
    # Official OTP release in Akkoma's own CI runtime, without downloading
    # OS packages or build dependencies. This run targets linux/amd64.
    base='akkoma/ci-base:1.15-otp25@sha256:f2bef33e4331a3507d0a0576711101dba1f90de9cb596d670be82ab61ddb7a55'
    docker pull --platform linux/amd64 "$base" >/dev/null
    curl --fail --silent --show-error --max-time 180 \
        https://akkoma-updates.s3.fr-par.scw.cloud/v3.20.0/akkoma-amd64-musl.zip -o "$work/otp.zip"
    python3 - "$work" <<'PYZIP'
import hashlib, pathlib, sys, zipfile
work = pathlib.Path(sys.argv[1])
assert hashlib.sha256((work / 'otp.zip').read_bytes()).hexdigest() == '9bc4e5b73b0ab5f7cdf656b7ff3611416ccde2ce0a0172eb8ff3a87f21030388'
with zipfile.ZipFile(work / 'otp.zip') as archive:
    archive.extractall(work)
    for entry in archive.infolist():
        (work / entry.filename).chmod((entry.external_attr >> 16) & 0o777)
PYZIP
    cat > "$work/Dockerfile" <<EOF
FROM $base
COPY release /opt/pleroma
WORKDIR /opt/pleroma
EOF
    docker build --network none --platform linux/amd64 -t "$image" "$work" > "$work/build.log" 2>&1 || {
        cat "$work/build.log" >&2
        exit 1
    }
else
    image=git.pleroma.social/pleroma/pleroma:v2.11.0@sha256:d2016be89c0466dde7785ab2e4cc82e671216ef7f439b5d7c64ebc85e7fa9a5e
    docker pull "$image" >/dev/null
fi
python3 "$ROOT/scripts/compat/server.py" config-pleroma "$work" "$port"
docker network create --internal "$name" >/dev/null
docker run -d --name "$name-db" --network "$name" --network-alias db \
    --mount type=volume,destination=/var/lib/postgresql/data \
    -e POSTGRES_USER=pleroma -e POSTGRES_PASSWORD=compat -e POSTGRES_DB=pleroma \
    postgres:14-alpine@sha256:e7d9e6aac7fe3232f7661da103aebc0ce38b04105b46af5e9edd5ef6bc343f20 >/dev/null
for attempt in {1..60}; do
    # The image's initialization server accepts Unix sockets before it
    # creates the database. Wait for the final TCP listener instead.
    if docker exec "$name-db" pg_isready -h 127.0.0.1 -U pleroma -d pleroma >/dev/null 2>&1; then break; fi
    if [[ "$attempt" == 60 ]]; then
        echo 'compat: database did not become ready' >&2
        exit 1
    fi
    sleep 1
done
docker run -d --name "$name" --network "$name" --user 0:0 \
    -e DB_USER=pleroma -e DB_PASS=compat \
    -e ERL_FLAGS='+S 4:4' -e PLEROMA_CONFIG_PATH=/etc/pleroma/config.exs \
    --mount type=volume,destination=/var/lib/pleroma \
    -v "$work/config.exs:/etc/pleroma/config.exs:ro" --entrypoint /bin/sh "$image" \
    -c 'mkdir -p /var/lib/pleroma/uploads /var/lib/pleroma/static && /opt/pleroma/bin/pleroma_ctl migrate && exec /opt/pleroma/bin/pleroma start' >/dev/null
# An internal Docker network blocks server egress and does not publish ports.
# Only this fixed-upstream proxy joins the edge network and binds loopback.
docker network create "$name-edge" >/dev/null
cat > "$work/nginx.conf" <<EOF
events {}
http {
  access_log off;
  server {
    listen 4000;
    client_max_body_size 128m;
    location / {
      proxy_pass http://$name:4000;
      proxy_set_header Host \$http_host;
      proxy_set_header X-Forwarded-For 127.0.0.1;
      proxy_read_timeout 120s;
    }
  }
}
EOF
docker create --name "$name-proxy" --network "$name-edge" \
    -p "127.0.0.1:$port:4000" -v "$work/nginx.conf:/etc/nginx/nginx.conf:ro" \
    nginx:1.28.0-alpine@sha256:30f1c0d78e0ad60901648be663a710bdadf19e4c10ac6782c235200619158284 >/dev/null
docker network connect "$name" "$name-proxy"
docker start "$name-proxy" >/dev/null
python3 "$ROOT/scripts/compat/server.py" wait "$MSTDN_HOST" "$name"
python3 "$ROOT/scripts/compat/server.py" users-pleroma "$name" "$work"
python3 "$ROOT/scripts/compat/server.py" authorize "$MSTDN_HOST" "$system" "$work"
export MSTDN_COMPAT_CONTAINER="$name"
export MSTDN_COMPAT_PROXY="$name-proxy"
python3 "$ROOT/scripts/compat/seed.py" "$work" "$system"
