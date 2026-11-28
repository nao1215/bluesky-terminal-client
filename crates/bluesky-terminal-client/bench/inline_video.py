#!/usr/bin/env python3
"""Measure j -> first complete video frame on a kitty-protocol PTY.

    python3 bench/inline_video.py /path/to/bsky --delay-ms 150 --runs 5

Each fresh process starts on a text post, waits one second with a video
immediately below it, then selects that video. Only video requests have
the fixed delay. The end point is the synchronized-output end following
the first video transmission, not its thumbnail or the HTTP response.
Uses the same HLS fixture as the atago suite; no external server/account.
"""

import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import statistics
import struct
import tempfile
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

FIXTURES = Path(__file__).resolve().parent.parent / "e2e/atago/testdata"
MARK = b"READY_TO_SELECT_VIDEO"
VIDEO = re.compile(rb"\x1b_G[^;]*i=(?:11881305|11881306),[^;]*a=T[,;]")


class Server(ThreadingHTTPServer):
    def handle_error(self, *args):
        pass  # a cancelled read-ahead can close its socket before the reply


def measure(binary, delay, dwell):
    hits = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_HEAD(self):
            self.send_response(200)
            self.end_headers()

        def do_GET(self):
            path = self.path.split("?")[0]
            if path.startswith("/watch/"):
                hits.append((path, time.perf_counter()))
                time.sleep(delay)
                data = (FIXTURES / "hls" / path.removeprefix("/watch/")).read_bytes()
            elif path == "/thumbnail.png":
                data = (FIXTURES / "photo.png").read_bytes()
            else:
                body = {"notifications": [], "preferences": [], "feeds": []}
                if path.endswith("getTimeline"):
                    author = {"did": "did:plc:me", "handle": "me.test"}
                    posts = [{
                        "uri": f"at://did:plc:me/app.bsky.feed.post/{i}", "cid": f"c{i}",
                        "author": author,
                        "record": {"text": text, "createdAt": "2026-09-20T10:00:00Z"},
                    } for i, text in enumerate([MARK.decode(), "VIDEO_POST"])]
                    posts[1]["embed"] = {
                        "$type": "app.bsky.embed.video#view", "cid": "video",
                        "playlist": f"{base}/watch/playlist.m3u8",
                        "thumbnail": f"{base}/thumbnail.png",
                        "aspectRatio": {"width": 16, "height": 9},
                    }
                    body = {"feed": [{"post": p} for p in posts]}
                data = json.dumps(body).encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    server = Server(("127.0.0.1", 0), Handler)
    base = f"http://127.0.0.1:{server.server_port}"
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory() as cfg:
        accounts = Path(cfg) / "accounts"
        accounts.mkdir()
        (accounts / "did_plc_me.json").write_text(json.dumps({
            "service": base, "did": "did:plc:me", "handle": "me.test",
            "accessJwt": "a", "refreshJwt": "r",
        }))
        (Path(cfg) / "accounts.json").write_text('{"current":"did:plc:me"}')
        pid, fd = pty.fork()
        if pid == 0:
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 100, 0, 0))
            os.environ.update(BSKY_CONFIG_DIR=cfg, BSKY_CACHE_DIR="off",
                              BSKY_INLINE_VIDEO="1", TERM="xterm-kitty", LC_ALL="C.UTF-8")
            for name in ("NO_COLOR", "TMUX", "BSKY_PICTURES"):
                os.environ.pop(name, None)
            os.execv(binary, [binary])
        out = b""
        started = time.perf_counter()
        ready_at = pressed_at = None
        answered = False
        result = None
        try:
            while time.perf_counter() - started < 10:
                if select.select([fd], [], [], 0.002)[0]:
                    out += os.read(fd, 65536)
                now = time.perf_counter()
                if not answered and b"\x1b[5n" in out:
                    os.write(fd, b"\x1b_Gi=31;OK\x1b\\\x1b[6;20;10t\x1b[?62;4c\x1b[0n")
                    answered = True
                if ready_at is None and MARK in out:
                    ready_at = now
                if pressed_at is None and ready_at is not None and now - ready_at >= dwell:
                    out = b""
                    pressed_at = time.perf_counter()
                    os.write(fd, b"j")
                if pressed_at is not None:
                    video = VIDEO.search(out)
                    if video and b"\x1b[?2026l" in out[video.end():]:
                        result = (now - pressed_at) * 1000
                        break
            if result is None:
                raise RuntimeError(f"no video frame within 10 s; requests={hits}; tail={out[-500:]!r}")
            return {"ms": round(result, 2),
                    "video_requests_before_key": sum(at < pressed_at for _, at in hits),
                    "video_requests_after_key": sum(at >= pressed_at for _, at in hits)}
        finally:
            os.kill(pid, signal.SIGTERM)
            os.waitpid(pid, 0)
            os.close(fd)
            server.shutdown()
            server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--delay-ms", type=int, default=150)
    parser.add_argument("--dwell-ms", type=int, default=1000)
    parser.add_argument("--runs", type=int, default=5)
    args = parser.parse_args()
    results = [measure(str(args.binary.resolve()), args.delay_ms / 1000, args.dwell_ms / 1000)
               for _ in range(args.runs)]
    print(json.dumps({"delay_ms": args.delay_ms, "dwell_ms": args.dwell_ms,
                      "median_ms": statistics.median(r["ms"] for r in results),
                      "runs": results}, indent=2))


if __name__ == "__main__":
    main()
