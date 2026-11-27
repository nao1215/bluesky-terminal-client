#!/bin/sh
# Record the README's demo in a real kitty window, as bsky's
# doc/record-demo.sh records its own:
#
#   doc/record-demo.sh demo     doc/img/demo.gif: the home timeline with its
#                               pictures, a content warning opened, a
#                               picture full screen, a thread, the local
#                               timeline and the notifications
#
# It runs against doc/demo-server.py, with made-up accounts and notes, so no
# real account is shown, and nothing reaches a real server: the login is a
# made-up one for the stand-in, and the server list is not read.
#
# Needs kitty, xdotool, xwininfo and ffmpeg, and an X11 display (Xwayland is fine):
# kitty runs as an X11 window so ffmpeg can capture it, and the keys are sent
# with kitty's remote control.
set -eu
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
  DEMO_ENV="MSKY_CONFIG_DIR=$CFG MSKY_CACHE_DIR=off MSKY_SERVERS_URL=off MSKY_DOWNLOAD_DIR=$WORK/dl TZ=Asia/Tokyo"
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
    env PATH="$BIN:$PATH" COLORTERM=truecolor $DEMO_ENV msky &
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
    -framerate 15 -i :0 -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p "$WORK/rec.mp4" &
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

demo() {
  serve
  start 1120 700
  ready "♡"
  record
  sleep 1.5
  # The owl, full screen, and back.
  send " "
  ready "d download"
  sleep 2.5
  send "\033"
  sleep 1.5
  # Down to the content warning, opened.
  keys j 2 1
  send "w"
  sleep 2
  # The renote, the note it renotes, and the thread of a note with replies.
  keys j 2 1
  send "v"
  ready "Thread"
  keys j 2 0.8
  sleep 1
  send "\033"
  sleep 1.2
  # The local timeline, and the notifications.
  send "L"
  ready "Local"
  sleep 1
  send "4"
  ready "reacted"
  sleep 2
  send "1"
  sleep 1.5
  stop "$OUT/demo.gif"
  quit
}

case "${1:-demo}" in
  demo) demo ;;
  *) echo "usage: $0 [demo]" >&2; exit 2 ;;
esac
