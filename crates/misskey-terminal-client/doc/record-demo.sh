#!/bin/sh
# Record the README's demo in a real kitty window, as bsky's
# doc/record-demo.sh records its own:
#
#   doc/record-demo.sh demo-fake  doc/img/demo.gif, the README's: msky
#                                 against doc/demo-server.py, with made-up
#                                 accounts and notes in English, the
#                                 author's photos, a video and custom
#                                 emoji, where nothing reaches a real
#                                 server. Runs anywhere, on Xvfb too
#                                 (DISPLAY=:99).
#   doc/record-demo.sh columns    doc/img/columns.gif: an antenna and the
#                                 notifications added as columns beside
#                                 home, and scrolled, against the same
#                                 stand-in server.
#   doc/record-demo.sh screens    doc/img/viewer.gif and doc/img/*.png: the
#                                 README's screens (pictures and a video full
#                                 screen, themes, actions, a thread, search,
#                                 the profile editor, the composer, chat,
#                                 accounts, settings), against the same
#                                 stand-in server; nothing typed is sent.
#   doc/record-demo.sh demo       doc/img/demo-account.gif: msky as the
#                                 account you logged in with `msky login`,
#                                 on your own PC: the home timeline with its
#                                 pictures, a picture full screen, a thread
#                                 and the local timeline (not the
#                                 notifications: showing them marks them
#                                 read). msky reads its own config folder;
#                                 this script never reads, copies or prints
#                                 the token, and presses read-only keys only
#                                 (j space v 1 2 esc): nothing is posted,
#                                 reacted to, followed or marked read. It
#                                 refuses to run on CI (CI or GITHUB_ACTIONS
#                                 set). A real server's timelines can show
#                                 anything: look at every frame before
#                                 sharing the GIF, and do not commit it.
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
# The crate is a member of the workspace two directories up, which builds
# into its own target/.
BIN=$(cd ../.. && pwd)/target/release
WORK=$(mktemp -d)
SOCK=unix:$WORK/kitty.sock
mkdir -p "$OUT"
cargo build --release --locked -p misskey-terminal-client

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
  PERMS='["read:account","read:notifications","read:following","read:reactions","write:notes","write:drive","write:following","write:reactions","write:notifications","read:mutes","write:mutes","read:blocks","write:blocks","read:chat","write:chat","write:account","write:report-abuse"]'
  printf '{"service": "http://127.0.0.1:%s", "userId": "u-river", "username": "river", "token": "demo", "permissions": %s}\n' "$PORT" "$PERMS" > "$CFG/accounts/127.0.0.1_$PORT.json"
  # A second made-up account, for the account list.
  printf '{"service": "http://localhost:%s", "userId": "u-alice", "username": "alice", "token": "demo-work", "permissions": %s}\n' "$PORT" "$PERMS" > "$CFG/accounts/localhost_$PORT.json"
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
    --listen-on "$SOCK" --class mskydemo --title msky --directory "${START_DIR:-$(pwd)}" \
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
  # Another theme than bsky's demo, at the same size, so the two sit side
  # by side in the repository's README.
  use_theme dracula
  start 1120 700
  ready "♡"
  record
  # The timeline first: the photos of the top note, the video under it.
  sleep 3
  # The aquarium's photos, full screen: the first and the next, and back.
  # No ready here: reading the screen with kitty's get-text while the
  # photo was being sent left the viewer empty for seconds in the
  # recording, so the viewer gets a fixed time instead.
  send " "
  sleep 4
  send "\033[C"
  sleep 2
  send "\033"
  sleep 1.5
  # Down to the game video, seen in the timeline, played, and back.
  send "j"
  sleep 2
  send " "
  sleep 4
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
  # Down to Mika's note and the profile, and back.
  keys j 2 0.8
  send "\r"
  ready "Aquariums and sea life"
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
  stop "$OUT/demo.gif" 960 8
  quit
}

demo_columns() {
  serve
  start 1400 700
  ready "♡"
  record
  sleep 1.5
  # + offers the antennas, lists and channels: Aquariums goes beside home.
  send "+"
  sleep 1.5
  send "j"
  sleep 0.8
  send "\r"
  sleep 2.5
  keys j 3 0.9
  sleep 1
  # Then the notifications.
  send "+"
  sleep 1.5
  keys j 4 0.6
  send "\r"
  sleep 2.5
  # Back to the home column, and down it.
  keys "\033[D" 2 0.8
  keys j 4 0.9
  sleep 1.5
  stop "$OUT/columns.gif" 1120 8
  quit
}

