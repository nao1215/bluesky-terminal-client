#!/bin/sh
# Record fictional data only. Run from any directory on an X11 desktop.
set -eu
umask 077
cd "$(dirname "$0")/.."
for tool in cargo python3 kitty xdotool xwininfo ffmpeg; do
  command -v "$tool" >/dev/null || { echo "Missing command: $tool" >&2; exit 1; }
done
: "${DISPLAY:?An X11 display is required}"
[ "$#" -eq 0 ] || { echo "usage: $0" >&2; exit 2; }
OUT=$(pwd)/doc/demo/demo.gif
cargo build --release --locked -p nostr-terminal-client
TARGET=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
BIN=$TARGET/release/nstr
WORK=$(mktemp -d)
SOCK=unix:$WORK/kitty.sock
KPID=""
FPID=""
SPID=""
cleanup() {
  for pid in "$FPID" "$KPID" "$SPID"; do
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; fi
  done
  rm -rf "$WORK"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -p "$WORK/home" "$WORK/config" "$WORK/cache" "$WORK/data"
python3 doc/demo-server.py "$WORK/port" >"$WORK/server.log" 2>&1 &
SPID=$!
for _ in $(seq 1 100); do
  [ -s "$WORK/port" ] && break
  kill -0 "$SPID" 2>/dev/null || { cat "$WORK/server.log" >&2; exit 1; }
  sleep 0.1
done
[ -s "$WORK/port" ] || { cat "$WORK/server.log" >&2; echo "Demo server timed out" >&2; exit 1; }
# Drop inherited account, proxy and browser variables. Only X11 authorization
# is passed through. Both kitty and the client get a disposable HOME.
env -i PATH="$PATH" HOME="$WORK/home" XDG_CONFIG_HOME="$WORK/config"   XDG_CACHE_HOME="$WORK/cache" XDG_DATA_HOME="$WORK/data"   DISPLAY="$DISPLAY" XAUTHORITY="${XAUTHORITY:-$HOME/.Xauthority}"   LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC LIBGL_ALWAYS_SOFTWARE=1   NSTR_CONFIG_DIR="$WORK/cfg"   kitty --config NONE -o linux_display_server=x11 -o remember_window_size=no     -o initial_window_width=1120 -o initial_window_height=760     -o font_family="DejaVu Sans Mono" -o font_size=11     -o confirm_os_window_close=0 -o allow_remote_control=yes     -o cursor_blink_interval=0 -o window_padding_width=0     --listen-on "$SOCK" --class nstrdemo --title nstr     --directory "$WORK/home" "$BIN" &
KPID=$!
W=""
for _ in $(seq 1 100); do
  for id in $(xdotool search --pid "$KPID" --class nstrdemo 2>/dev/null || true); do
    if xwininfo -id "$id" | grep -q IsViewable; then W=$id; break; fi
  done
  [ -n "$W" ] && break
  sleep 0.1
done
[ -n "$W" ] || { echo "No kitty window" >&2; exit 1; }
send() { kitty @ --to "$SOCK" send-text "$1"; }
ready() {
  for _ in $(seq 1 60); do
    screen=$(kitty @ --to "$SOCK" get-text 2>/dev/null || true)
    if printf '%s' "$screen" | grep -Fq "$1"; then sleep 1; return; fi
    sleep 0.5
  done
  printf 'Screen never showed %s:\n%s\n' "$1" "$screen" >&2
  exit 1
}
ready "An animated scene"
sleep 2
ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -window_id "$W"   -framerate 15 -i "$DISPLAY" -c:v libx264 -preset veryfast -crf 20   -pix_fmt yuv420p "$WORK/recording.mp4" &
FPID=$!
sleep 3
# Picture, then the separate video post. Avoid get-text during image transfers.
send " "
sleep 3
send "\033"
sleep 1
send j
sleep 1
send " "
sleep 6
send "\033"
sleep 1
send j
ready "Relay notebook"
sleep 5
send j
ready "Key recovery exercise"
send w
ready "The recovery drill"
sleep 2
send g
sleep 1
send "\r"
ready "yuna@atelier.example"
sleep 3
send "\033"
sleep 1
kill -INT "$FPID"
wait "$FPID" || true
FPID=""
ffmpeg -nostdin -loglevel error -y -i "$WORK/recording.mp4" -vf   "fps=8,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle"   "$WORK/demo.gif"
mv "$WORK/demo.gif" "$OUT"
send q
printf 'Recorded %s\n' "$OUT"
