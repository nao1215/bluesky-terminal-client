#!/usr/bin/env python3
"""A stand-in Misskey server for the benchmarks: fixed, made-up data sized
like a busy account, answered at once, so what is measured is msky's own
work (start-up, parsing, laying out text) rather than a network.

    python3 bench/mock.py WORKDIR

Serves on 127.0.0.1 at a free port, writes the port to WORKDIR/port, and
answers the API calls msky makes (POST /api/<endpoint>).
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

N = 100
TEXTS = [
    "今日は山に登りました 🏔️ The view from the top was worth every step, and the clouds rolled in just as we left 👨‍👩‍👧 #hiking https://example.com/trip また行きたい！ @bob@misskey.test",
    "Rust 1.98 is out. The new lint caught two bugs in my parser before lunch. $[x2 **すごい**] :blobcat:",
    "Reading list for the weekend: two papers on CRDTs and a novel. 📚🇯🇵 [link](https://example.com/list)",
    "駅前のパン屋で朝ごはん。🥐 été 1️⃣ ❤️ <center>$[ruby 朝 あさ]</center>",
]


def user(i):
    return {
        "id": f"u{i % 20}",
        "username": f"author{i % 20}",
        "host": None if i % 2 else "other.test",
        "name": f"Author {i % 20} 🌸",
        "avatarUrl": None,
        "emojis": {},
    }


def note(i, reply_id=None):
    return {
        "id": f"n{N - i:04d}",
        "createdAt": f"2026-11-20T{i % 24:02d}:{i % 60:02d}:00.000Z",
        "userId": f"u{i % 20}",
        "user": user(i),
        "text": TEXTS[i % len(TEXTS)] * (1 + i % 3),
        "cw": "ネタバレ" if i % 9 == 0 else None,
        "visibility": "public",
        "renoteCount": i % 7,
        "repliesCount": i % 5,
        "reactions": {"👍": i, ":blobcat@.:": i % 3},
        "reactionEmojis": {},
        "emojis": {},
        "files": [],
        "replyId": reply_id,
        "renoteId": None,
    }


ROOT = dict(note(0), repliesCount=N)
ME = {"id": "u-bench", "username": "bench", "host": None, "name": "Bench", "emojis": {}}


def answer(endpoint, body):
    # A later page brings nothing new, which ends the reading.
    later = bool(body.get("untilId"))
    if endpoint in ("notes/timeline", "notes/local-timeline", "notes/hybrid-timeline",
                    "notes/global-timeline", "notes/search", "users/notes"):
        return [] if later else [note(i) for i in range(N)]
    if endpoint == "i/notifications":
        return [] if later else [
            {"id": f"x{N - i:04d}", "createdAt": f"2026-11-20T10:{i % 60:02d}:00.000Z",
             "type": "reply", "user": user(i), "note": note(i, ROOT["id"])}
            for i in range(1, N + 1)]
    if endpoint == "notes/show":
        return ROOT
    if endpoint == "notes/children":
        return [] if later else [note(i, ROOT["id"]) for i in range(1, N + 1)]
    if endpoint == "notes/conversation":
        return []
    if endpoint == "i":
        return ME
    if endpoint == "emojis":
        return {"emojis": []}
    if endpoint == "meta":
        return {"maxNoteTextLength": 3000}
    return None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        try:
            body = json.loads(raw or b"{}")
        except ValueError:
            body = {}
        path = self.path.split("?")[0]
        got = answer(path[len("/api/"):], body if isinstance(body, dict) else {}) if path.startswith("/api/") else None
        if got is None:
            data = json.dumps({"error": {"code": "NO_SUCH_ENDPOINT", "message": path}}).encode()
            self.send_response(404)
        else:
            data = json.dumps(got).encode()
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main():
    work = Path(sys.argv[1])
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    (work / "port.tmp").write_text(str(server.server_address[1]))
    (work / "port.tmp").rename(work / "port")
    server.serve_forever()


if __name__ == "__main__":
    main()
