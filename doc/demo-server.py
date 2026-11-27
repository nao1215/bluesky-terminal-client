#!/usr/bin/env python3
"""A stand-in Misskey server with made-up accounts and notes, for recording
the README's pictures without showing a real account, as bsky's
doc/demo-server.py does for Bluesky.

    doc/demo-server.py PORTFILE

Serves on 127.0.0.1 at a free port, writes the port to PORTFILE, and answers
the API calls msky's client makes with fixed data. Every name here is made
up; the pictures are those of doc/demo.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PICTURES = Path(__file__).resolve().parent / "demo"
BASE = ""  # set once the port is known
# Written in place of the server's address, which is known only once it
# serves, and put back in every answer.
HERE = "@@HERE@@"


def img(name):
    return f"{HERE}/img/{name}"


def user(username, name, avatar=None, host=None):
    u = {"id": f"u-{username}", "username": username, "host": host, "name": name}
    if avatar:
        u["avatarUrl"] = img(avatar)
    return u


ME = user("river", "River", "carol.jpg")
ALICE = user("alice", "Alice Chen", "alice.jpg")
BOB = user("bob", "ボブ 田中", "bob.jpg")
MIKA = user("mika", "みか 🐈", "carol.jpg", host="mi.example")
KEN = user("ken", "Ken", None)


def photo(fid, name, w, h, comment="", sensitive=False):
    return {"id": fid, "name": name, "type": "image/jpeg", "size": 40000,
            "url": img(name), "thumbnailUrl": img(name), "comment": comment,
            "isSensitive": sensitive, "properties": {"width": w, "height": h}}


def note(nid, u, text, when, reactions=None, renotes=0, replies=0, files=None, cw=None, **extra):
    n = {"id": nid, "createdAt": when, "userId": u["id"], "user": u, "text": text, "cw": cw,
         "reactions": reactions or {}, "renoteCount": renotes, "repliesCount": replies,
         "files": files or []}
    n.update(extra)
    return n


def timeline():
    owl = note("n1", ALICE, "森で見つけたフクロウ 🦉 じっとこっちを見てた\nThe owl watched us the whole time.",
               "2026-11-27T09:41:00.000Z", {"❤️": 24, ":blobcat@.:": 8, "👍🏽": 5}, 6, 2,
               [photo("f1", "owl.jpg", 800, 420, "a brown owl on a branch")])
    lunch = note("n2", BOB, "今日のお昼 🍔 ポテトもつけた", "2026-11-27T09:12:00.000Z",
                 {"🍔": 12, "🤤": 7}, 1, 1, [photo("f2", "burger.jpg", 800, 600, "a burger and fries")])
    spoiler = note("n3", MIKA, "犯人は最初に出てきた執事でした 👨‍👩‍👧", "2026-11-27T08:55:00.000Z",
                   {"😱": 9}, 0, 4, cw="映画のネタバレ 🎬")
    desk = note("n4o", KEN, "New desk setup, finally. 日本語の本も並べた 📚", "2026-11-27T07:30:00.000Z",
                {"✨": 15}, 3, 0, [photo("f4", "desk.jpg", 800, 534, "a desk with a laptop and books")])
    renote = {"id": "n4", "createdAt": "2026-11-27T08:20:00.000Z", "userId": ALICE["id"], "user": ALICE,
              "text": None, "cw": None, "files": [], "reactions": {}, "renoteId": "n4o", "renote": desk}
    reading = note("n5", ME, "週末は読書 📖 おすすめある？", "2026-11-27T07:02:00.000Z",
                   {"👀": 3}, 0, 3, [photo("f5", "reading.jpg", 800, 450, "an open book")])
    quote = note("n6", BOB, "これいいな 🇯🇵 🇺🇸", "2026-11-27T06:40:00.000Z", {"👍": 2},
                 renoteId="n5", renote=reading)
    plain = [note(f"p{i}", [ALICE, BOB, KEN][i % 3],
                  ["おはようございます ☀️", "Coffee first ☕", "雨が降ってきた ☔ 傘ない",
                   "Shipping a release today 🚀", "猫が膝から降りない 🐈‍⬛"][i % 5],
                  f"2026-11-27T0{5 - i // 3}:{50 - i * 4:02d}:00.000Z", {"👍": i + 1}, 0, 0)
             for i in range(9)]
    return [owl, lunch, spoiler, renote, reading, quote] + plain


def local():
    return [note(f"l{i}", [ALICE, BOB, ME][i % 3], t, f"2026-11-27T09:{50 - i * 3:02d}:00.000Z", {"👍": i})
            for i, t in enumerate(["ローカルのみなさんこんにちは 👋", "Misskey のカスタム絵文字 :blobcat: はそのまま",
                                   "Local timeline, made-up notes only", "家族旅行の写真はまた今度 👨‍👩‍👧"])]


def notifications():
    return [
        {"id": "x1", "createdAt": "2026-11-27T09:45:00.000Z", "type": "reaction", "reaction": ":blobcat@.:",
         "user": ALICE, "note": timeline()[4]},
        {"id": "x2", "createdAt": "2026-11-27T09:30:00.000Z", "type": "reply", "user": BOB,
         "note": note("r1", BOB, "ミステリーなら『十角館』かな", "2026-11-27T09:30:00.000Z")},
        {"id": "x3", "createdAt": "2026-11-27T09:00:00.000Z", "type": "follow", "user": MIKA},
        {"id": "x4", "createdAt": "2026-11-27T08:40:00.000Z", "type": "renote", "user": KEN,
         "note": {"id": "rn9", "renoteId": "n5", "renote": timeline()[4], "user": KEN}},
    ]


def find(nid):
    for n in timeline() + local():
        if n["id"] == nid:
            return n
        if n.get("renote") and n["renote"]["id"] == nid:
            return n["renote"]
    return None


def children(nid):
    if nid == "n5":
        return [note("c1", BOB, "ミステリーなら『十角館』かな", "2026-11-27T09:30:00.000Z", replies=1),
                note("c2", ALICE, "Try “Project Hail Mary” 🚀", "2026-11-27T09:35:00.000Z")]
    if nid == "c1":
        return [note("c1a", ME, "ありがとう、読んでみる 🙏", "2026-11-27T09:38:00.000Z")]
    return []


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, status, body, kind="application/json"):
        if isinstance(body, bytes):
            data = body
        else:
            data = json.dumps(body, ensure_ascii=False).replace(HERE, BASE).encode()
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.startswith("/img/"):
            p = PICTURES / Path(self.path[5:]).name
            if p.is_file():
                return self.send(200, p.read_bytes(), "image/jpeg")
        self.send(404, {"error": {"code": "NOT_FOUND"}})

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(length) or b"{}")
        until = body.get("untilId")
        path = self.path
        if path in ("/api/notes/timeline", "/api/notes/hybrid-timeline", "/api/notes/global-timeline"):
            return self.send(200, [] if until else timeline())
        if path == "/api/notes/local-timeline":
            return self.send(200, [] if until else local())
        if path == "/api/i/notifications":
            return self.send(200, [] if until else notifications())
        if path == "/api/notes/show":
            n = find(body.get("noteId"))
            return self.send(200, n) if n else self.send(400, {"error": {"code": "NO_SUCH_NOTE"}})
        if path == "/api/notes/children":
            return self.send(200, children(body.get("noteId")))
        if path == "/api/notes/conversation":
            return self.send(200, [])
        if path == "/api/i":
            return self.send(200, ME)
        if path == "/api/meta":
            return self.send(200, {"name": "Demo", "maxNoteTextLength": 3000})
        self.send(404, {"error": {"code": "NOT_FOUND"}})


def main():
    global BASE
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    BASE = f"http://127.0.0.1:{server.server_address[1]}"
    Path(sys.argv[1]).write_text(str(server.server_address[1]))
    server.serve_forever()


if __name__ == "__main__":
    main()
