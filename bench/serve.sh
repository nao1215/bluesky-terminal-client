#!/bin/sh
# Start or stop the stand-in server of the benchmarks, and give bsky an
# account on it in WORKDIR/cfg.
#
#   sh bench/serve.sh start WORKDIR
#   sh bench/serve.sh stop WORKDIR
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=$2
case $1 in
start)
  # Its own session, so it outlives this script, which himorime waits for.
  setsid python3 "$here/mock.py" "$work" >/dev/null &
  echo $! >"$work/server.pid"
  i=0
  while [ ! -s "$work/port" ]; do
    i=$((i + 1))
    [ "$i" -gt 100 ] && { echo "the mock server did not start" >&2; exit 1; }
    sleep 0.05
  done
  port=$(cat "$work/port")
  mkdir -p "$work/cfg/accounts"
  printf '{"service": "http://127.0.0.1:%s", "did": "did:plc:bench", "handle": "bench.test", "accessJwt": "a", "refreshJwt": "r"}\n' "$port" >"$work/cfg/accounts/did_plc_bench.json"
  printf '{"current": "did:plc:bench"}\n' >"$work/cfg/accounts.json"
  ;;
stop)
  [ -f "$work/server.pid" ] && kill "$(cat "$work/server.pid")" || true
  ;;
esac
