#!/usr/bin/env python3
"""Start the client on a pseudo-terminal and quit it as soon as its first
note is on screen: what a user waits for after typing msky.

    python3 bench/first_post.py MSKY TERMINAL_MS SERVER_MS

The terminal answers the client's questions about what it can draw after
TERMINAL_MS (a terminal over ssh answers a round trip later), and a
stand-in server answers every request after SERVER_MS. Exits 0 once the
note is drawn and the client has quit, 1 if it is not drawn in 10 s.
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

msky = sys.argv[1]
terminal_delay = int(sys.argv[2]) / 1000
server_delay = int(sys.argv[3]) / 1000
MARK = "FIRSTPOSTMARK"
TIMELINES = ("notes/timeline", "notes/local-timeline", "notes/hybrid-timeline", "notes/global-timeline")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        time.sleep(server_delay)
        endpoint = self.path.split("?")[0][len("/api/"):]
        if endpoint in TIMELINES:
            body = [{"id": "n1", "createdAt": "2026-11-20T10:00:00.000Z", "userId": "u-me",
                     "user": {"id": "u-me", "username": "me"}, "text": MARK}]
        elif endpoint == "i":
            body = {"id": "u-me", "username": "me"}
        elif endpoint == "emojis":
            body = {"emojis": []}
        elif endpoint == "meta":
            body = {}
        else:
            body = []
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
    port = server.server_address[1]
    cfg = tempfile.mkdtemp()
    os.makedirs(f"{cfg}/accounts")
    with open(f"{cfg}/accounts/127.0.0.1_{port}.json", "w") as f:
        json.dump({"service": f"http://127.0.0.1:{port}", "userId": "u-me",
                   "username": "me", "token": "bench-token"}, f)
    with open(f"{cfg}/accounts.json", "w") as f:
        json.dump({"current": f"127.0.0.1:{port}"}, f)
    pid, fd = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
        os.environ.update(MSKY_CONFIG_DIR=cfg, MSKY_CACHE_DIR="off", TERM="xterm-256color",
                          LC_ALL="C.UTF-8")
        os.execv(msky, [msky])
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
