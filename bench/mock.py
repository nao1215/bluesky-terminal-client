#!/usr/bin/env python3
"""A stand-in Bluesky server for the benchmarks: fixed, made-up data sized
like a busy account, answered at once, so what is measured is bsky's own
work (start-up, parsing, laying out text) rather than a network.

    python3 bench/mock.py WORKDIR

Serves on 127.0.0.1 at a free port, writes the port to WORKDIR/port, and
answers the XRPC calls bsky makes.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

N = 100
TEXTS = [
    "今日は山に登りました 🏔️ The view from the top was worth every step, and the clouds rolled in just as we left 👨‍👩‍👧 #hiking https://example.com/trip また行きたい！ @bob.test",
    "Rust 1.98 is out. The new lint caught two bugs in my parser before lunch.",
    "Reading list for the weekend: two papers on CRDTs and a novel. 📚🇯🇵",
    "駅前のパン屋で朝ごはん。🥐 été 1️⃣ ❤️",
]


def author(i):
    return {
        "did": f"did:plc:a{i % 20}",
        "handle": f"author{i % 20}.test",
        "displayName": f"Author {i % 20} 🌸",
        "viewer": {"following": f"at://did:plc:bench/app.bsky.graph.follow/{i % 20}"},
    }


def post(i, reply_to=None):
    record = {"$type": "app.bsky.feed.post", "text": TEXTS[i % len(TEXTS)] * (1 + i % 3),
              "createdAt": f"2026-09-20T{i % 24:02d}:{i % 60:02d}:00.000Z"}
    if reply_to:
        record["reply"] = {"root": reply_to, "parent": reply_to}
    return {
        "uri": f"at://did:plc:a{i % 20}/app.bsky.feed.post/p{i}",
        "cid": f"cid-p{i}",
        "author": author(i),
        "record": record,
        "likeCount": i, "repostCount": i % 7, "replyCount": i % 5,
        "indexedAt": record["createdAt"],
    }


ROOT = post(0)
ROOT_REF = {"uri": ROOT["uri"], "cid": ROOT["cid"]}


def answer(path, query):
    if path == "app.bsky.feed.getTimeline":
        return {"feed": [{"post": post(i)} for i in range(N)]}
    if path == "app.bsky.feed.searchPosts":
        return {"posts": [post(i) for i in range(N)]}
    if path == "app.bsky.notification.listNotifications":
        return {"notifications": [
            {"uri": f"at://did:plc:a{i % 20}/app.bsky.feed.post/r{i}", "cid": f"c{i}", "author": author(i),
             "reason": "reply", "reasonSubject": ROOT["uri"],
             "record": {"$type": "app.bsky.feed.post", "text": TEXTS[i % len(TEXTS)], "reply": {"root": ROOT_REF, "parent": ROOT_REF}},
             "isRead": i > 10, "indexedAt": f"2026-09-20T10:{i % 60:02d}:00.000Z"}
            for i in range(N)]}
    if path == "app.bsky.feed.getPosts":
        return {"posts": [ROOT]}
    if path == "app.bsky.feed.getPostThread":
        return {"thread": {"$type": "app.bsky.feed.defs#threadViewPost", "post": ROOT, "replies": [
            {"$type": "app.bsky.feed.defs#threadViewPost", "post": post(i, ROOT_REF), "replies": []}
            for i in range(1, N + 1)]}}
    if path == "app.bsky.actor.getPreferences":
        return {"preferences": []}
    if path == "app.bsky.feed.getFeedGenerators":
        return {"feeds": []}
    if path == "app.bsky.notification.updateSeen":
        return {}
    return None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self):
        url = urlparse(self.path)
        body = answer(url.path[len("/xrpc/"):], parse_qs(url.query)) if url.path.startswith("/xrpc/") else None
        if body is None:
            data = json.dumps({"error": "NotFound", "message": url.path}).encode()
            self.send_response(404)
        else:
            data = json.dumps(body).encode()
            self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.reply()

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        self.reply()


def main():
    work = Path(sys.argv[1])
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    (work / "port.tmp").write_text(str(server.server_address[1]))
    (work / "port.tmp").rename(work / "port")
    server.serve_forever()


if __name__ == "__main__":
    main()
