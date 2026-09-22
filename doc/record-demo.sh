#!/bin/sh
# Record doc/demo.gif, the README's demo: bs in a real kitty window, the
# timeline scrolled, then a search for "github" scrolled.
#
# It uses the account logged in with bs, so the recording shows that
# account's timeline. Needs kitty, xdotool, and ffmpeg, and an X11 display
# (Xwayland is fine): kitty runs as an X11 window so ffmpeg can capture it.
# The keys are sent with kitty's remote control.
set -eu
cd "$(dirname "$0")/.."
OUT=$(pwd)/doc
WORK=$(mktemp -d)
BIN=$(pwd)/target/release
SOCK=unix:$WORK/kitty.sock
cargo build --release --locked

kitty -o linux_display_server=x11 -o remember_window_size=no \
  -o initial_window_width=1120 -o initial_window_height=700 \
  -o font_family="DejaVu Sans Mono" -o font_size=11 \
  -o confirm_os_window_close=0 -o allow_remote_control=yes \
  -o cursor_blink_interval=0 -o window_padding_width=0 \
  --listen-on "$SOCK" --class bsdemo --title bs \
  env PATH="$BIN:$PATH" COLORTERM=truecolor bs &
KPID=$!

W=""
for _ in $(seq 1 50); do
  W=$(xdotool search --class bsdemo | head -1 || true)
  [ -n "$W" ] && break
  sleep 0.2
done
[ -n "$W" ] || { echo "no kitty window"; exit 1; }

send() { kitty @ --to "$SOCK" send-text "$1"; }
keys() { # keys KEY COUNT DELAY
  i=0
  while [ "$i" -lt "$2" ]; do send "$1"; sleep "$3"; i=$((i + 1)); done
}

# Let the timeline and its pictures load before the camera starts.
sleep 6
ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -window_id "$W" \
  -framerate 15 -i :0 -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p "$WORK/demo.mp4" &
FPID=$!
sleep 1.5

# The timeline, scrolled post by post.
keys j 10 0.7
sleep 1

# Search posts for "github" and scroll the results.
send "/"
sleep 0.6
for c in g i t h u b; do send "$c"; sleep 0.15; done
sleep 0.4
send "\r"
sleep 4
keys j 10 0.7
sleep 1.5

kill -INT "$FPID"
wait "$FPID" || true
send "q"
sleep 1
kill "$KPID" || true
ffmpeg -nostdin -loglevel error -y -i "$WORK/demo.mp4" -vf "fps=10,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" "$OUT/demo.gif"
rm -rf "$WORK"
ls -la "$OUT/demo.gif"
