#!/bin/sh
# Record the README's demo in a real kitty window, as bsky's
# doc/record-demo.sh records its own:
#
#   doc/record-demo.sh demo       doc/img/demo.gif: msky as the account you
#                                 logged in with `msky login`, on your own
#                                 PC: the home timeline with its pictures, a
#                                 picture full screen, a thread and the
#                                 local timeline (not the notifications:
#                                 showing them marks them read). msky reads
#                                 its own config folder; this script never
#                                 reads, copies or prints the token, and
#                                 presses read-only keys only (j space v 1 2
#                                 esc): nothing is posted, reacted to,
#                                 followed or marked read. It refuses to run
#                                 on CI (CI or GITHUB_ACTIONS set). Look at
#                                 every frame before committing the GIF.
#   doc/record-demo.sh demo-fake  doc/img/demo-fake.gif: the same against
#                                 doc/demo-server.py, with made-up accounts
#                                 and notes, where nothing reaches a real
#                                 server; for anything that is not your PC.
#
# Needs kitty, xdotool, xwininfo and ffmpeg, and an X11 display (Xwayland is fine):
# kitty runs as an X11 window so ffmpeg can capture it, and the keys are sent
# with kitty's remote control.
set -eu
# Your own account is recorded on your own PC only.
if [ "${1:-demo}" = demo ] && { [ -n "${CI:-}" ] || [ -n "${GITHUB_ACTIONS:-}" ]; }; then
  echo "demo records your own account and runs on your own PC only; on CI use demo-fake" >&2
  exit 2
fi
cd "$(dirname "$0")/.."
OUT=$(pwd)/doc/img
BIN=$(pwd)/target/release
WORK=$(mktemp -d)
SOCK=unix:$WORK/kitty.sock
mkdir -p "$OUT"
cargo build --release --locked

