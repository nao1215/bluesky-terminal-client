#!/usr/bin/env python3
"""A stand-in Bluesky server with made-up accounts, posts and messages, for
recording the README's pictures of screens that would otherwise show a real
account's private data (direct messages) or change it (marking messages
read, moving its session file to the layout of several accounts).

    doc/demo-server.py PORTFILE

Serves on 127.0.0.1 at a free port, writes the port to PORTFILE, and answers
the XRPC calls bsky makes with fixed data. Every name here is made up.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

TESTDATA = Path(__file__).resolve().parent.parent / "e2e" / "atago" / "testdata"
BASE = ""  # set once the port is known


def img(name):
    return f"{BASE}/img/{name}"


ME = {"did": "did:plc:river", "handle": "river.example", "displayName": "River"}
ALICE = {"did": "did:plc:alice", "handle": "alice.example", "displayName": "Alice Chen",
         "avatar": img("alice.png"),
         "viewer": {"following": "at://did:plc:river/app.bsky.graph.follow/a"}}
BOB = {"did": "did:plc:bob", "handle": "bob.example", "displayName": "Bob Tanaka",
       "avatar": img("bob.png"),
       "viewer": {"following": "at://did:plc:river/app.bsky.graph.follow/b"}}
CAROL = {"did": "did:plc:carol", "handle": "carol.example", "displayName": "Carol"}


def post(author, rkey, text, when, likes=0, reposts=0, replies=0, embed=None):
    p = {
        "uri": f"at://{author['did']}/app.bsky.feed.post/{rkey}",
        "cid": f"cid-{rkey}",
        "author": author,
        "record": {"$type": "app.bsky.feed.post", "text": text, "createdAt": when},
        "likeCount": likes, "repostCount": reposts, "replyCount": replies,
        "indexedAt": when,
    }
    if embed:
        p["embed"] = embed
    return p


def fix_urls():
    for a in (ALICE, BOB):
        a["avatar"] = img("alice.png" if a is ALICE else "bob.png")


def timeline():
    return [
        post(ALICE, "t1", "Finished the trail before the rain. The view from the ridge was worth every step.",
             "2026-09-23T09:40:00.000Z", 12, 3, 2,
             {"$type": "app.bsky.embed.images#view",
              "images": [{"thumb": img("photo.png"), "fullsize": img("photo.png"), "alt": "the ridge",
                          "aspectRatio": {"width": 4, "height": 3}}]}),
        post(BOB, "t2", "Rust 1.98 is out. The new lint caught two bugs in my parser before lunch.",
             "2026-09-23T08:15:00.000Z", 30, 8, 5),
        post(ALICE, "t3", "今日は早起きして、駅前のパン屋で朝ごはん。🥐", "2026-09-23T07:02:00.000Z", 6, 0, 1),
        post(BOB, "t4", "Reading list for the weekend: two papers on CRDTs and a novel.",
             "2026-09-22T21:30:00.000Z", 4, 1, 0),
    ]


def cats():
    return [
        post(CAROL, "c1", "She has decided the keyboard is the warmest place in the house.",
             "2026-09-23T09:00:00.000Z", 88, 12, 9,
             {"$type": "app.bsky.embed.images#view",
              "images": [{"thumb": img("alice.png"), "fullsize": img("alice.png"), "alt": "a cat",
                          "aspectRatio": {"width": 1, "height": 1}}]}),
        post(CAROL, "c2", "Nap number four of the day.", "2026-09-23T06:00:00.000Z", 41, 2, 3),
    ]


def notifications():
    return [
        {"uri": "at://did:plc:bob/app.bsky.feed.post/r1", "cid": "c", "author": BOB, "reason": "reply",
         "record": {"$type": "app.bsky.feed.post", "text": "Which trail was it? I want to try it next month."},
         "isRead": False, "indexedAt": "2026-09-23T09:50:00.000Z"},
        {"uri": "at://did:plc:alice/app.bsky.feed.like/l1", "cid": "c", "author": ALICE, "reason": "like",
         "reasonSubject": "at://did:plc:river/app.bsky.feed.post/m1",
         "record": {"$type": "app.bsky.feed.like"}, "isRead": False, "indexedAt": "2026-09-23T09:10:00.000Z"},
        {"uri": "at://did:plc:carol/app.bsky.graph.follow/f1", "cid": "c", "author": CAROL, "reason": "follow",
         "record": {"$type": "app.bsky.graph.follow"}, "isRead": True, "indexedAt": "2026-09-22T18:00:00.000Z"},
    ]


def message(mid, sender, text, when):
    return {"$type": "chat.bsky.convo.defs#messageView", "id": mid, "rev": mid, "text": text,
            "sender": {"did": sender}, "sentAt": when}


CONVO_A = [
    message("a1", "did:plc:alice", "Are you still up for the hike on Saturday?", "2026-09-23T08:00:00.000Z"),
    message("a2", "did:plc:river", "Yes. Meet at the station at 7?", "2026-09-23T08:04:00.000Z"),
    message("a3", "did:plc:alice", "7 works. I will bring the map and some coffee.", "2026-09-23T08:06:00.000Z"),
    message("a4", "did:plc:alice", "The forecast says clear skies after 9. 🌤", "2026-09-23T09:30:00.000Z"),
]
CONVO_B = [
    message("b1", "did:plc:bob", "Thanks for the review, the fix is merged.", "2026-09-22T20:00:00.000Z"),
]


def convos():
    return [
        {"id": "convo-alice", "rev": "r", "members": [ME, ALICE], "lastMessage": CONVO_A[-1],
         "muted": False, "unreadCount": 1},
        {"id": "convo-bob", "rev": "r", "members": [ME, BOB], "lastMessage": CONVO_B[-1],
         "muted": False, "unreadCount": 0},
    ]


def answer(path, query):
    if path == "app.bsky.feed.getTimeline":
        return {"feed": [{"post": p} for p in timeline()]}
    if path == "app.bsky.feed.getFeed":
        return {"feed": [{"post": p} for p in cats()]}
    if path == "app.bsky.actor.getPreferences":
        return {"preferences": [{"$type": "app.bsky.actor.defs#savedFeedsPrefV2", "items": [
            {"id": "1", "type": "timeline", "value": "following", "pinned": True},
            {"id": "2", "type": "feed", "value": "at://did:plc:carol/app.bsky.feed.generator/cats", "pinned": True},
        ]}]}
    if path == "app.bsky.feed.getFeedGenerators":
        return {"feeds": [{"uri": "at://did:plc:carol/app.bsky.feed.generator/cats", "cid": "c",
                           "did": "did:web:feeds.example", "creator": CAROL, "displayName": "Cats",
                           "indexedAt": "2026-09-01T00:00:00.000Z"}]}
    if path == "app.bsky.notification.listNotifications":
        return {"notifications": notifications()}
    if path == "app.bsky.feed.searchPosts":
        return {"posts": [p for p in timeline() + cats() if "Rust" in p["record"]["text"] or "trail" in p["record"]["text"]]}
    if path == "app.bsky.actor.getProfile":
        return dict(ME, followersCount=128, followsCount=64, postsCount=512, description="Walks, code, and bread.")
    if path == "app.bsky.feed.getAuthorFeed":
        return {"feed": []}
    if path == "chat.bsky.convo.listConvos":
        return {"convos": convos()}
    if path == "chat.bsky.convo.getMessages":
        msgs = CONVO_A if query.get("convoId", [""])[0] == "convo-alice" else CONVO_B
        return {"messages": list(reversed(msgs))}
    if path in ("chat.bsky.convo.updateRead", "app.bsky.notification.updateSeen"):
        return {}
    if path == "app.bsky.feed.getPosts":
        return {"posts": [post(BOB, "r1", "Which trail was it? I want to try it next month.",
                               "2026-09-23T09:50:00.000Z")]}
    if path == "com.atproto.identity.resolveHandle":
        return {"did": "did:plc:alice"}
    return None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self):
        url = urlparse(self.path)
        if url.path.startswith("/img/"):
            name = {"alice.png": "avatar-alice.png", "bob.png": "avatar-bob.png",
                    "photo.png": "photo.png"}.get(url.path[5:])
            if name:
                data = (TESTDATA / name).read_bytes()
                self.send_response(200)
                self.send_header("Content-Type", "image/png")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
        body = None
        if url.path.startswith("/xrpc/"):
            body = answer(url.path[6:], parse_qs(url.query))
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
        length = int(self.headers.get("Content-Length") or 0)
        self.rfile.read(length)
        self.reply()


def main():
    global BASE
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    BASE = f"http://127.0.0.1:{server.server_address[1]}"
    fix_urls()
    Path(sys.argv[1]).write_text(str(server.server_address[1]))
    server.serve_forever()


if __name__ == "__main__":
    main()
