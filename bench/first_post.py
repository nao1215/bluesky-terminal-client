#!/usr/bin/env python3
"""Start the client on a pseudo-terminal and quit it as soon as its first
post is on screen: what a user waits for after typing bsky.

    python3 bench/first_post.py BSKY TERMINAL_MS SERVER_MS

The terminal answers the client's questions about what it can draw after
TERMINAL_MS (a terminal over ssh answers a round trip later), and a
stand-in server answers every request after SERVER_MS. Exits 0 once the
post is drawn and the client has quit, 1 if it is not drawn in 10 s.
"""

import fcntl
import json
import os
import pty
import select
import struct
import sys
import tempfile
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

bsky = sys.argv[1]
terminal_delay = int(sys.argv[2]) / 1000
server_delay = int(sys.argv[3]) / 1000
MARK = "FIRSTPOSTMARK"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        time.sleep(server_delay)
        if self.path.split("?")[0].endswith("getTimeline"):
            body = {"feed": [{"post": {
                "uri": "at://did:plc:me/app.bsky.feed.post/1", "cid": "c",
                "author": {"did": "did:plc:me", "handle": "me.test"},
                "record": {"text": MARK, "createdAt": "2026-09-20T10:00:00.000Z"},
                "indexedAt": "2026-09-20T10:00:00.000Z"}}]}
        else:
            body = {"notifications": [], "preferences": [], "feeds": []}
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


class Server(ThreadingHTTPServer):
    def handle_error(self, *args):
        pass  # the client quit before an answer it no longer needs


def main():
    server = Server(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    cfg = tempfile.mkdtemp()
    os.makedirs(f"{cfg}/accounts")
    with open(f"{cfg}/accounts/did_plc_me.json", "w") as f:
        json.dump({"service": f"http://127.0.0.1:{server.server_address[1]}", "did": "did:plc:me",
                   "handle": "me.test", "accessJwt": "a", "refreshJwt": "r"}, f)
    with open(f"{cfg}/accounts.json", "w") as f:
        f.write('{"current": "did:plc:me"}')
    pid, fd = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        os.environ.update(BSKY_CONFIG_DIR=cfg, BSKY_CACHE_DIR="off", TERM="xterm-256color")
        os.execv(bsky, [bsky])
    start = time.monotonic()
    out = b""
    asked = None
    answered = shown = False
    while time.monotonic() - start < 10:
        ready, _, _ = select.select([fd], [], [], 0.002)
        if ready:
            try:
                out += os.read(fd, 65536)
            except OSError:
                break
        now = time.monotonic()
        if asked is None and b"\x1b[5n" in out:
            asked = now
        if not answered and asked is not None and now - asked >= terminal_delay:
            # A terminal without pictures: VT220 attributes, then "OK".
            os.write(fd, b"\x1b[?62;4c\x1b[0n")
            answered = True
        if not shown and MARK.encode() in out:
            shown = True
            os.write(fd, b"q")
    if not shown:
        os.kill(pid, 9)
    os.waitpid(pid, 0)
    sys.exit(0 if shown else 1)


if __name__ == "__main__":
    main()
