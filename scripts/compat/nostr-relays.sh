#!/usr/bin/env bash
# Build official release sources, expose only loopback ports, run nstr's live suite.
# Requires Docker with buildx, git, cargo, python3 and atago. Bootstrap downloads
# source/build dependencies; relay ports are published only on 127.0.0.1.
# NSTR_COMPAT_OFFLINE=1 uses existing images and a local strfry configuration.
set -euo pipefail
umask 077

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
SUITE="$ROOT/crates/nostr-terminal-client/e2e/compat"
case "${NSTR_COMPAT_MODE:-all}" in
    all) implementations=(strfry nostr-rs-relay); modes=(strfry nostr-rs-relay both) ;;
    both) implementations=(strfry nostr-rs-relay); modes=(both) ;;
    strfry|nostr-rs-relay) implementations=("$NSTR_COMPAT_MODE"); modes=("$NSTR_COMPAT_MODE") ;;
    *) echo 'NSTR_COMPAT_MODE must be all, both, strfry or nostr-rs-relay' >&2; exit 2 ;;
esac
STRFRY_VERSION=1.0.1
NOSTR_RS_VERSION=0.10.0
for tool in docker git cargo python3 atago ffmpeg; do
    command -v "$tool" >/dev/null || { echo "missing tool: $tool" >&2; exit 127; }