KPID=""
FPID=""
SPID=""
W=""
# However the script ends, no kitty, ffmpeg or server of it is left running.
cleanup() {
  [ -n "$FPID" ] && kill "$FPID" || true
  [ -n "$KPID" ] && kill "$KPID" || true
  [ -n "$SPID" ] && kill "$SPID" || true
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# The stand-in server, and a login for it in a config directory of its own.
serve() {
  python3 doc/demo-server.py "$WORK/port" &
  SPID=$!
  for _ in $(seq 1 50); do [ -s "$WORK/port" ] && break; sleep 0.1; done
  PORT=$(cat "$WORK/port")
  CFG="$WORK/cfg"
  mkdir -p "$CFG/accounts"
  printf '{"service": "http://127.0.0.1:%s", "userId": "u-river", "username": "river", "token": "demo"}\n' "$PORT" > "$CFG/accounts/127.0.0.1_$PORT.json"
  printf '{"current": "127.0.0.1:%s"}\n' "$PORT" > "$CFG/accounts.json"
  DEMO_ENV="MSKY_CONFIG_DIR=$CFG MSKY_CACHE_DIR=off MSKY_SERVERS_URL=off MSKY_DOWNLOAD_DIR=$WORK/dl TZ=Asia/Tokyo LC_ALL=C.UTF-8 LANG=C.UTF-8 LANGUAGE="
}

# Start msky in a kitty window of WIDTH x HEIGHT pixels.
start() {
  # Software rendering: a GPU-drawn window read from outside stops showing
  # changes after a while.
  LIBGL_ALWAYS_SOFTWARE=1 kitty -o linux_display_server=x11 -o remember_window_size=no \
    -o initial_window_width="$1" -o initial_window_height="$2" \
    -o font_family="DejaVu Sans Mono" -o font_size=11 \
    -o confirm_os_window_close=0 -o allow_remote_control=yes \
    -o cursor_blink_interval=0 -o window_padding_width=0 \
    --listen-on "$SOCK" --class mskydemo --title msky \
    env PATH="$BIN:$PATH" COLORTERM=truecolor ${DEMO_ENV:-} msky &
  KPID=$!
  W=""
  # kitty makes more than one X window; the one shown is the one to
  # capture.
  for _ in $(seq 1 50); do
    for id in $(xdotool search --pid "$KPID" --class mskydemo || true); do
      if xwininfo -id "$id" | grep -q "IsViewable"; then W=$id; break; fi
    done
    [ -n "$W" ] && break
    sleep 0.2
  done
  [ -n "$W" ] || { echo "no kitty window" >&2; exit 1; }
  for _ in $(seq 1 50); do
    kitty @ --to "$SOCK" ls > /dev/null && break
    sleep 0.2
  done
}

quit() {
  send "q"
  sleep 1
  kill "$KPID" || true
  wait "$KPID" || true
  KPID=""
  rm -f "$WORK/kitty.sock"
}

send() { kitty @ --to "$SOCK" send-text "$1"; }

# Wait until the screen shows TEXT; give up after a while.
ready() {
  for _ in $(seq 1 60); do
    screen=$(kitty @ --to "$SOCK" get-text || true)
    if printf '%s' "$screen" | grep -q "$1"; then
      # Time for the pictures on screen to arrive.
      sleep 2
      return 0
    fi
    sleep 0.5
  done
  echo "the screen never showed $1; it showed:" >&2
  printf '%s\n' "$screen" >&2
  exit 1
}
keys() { # KEY COUNT DELAY
  i=0
  while [ "$i" -lt "$2" ]; do send "$1"; sleep "$3"; i=$((i + 1)); done
}

record() {
  ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -window_id "$W" \
    -framerate 15 -i "${DISPLAY:-:0}" -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p "$WORK/rec.mp4" &
  FPID=$!
  sleep 1
}

stop() { # OUTPUT.gif [WIDTH] [FPS]
  kill -INT "$FPID"
  wait "$FPID" || true
  FPID=""
  ffmpeg -nostdin -loglevel error -y -i "$WORK/rec.mp4" -vf \
    "fps=${3:-10},scale=${2:-960}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
    "$1"
  ls -la "$1"
}

demo_fake() {
  serve
  start 1120 700
  ready "♡"
  record
  sleep 1.5
  # The dolphin video, played in the client, and back.
  send " "
  sleep 3.5
  send "\033"
  sleep 1.5
  # The aquarium's four photos, full screen, and back.
  send "j"
  sleep 1
  send " "
  ready "d download"
  sleep 2
  send "\033"
  sleep 1.5
  # Down to the content warning, opened.
  send "j"
  sleep 1
  send "w"
  sleep 2
  # The renote of the food samples, and the thread of the grapes.
  keys j 2 1
  send "v"
  ready "Thread"
  keys j 2 0.8
  sleep 1
  send "\033"
  sleep 1.2
  # Down to Mika's note and her profile, and back.
  keys j 2 0.8
  send "\r"
  ready "水族館と海"
  sleep 1.5
  keys j 2 0.8
  sleep 1
  send "\033"
  sleep 1.2
  # The notifications, and home again.
  send "4"
  ready "reacted"
  sleep 2
  send "1"
  sleep 1.5
  stop "$OUT/demo-fake.gif" 860 8
  quit
}

# Your own account, on your own PC only.
demo() {
  # msky's own config folder and picture cache; the server list is not
  # read, and no key that writes is pressed. The notifications tab is not
  # visited: showing it marks the notifications read, which is a write.
  DEMO_ENV="MSKY_SERVERS_URL=off"
  start 1120 700
  ready "♡"
  record
  sleep 2
  keys j 3 1.2
  # The pictures of the note shown, if it has any, and back.
  send " "
  sleep 3
  send "\033"
  sleep 1.5
  # A thread, and back.
  send "v"
  ready "Thread"
  keys j 2 1
  sleep 1
  send "\033"
  sleep 1.5
  # The Local tab, and back to Home.
  send "2"
  sleep 3
  keys j 3 1.2
  sleep 1
  send "1"
  sleep 1.5
  stop "$OUT/demo.gif"
  quit
}

case "${1:-demo}" in
  demo) demo ;;
  demo-fake) demo_fake ;;
  *) echo "usage: $0 [demo|demo-fake]" >&2; exit 2 ;;
esac
