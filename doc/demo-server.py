#!/usr/bin/env python3
"""A stand-in Misskey server with made-up accounts and notes, for recording
the README's pictures without showing a real account, as bsky's
doc/demo-server.py does for Bluesky.

    doc/demo-server.py PORTFILE

Serves on 127.0.0.1 at a free port, writes the port to PORTFILE, and answers
the API calls msky's client makes with fixed data. Every name here is made
up; the pictures are those of doc/demo: photos and a video by msky's
author, and custom emoji cut from them.
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
MIKA = user("mika", "みか :iruka:", "carol.jpg", host="mi.example")
KEN = user("ken", "Ken", None)
MIKA["emojis"] = {"iruka": img("emoji-iruka.png")}

# The server's custom emoji, cut from the demo's own photos.
EMOJIS = ["iruka", "todo", "budou", "ebi", "salmon"]


def photo(fid, name, w, h, comment="", sensitive=False):
    return {"id": fid, "name": name, "type": "image/jpeg", "size": 90000,
            "url": img(name), "thumbnailUrl": img(name), "comment": comment,
            "isSensitive": sensitive, "properties": {"width": w, "height": h}}


def video(fid, name, poster, w, h, comment=""):
    return {"id": fid, "name": name, "type": "video/mp4", "size": 885000,
            "url": img(name), "thumbnailUrl": img(poster), "comment": comment,
            "isSensitive": False, "properties": {"width": w, "height": h}}


def note(nid, u, text, when, reactions=None, renotes=0, replies=0, files=None, cw=None, **extra):
    n = {"id": nid, "createdAt": when, "userId": u["id"], "user": u, "text": text, "cw": cw,
         "reactions": reactions or {}, "renoteCount": renotes, "repliesCount": replies,
         "files": files or []}
    if u.get("emojis"):
        n["emojis"] = u["emojis"]
    n.update(extra)
    return n


def timeline():
    show = note("n1", MIKA, "イルカショー、ジャンプが高すぎる :iruka: :iruka:\n水しぶきがここまで来た 💦",
                "2026-11-27T09:41:00.000Z", {":iruka@.:": 31, "❤️": 18, "👏🏽": 6}, 9, 2,
                [video("v1", "dolphin.mp4", "dolphin.jpg", 640, 480, "a dolphin jumps at an aquarium show")])
    aquarium = note("n2", ALICE, "水族館に来た 🐟 ターポンの群れ、近くで見ると大きい\nThe tarpon are huge up close.",
                    "2026-11-27T09:20:00.000Z", {"🐟": 14, ":todo@.:": 9, "✨": 5}, 4, 1,
                    [photo("f1", "fish.jpg", 800, 600, "big silver fish in a blue tank"),
                     photo("f2", "sealions.jpg", 800, 600, "sea lions on a rock"),
                     photo("f3", "pup.jpg", 800, 1066, "a poster: a sea lion pup was born"),
                     photo("f4", "turtles.jpg", 800, 600, "turtles in a pool")])
    spoiler = note("n3", KEN, "犯人は最初に出てきた執事でした 👨‍👩‍👧", "2026-11-27T09:05:00.000Z",
                   {"😱": 9}, 0, 4, cw="映画のネタバレ 🎬")
    sample = note("n4o", BOB, "食品サンプルなのにお腹が空く :salmon: :ebi:", "2026-11-27T08:30:00.000Z",
                  {":ebi@.:": 12, "🤤": 7}, 3, 0,
                  [photo("f5", "salmon.jpg", 800, 600, "a salmon bowl, a food sample"),
                   photo("f6", "shrimp.jpg", 800, 600, "sweet shrimp sashimi, a food sample")])
    renote = {"id": "n4", "createdAt": "2026-11-27T08:50:00.000Z", "userId": ALICE["id"], "user": ALICE,
              "text": None, "cw": None, "files": [], "reactions": {}, "renoteId": "n4o", "renote": sample}
    grapes = note("n5", ME, "庭のぶどうが色づいてきた :budou: 今年は甘いかな", "2026-11-27T08:02:00.000Z",
                  {":budou@.:": 11, "👀": 3}, 0, 3,
                  [photo("f7", "grapes.jpg", 800, 612, "grapes on a vine against the sky"),
                   photo("f8", "grapes2.jpg", 800, 1066, "picked grapes on a stone table")])
    quote = note("n6", BOB, "いいな、来年分けて 🇯🇵 🍇", "2026-11-27T07:40:00.000Z", {"👍": 2},
                 renoteId="n5", renote=grapes)
    building = note("n7", MIKA, "帰りに寄った海辺の水族館。建物がレトロでいい", "2026-11-27T07:10:00.000Z",
                    {"🏛️": 4, ":iruka@.:": 2}, 1, 0, [photo("f9", "aquarium.jpg", 800, 600, "an old aquarium building by the sea")])
    plain = [note(f"p{i}", [ALICE, BOB, KEN][i % 3],
                  ["おはようございます ☀️", "Coffee first ☕", "雨が降ってきた ☔ 傘ない",
                   "Shipping a release today 🚀", "猫が膝から降りない 🐈‍⬛"][i % 5],
                  f"2026-11-27T0{5 - i // 3}:{50 - i * 4:02d}:00.000Z", {"👍": i + 1}, 0, 0)
             for i in range(9)]
    return [show, aquarium, spoiler, renote, grapes, quote, building] + plain


def local():
    return [note(f"l{i}", [ALICE, BOB, ME][i % 3], t, f"2026-11-27T09:{50 - i * 3:02d}:00.000Z", {"👍": i})
            for i, t in enumerate(["ローカルのみなさんこんにちは 👋", "カスタム絵文字 :todo: も絵で出る",
                                   "Local timeline, made-up notes only", "家族旅行の写真はまた今度 👨‍👩‍👧"])]


def notifications():
    return [
        {"id": "x1", "createdAt": "2026-11-27T09:45:00.000Z", "type": "reaction", "reaction": ":budou@.:",
         "user": ALICE, "note": timeline()[4]},
        {"id": "x2", "createdAt": "2026-11-27T09:30:00.000Z", "type": "reply", "user": BOB,
         "note": note("r1", BOB, "ジャムにしても良さそう", "2026-11-27T09:30:00.000Z")},
        {"id": "x3", "createdAt": "2026-11-27T09:00:00.000Z", "type": "follow", "user": MIKA},
        {"id": "x4", "createdAt": "2026-11-27T08:40:00.000Z", "type": "renote", "user": KEN,
         "note": {"id": "rn9", "renoteId": "n5", "renote": timeline()[4], "user": KEN}},
    ]


def profile(u):
    """users/show: the account with its counts and what it says."""
    about = {
        "mika": "水族館と海の生き物が好き :iruka: 週末はだいたい日本海側にいます",
        "alice": "Photos, coffee and fish. 写真とコーヒー",
        "bob": "食べたものを載せます 🍔",
        "river": "読書と庭いじり :budou:",
    }
    p = dict(u)
    p.update({"description": about.get(u["username"], ""), "followersCount": 1280 if u is MIKA else 210,
              "followingCount": 312, "notesCount": 4096 if u is MIKA else 830,
              "isFollowing": u is not ME, "isFollowed": u is MIKA})
    return p


def notes_of(uid):
    seen = []
    for n in timeline() + local():
        for m in (n, n.get("renote")):
            if m and m.get("user", {}).get("id") == uid and m.get("text") is not None and m not in seen:
                seen.append(m)
    return seen


def find(nid):
    for n in timeline() + local():
        if n["id"] == nid:
            return n
        if n.get("renote") and n["renote"]["id"] == nid:
            return n["renote"]
    return None


def children(nid):
    if nid == "n5":
        return [note("c1", BOB, "ジャムにしても良さそう", "2026-11-27T09:30:00.000Z", replies=1),
                note("c2", ALICE, "Grape season 🍇 so jealous", "2026-11-27T09:35:00.000Z")]
    if nid == "c1":
        return [note("c1a", ME, "いいね、半分はジャムにする 🙏", "2026-11-27T09:38:00.000Z")]
    if nid == "n1":
        return [note("c3", ALICE, "動画いいね :iruka:", "2026-11-27T09:44:00.000Z")]
    return []


USERS = {u["username"]: u for u in (ME, ALICE, BOB, MIKA, KEN)}


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
                kind = {".png": "image/png", ".mp4": "video/mp4"}.get(p.suffix, "image/jpeg")
                return self.send(200, p.read_bytes(), kind)
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
            return self.send(200, profile(ME))
        if path == "/api/users/show":
            u = USERS.get(body.get("username") or "") or next(
                (u for u in USERS.values() if u["id"] == body.get("userId")), None)
            return self.send(200, profile(u)) if u else self.send(400, {"error": {"code": "NO_SUCH_USER"}})
        if path == "/api/users/notes":
            return self.send(200, [] if until else notes_of(body.get("userId")))
        if path == "/api/emojis":
            return self.send(200, {"emojis": [{"name": e, "url": img(f"emoji-{e}.png"), "aliases": [],
                                               "category": "demo"} for e in EMOJIS]})
        if path == "/api/notifications/mark-all-as-read":
            return self.send(204, b"")
        if path in ("/api/chat/history", "/api/notes/reactions", "/api/notes/renotes"):
            return self.send(200, [])
        if path == "/api/meta":
            return self.send(200, {"name": "Demo", "maxNoteTextLength": 3000})
        sys.stderr.write(f"demo-server: no answer for {path}\n")
        self.send(404, {"error": {"code": "NOT_FOUND"}})


def main():
    global BASE
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    BASE = f"http://127.0.0.1:{server.server_address[1]}"
    Path(sys.argv[1]).write_text(str(server.server_address[1]))
    server.serve_forever()


if __name__ == "__main__":
    main()