done
docker buildx version >/dev/null
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nstr-compat.XXXXXXXX")"
export DOCKER_CONFIG="$RUN_DIR/docker"
mkdir "$DOCKER_CONFIG"
RUN_ID="nstr-compat-$(basename "$RUN_DIR" | tr '[:upper:]' '[:lower:]')"
CONTAINERS=()
VOLUMES=()
IMAGES=()
BUILDER=""
NETWORK=""
MEDIA_PID=""
cleanup() {
    local status=$?
    trap - EXIT
    if [[ -n "$MEDIA_PID" ]]; then kill "$MEDIA_PID" 2>/dev/null || true; wait "$MEDIA_PID" 2>/dev/null || true; fi
    if ((status != 0)); then
        for container in "${CONTAINERS[@]}"; do docker logs --tail 100 "$container" >&2 || true; done
    fi
    for container in "${CONTAINERS[@]}"; do docker rm -f "$container" >/dev/null || true; done
    for volume in "${VOLUMES[@]}"; do docker volume rm "$volume" >/dev/null || true; done
    if [[ -n "$NETWORK" ]]; then docker network rm "$NETWORK" >/dev/null || true; fi
    if [[ -n "$BUILDER" ]]; then docker buildx rm "$BUILDER" >/dev/null || true; fi
    for image in "${IMAGES[@]}"; do docker image rm "$image" >/dev/null || true; done
    rm -rf -- "$RUN_DIR"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Accept the upstream's optional v prefix, but never a moving branch or latest tag.
checkout_release() {
    local repo=$1 version=$2 destination=$3 tag refs
    refs="$(git -c http.version=HTTP/1.1 ls-remote --tags --refs "$repo" "refs/tags/$version" "refs/tags/v$version")"
    if [[ "$refs" == *"refs/tags/$version"* ]]; then tag="$version"
    elif [[ "$refs" == *"refs/tags/v$version"* ]]; then tag="v$version"
    else echo "release $version not found in $repo" >&2; return 1; fi
    git -c http.version=HTTP/1.1 -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" --recurse-submodules --shallow-submodules "$repo" "$destination"
    git -C "$destination" rev-parse HEAD
}

echo "compat: strfry $STRFRY_VERSION; nostr-rs-relay $NOSTR_RS_VERSION"
if [[ "${NSTR_COMPAT_OFFLINE:-0}" == 1 ]]; then
    for implementation in "${implementations[@]}"; do
        if [[ "$implementation" == strfry ]]; then
            : "${NSTR_COMPAT_STRFRY_IMAGE:?offline mode requires an existing strfry image}"
            : "${NSTR_COMPAT_STRFRY_CONFIG:?offline mode requires the upstream strfry.conf}"
            docker image inspect "$NSTR_COMPAT_STRFRY_IMAGE" >/dev/null
            mkdir "$RUN_DIR/strfry"
            cp -- "$NSTR_COMPAT_STRFRY_CONFIG" "$RUN_DIR/strfry/strfry.conf"
        else
            : "${NSTR_COMPAT_NOSTR_RS_IMAGE:?offline mode requires an existing nostr-rs-relay image}"
            docker image inspect "$NSTR_COMPAT_NOSTR_RS_IMAGE" >/dev/null
        fi
    done
else
for implementation in "${implementations[@]}"; do
    if [[ "$implementation" == strfry ]]; then
        checkout_release https://github.com/hoytech/strfry.git "$STRFRY_VERSION" "$RUN_DIR/strfry"
cat > "$RUN_DIR/strfry/Dockerfile.compat" <<'DOCKER'
FROM ubuntu:24.04
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates g++ make git perl pkg-config libtool autoconf libssl-dev zlib1g-dev liblmdb-dev libflatbuffers-dev libsecp256k1-dev libzstd-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN make setup-golpe && make -j4
ENTRYPOINT ["/src/strfry", "--config=/etc/strfry.conf", "relay"]
DOCKER
    else
        checkout_release https://github.com/scsibug/nostr-rs-relay.git "$NOSTR_RS_VERSION" "$RUN_DIR/nostr-rs-relay"
cat > "$RUN_DIR/nostr-rs-relay/Dockerfile.compat" <<'DOCKER'
FROM rust:1.93.1-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends cmake protobuf-compiler && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --locked --release -j4
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/nostr-rs-relay /usr/local/bin/nostr-rs-relay
ENTRYPOINT ["nostr-rs-relay", "--config", "/etc/nostr-rs-relay.toml"]
DOCKER
    fi
done
# A private builder keeps build cache disposable without pruning anybody else's cache.
docker buildx create --name "$RUN_ID" --driver docker-container >/dev/null
BUILDER="$RUN_ID"
for implementation in "${implementations[@]}"; do
    image="$RUN_ID/$implementation:local"
    IMAGES+=("$image")
    docker buildx build --builder "$BUILDER" --load --tag "$image" \
        --file "$RUN_DIR/$implementation/Dockerfile.compat" "$RUN_DIR/$implementation"
done
NSTR_COMPAT_STRFRY_IMAGE="$RUN_ID/strfry:local"
NSTR_COMPAT_NOSTR_RS_IMAGE="$RUN_ID/nostr-rs-relay:local"
fi

if [[ -d "$RUN_DIR/strfry" ]]; then
python3 - "$RUN_DIR" <<'PY'
from pathlib import Path
import re
import sys
root = Path(sys.argv[1])
config = (root / "strfry/strfry.conf").read_text()
for name, value in [("db", '"/data/"'), ("bind", '"0.0.0.0"'), ("port", "7777"), ("nofiles", "0")]:
    config, count = re.subn(rf"(?m)^(\s*{name}\s*=\s*)[^\n]+", lambda m: m[1] + value, config)
    if count != 1:
        raise SystemExit(f"expected one strfry config setting: {name}")
(root / "strfry.conf").write_text(config)
PY
fi
cat > "$RUN_DIR/nostr-rs-relay.toml" <<'TOML'
[info]
name = "nstr local compatibility"
[database]
data_directory = "/data"
[network]
address = "0.0.0.0"
port = 8080
TOML

# Internal bridge networks suppress published ports on some Docker versions.
# Isolate this run on its own bridge and bind every published port to loopback.
docker network create "$RUN_ID" >/dev/null
NETWORK="$RUN_ID"
start_relay() {
    local implementation=$1 port=$2 config=$3 destination=$4 volume container image
    if [[ "$implementation" == strfry ]]; then image="$NSTR_COMPAT_STRFRY_IMAGE"
    else image="$NSTR_COMPAT_NOSTR_RS_IMAGE"; fi
    volume="$(docker volume create "$RUN_ID-$implementation")"
    VOLUMES+=("$volume")
    container="$(docker run --pull=never --detach --network "$NETWORK" --publish "127.0.0.1::$port" \
        --mount "type=volume,src=$volume,dst=/data" \
        --mount "type=bind,src=$config,dst=$destination,readonly" \
        "$image")"
    CONTAINERS+=("$container")
}
# Media files are generated here and served only over loopback.
mkdir "$RUN_DIR/media" "$RUN_DIR/tmp"
ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc2=size=160x96:rate=12 -t 1 -c:v libx264 -pix_fmt yuv420p "$RUN_DIR/media/pattern.mp4"
ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc2=size=160x96:rate=12 -t 1 -c:v libvpx-vp9 "$RUN_DIR/media/unsupported.webm"
for index in 0 1 2 3; do
    ffmpeg -hide_banner -loglevel error -i "$RUN_DIR/media/pattern.mp4" -vf "hue=h=$((index * 90))" -frames:v 1 "$RUN_DIR/media/picture-$index.png"
done
export PYTHONDONTWRITEBYTECODE=1
python3 "$SUITE/media.py" "$RUN_DIR/media" "$RUN_DIR/media-ready" > "$RUN_DIR/media.log" 2>&1 &
MEDIA_PID=$!
for ((attempt=0; attempt<300; attempt++)); do
    [[ -s "$RUN_DIR/media-ready" ]] && break
    kill -0 "$MEDIA_PID" || { cat "$RUN_DIR/media.log" >&2; exit 1; }
    sleep 0.1
done
NSTR_COMPAT_MEDIA="$(cat "$RUN_DIR/media-ready")"
export NSTR_COMPAT_MEDIA
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
cargo build --manifest-path "$ROOT/Cargo.toml" --locked -p nostr-terminal-client
export PATH="$CARGO_TARGET_DIR/debug:$PATH"
export LANG=C.UTF-8 LC_ALL=C.UTF-8
unset LC_MESSAGES
cd "$RUN_DIR"
for mode in "${modes[@]}"; do
    urls=()
    if [[ "$mode" != nostr-rs-relay ]]; then
        start_relay strfry 7777 "$RUN_DIR/strfry.conf" /etc/strfry.conf
        urls+=("ws://$(docker port "${CONTAINERS[0]}" 7777/tcp)/")
    fi
    if [[ "$mode" != strfry ]]; then
        start_relay nostr-rs-relay 8080 "$RUN_DIR/nostr-rs-relay.toml" /etc/nostr-rs-relay.toml
        urls+=("ws://$(docker port "${CONTAINERS[${#CONTAINERS[@]}-1]}" 8080/tcp)/")
    fi
    for relay in "${urls[@]}"; do
        python3 "$SUITE/fixture.py" wait "$relay" > "$RUN_DIR/$mode-server.json"
        cat "$RUN_DIR/$mode-server.json"
    done
    NSTR_COMPAT_RELAYS="$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1:]))' "${urls[@]}")"
    export NSTR_COMPAT_RELAYS
    export NSTR_COMPAT_COUNT="${#urls[@]}"
    export NSTR_COMPAT_CONFIG="$RUN_DIR/$mode-config"
    python3 "$SUITE/fixture.py" prepare "$NSTR_COMPAT_CONFIG"
    NSTR_COMPAT_PUBLIC="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["publicKey"])' "$NSTR_COMPAT_CONFIG/config.json")"
    export NSTR_COMPAT_PUBLIC
    export NSTR_COMPAT_PICTURES_CONFIG="$NSTR_COMPAT_CONFIG/pictures"
    export NSTR_COMPAT_VIDEOS_CONFIG="$NSTR_COMPAT_CONFIG/videos"
    echo "compat: testing $mode ($NSTR_COMPAT_RELAYS)"
    status=0
    TMPDIR="$RUN_DIR/tmp" atago run --parallel 1 --report json "$@" "$SUITE" > "$RUN_DIR/$mode.json" || status=$?
    python3 "$SUITE/report.py" "$RUN_DIR/$mode.json" "$mode"
    if [[ -n "${NSTR_COMPAT_REPORT_DIR:-}" ]]; then
        mkdir -p "$NSTR_COMPAT_REPORT_DIR"
        cp "$RUN_DIR/$mode.json" "$NSTR_COMPAT_REPORT_DIR/$mode.json"
        cp "$RUN_DIR/$mode-server.json" "$NSTR_COMPAT_REPORT_DIR/$mode-server.json"
    fi
    if ((status != 0)); then exit "$status"; fi
    # Each mode starts with fresh databases, so global hashtag searches cannot
    # see earlier modes' accounts or writes. Only this run's resources are removed.
    for container in "${CONTAINERS[@]}"; do docker rm -f "$container" >/dev/null; done
    for volume in "${VOLUMES[@]}"; do docker volume rm "$volume" >/dev/null; done
    CONTAINERS=()
    VOLUMES=()
done