typed() { # TEXT, a key at a time
  printf '%s' "$1" | fold -w1 | while IFS= read -r c || [ -n "$c" ]; do send "$c"; sleep 0.08; done
}

shot() { # OUTPUT.png
  ffmpeg -nostdin -loglevel error -y -f x11grab -draw_mouse 0 -window_id "$W" -i "${DISPLAY:-:0}" -frames:v 1 "$1"
  ls -la "$1"
}

use_theme() { printf '{"theme": "%s", "language": "en"}\n' "$1" > "$CFG/settings.json"; }

# The README's screens, against the stand-in server: nothing is sent
# anywhere but to it, and nothing typed is sent at all.
screens() {
  serve
  # Pictures and a video full screen: the next picture, then the video.
  use_theme dracula
  start 1120 700
  ready "♡"
  record
  sleep 1
  send " "
  ready "d download"
  sleep 2.5
  send "\033[C"
  sleep 2.5
  send "\033[C"
  sleep 2.5
  send "\033"
  sleep 1
  send "j"
  sleep 1
  send " "
  sleep 5
  send "\033"
  sleep 1
  stop "$OUT/viewer.gif" 720 8
  quit
  # The themes.
  for theme in dracula nord gruvbox catppuccin-latte tokyo-night solarized-light; do
    use_theme "$theme"
    start 960 600
    ready "♡"
    sleep 4
    shot "$OUT/theme-$theme.png"
    quit
  done
  use_theme dracula
  # . on a note.
  start 1120 700
  ready "♡"
  send "j"
  sleep 1
  send "."
  ready "Actions"
  sleep 1.5
  shot "$OUT/actions.png"
  quit
  # A thread.
  start 1120 700
  ready "♡"
  keys j 4 0.6
  send "v"
  ready "Thread"
  sleep 2
  shot "$OUT/thread.png"
  quit
  # Accounts found for "aquarium".
  start 1120 700
  ready "♡"
  send "/"
  sleep 0.6
  typed aquarium
  send "\r"
  sleep 1.5
  send "t"
  ready "following"
  sleep 1.5
  shot "$OUT/search.png"
  quit
  # The profile editor, opened and closed: nothing is saved.
  start 1120 700
  ready "♡"
  send "6"
  ready "Edit profile"
  send "e"
  ready "New avatar"
  sleep 1
  shot "$OUT/editor.png"
  send "\033"
  sleep 0.5
  quit
  # The composer's picture browser, over the demo's photos; not sent.
  mkdir -p "$WORK/pictures"
  cp doc/demo/fish.jpg doc/demo/grapes.jpg doc/demo/salmon.jpg doc/demo/aquarium.jpg "$WORK/pictures/"
  START_DIR="$WORK/pictures" start 1120 700
  ready "♡"
  send "n"
  sleep 0.5
  typed "Photos from the terminal"
  send "\017"
  sleep 1.5
  send "j"
  sleep 2
  shot "$OUT/compose.png"
  send "\033"
  sleep 0.5
  send "\033"
  sleep 0.5
  quit
  # A conversation, a reply typed but not sent.
  start 1120 700
  ready "♡"
  send "7"
  ready "@mika@mi.example"
  send "\r"
  ready "dolphin show"
  send "i"
  sleep 0.3
  typed "See you at the gate"
  sleep 1
  shot "$OUT/chat.png"
  send "\033"
  sleep 0.5
  quit
  # The account list.
  start 1120 700
  ready "♡"
  send "A"
  ready "Accounts"
  sleep 1.5
  shot "$OUT/accounts.png"
  send "\033"
  quit
  # The settings.
  start 1120 700
  ready "♡"
  send "s"
  ready "Settings"
  sleep 1
  shot "$OUT/settings.png"
  send "\033"
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
  stop "$OUT/demo-account.gif"
  quit
}

case "${1:-demo}" in
  demo) demo ;;
  demo-fake) demo_fake ;;
  columns) demo_columns ;;
  screens) screens ;;
  *) echo "usage: $0 [demo|demo-fake]" >&2; exit 2 ;;
esac
