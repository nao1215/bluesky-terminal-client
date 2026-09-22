#!/bin/sh
# Record the README's demos in a real kitty window, with the account logged
# in with bsky:
#
#   doc/record-demo.sh demo     doc/img/demo.gif: the timeline, a pinned feed,
#                               then a search
#   doc/record-demo.sh viewer   doc/img/viewer.gif: a picture and a video of
#                               your profile's posts, full screen
#   doc/record-demo.sh themes   doc/img/theme-*.png: the timeline in a few themes
#
# Needs kitty, xdotool, and ffmpeg, and an X11 display (Xwayland is fine):
# kitty runs as an X11 window so ffmpeg can capture it, and the keys are sent
# with kitty's remote control. `themes` sets the theme in settings.json and
# puts the file back as it was when it is done.
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
W=""
# However the script ends, no kitty or ffmpeg of it is left running.
cleanup() {
  [ -n "$FPID" ] && kill "$FPID" || true
  [ -n "$KPID" ] && kill "$KPID" || true
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# Start bsky in a kitty window of WIDTH x HEIGHT pixels.
start() {
  # Software rendering: a GPU-drawn window read from outside stops showing
  # changes after a while, and a playing video would look frozen.
  LIBGL_ALWAYS_SOFTWARE=1 kitty -o linux_display_server=x11 -o remember_window_size=no \
    -o initial_window_width="$1" -o initial_window_height="$2" \
    -o font_family="DejaVu Sans Mono" -o font_size=11 \
    -o confirm_os_window_close=0 -o allow_remote_control=yes \
    -o cursor_blink_interval=0 -o window_padding_width=0 \
    --listen-on "$SOCK" --class bsdemo --title bsky \
    env PATH="$BIN:$PATH" COLORTERM=truecolor bsky &
  KPID=$!
  W=""
  for _ in $(seq 1 50); do
    # This kitty's window, not one left by an earlier run.
    W=$(xdotool search --onlyvisible --pid "$KPID" --class bsdemo | head -1 || true)
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

# Wait until the screen shows TEXT and no error, pressing R to load again
# when the server failed; give up after a while.
ready() {
  for i in $(seq 1 60); do
    screen=$(kitty @ --to "$SOCK" get-text || true)
    if printf '%s' "$screen" | grep -q "failed"; then
      send "R"
      sleep 3
    elif printf '%s' "$screen" | grep -q "$1"; then
      # Time for the pictures on screen to arrive.
      sleep 3
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
typed() { # TEXT, a key at a time
  printf '%s\n' "$1" | fold -w1 | while read -r c; do [ -n "$c" ] && send "$c"; sleep 0.12; done
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

shot() { # OUTPUT.png
  ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -window_id "$W" -i :0 -frames:v 1 "$1"
  ls -la "$1"
}

demo() {
  use_theme bluesky
  start 1120 700
  ready "♡"
  record
  keys j 4 1.2
  sleep 1
  # The next feed (the first one pinned, or Discover), a look down it, and
  # back to Following.
  send "]"
  sleep 4
  keys j 3 1.2
  sleep 1
  send "["
  sleep 1.5
  send "/"
  sleep 0.6
  typed github
  sleep 0.4
  send "\r"
  ready "♡"
  keys j 10 0.7
  sleep 1.5
  stop "$OUT/demo.gif"
  quit
}

viewer() {
  use_theme bluesky
  start 1120 700
  ready "♡"
  send "4"
  ready "followers"
  sleep 2
  record
  sleep 1.5
  # The newest post's picture, full size: held long enough to look at.
  send " "
  ready "d download"
  sleep 4
  send "\x1b"
  sleep 1.5
  # The next post's video, played, then back.
  send "j"
  sleep 1.5
  send " "
  ready "playing"
  sleep 5
  send "\x1b"
  sleep 1.5
  # A playing video changes every frame: smaller and slower, so the GIF
  # stays a size GitHub shows.
  stop "$OUT/viewer.gif" 720 8
  quit
}

themes() {
  for theme in bluesky bluesky-light dracula nord gruvbox catppuccin-latte; do
    use_theme "$theme"
    start 960 600
    ready "♡"
    # Every picture on the first screen, the video's thumbnail included.
    sleep 5
    shot "$OUT/theme-$theme.png"
    quit
  done
}

bs_config_dir() {
  if [ -n "${BSKY_CONFIG_DIR:-}" ]; then echo "$BSKY_CONFIG_DIR"; else echo "${XDG_CONFIG_HOME:-$HOME/.config}/bsky"; fi
}

# Every recording is made in a theme it sets; the settings.json the account
# had is put back however the script ends.
CFG=$(bs_config_dir)
SAVED_SETTINGS="$WORK/settings.json.saved"
if [ -f "$CFG/settings.json" ]; then cp "$CFG/settings.json" "$SAVED_SETTINGS"; fi
restore_settings() {
  if [ -f "$SAVED_SETTINGS" ]; then cp "$SAVED_SETTINGS" "$CFG/settings.json"; else rm -f "$CFG/settings.json"; fi
}
trap 'restore_settings; cleanup' EXIT INT TERM
use_theme() { printf '{"theme": "%s"}\n' "$1" > "$CFG/settings.json"; }

case "${1:-demo}" in
  demo) demo ;;
  viewer) viewer ;;
  themes) themes ;;
  all) demo; viewer; themes ;;
  *) echo "usage: $0 [demo|viewer|themes|all]" >&2; exit 2 ;;
esac
