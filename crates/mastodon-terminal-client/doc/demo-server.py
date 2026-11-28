#!/usr/bin/env python3
"""Loopback Mastodon fixture. Usage: demo-server.py PORTFILE.

Writes an isolated login in PORTFILE's sibling cfg directory before readiness.
All people and statuses are fictional. Media comes from doc/demo/SOURCES.md.
"""

import json
import socketserver
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

MEDIA = Path(__file__).resolve().parent / "demo"


def account(uid, username, name, host):
    return {"id": uid, "username": username, "acct": f"{username}@{host}",
            "display_name": name, "note": "<p>Protocols, self-hosting and reproducible systems.</p>",
            "followers_count": 128, "following_count": 64, "statuses_count": 512}


ME = account("1", "sora", "Sora", "infra.example")
PEOPLE = [ME, account("2", "inez", "Inez", "protocols.example"),
          account("3", "dev", "Dev Shah", "ops.example"),
          account("4", "leonie", "Leonie", "federation.example")]


def timeline(base):
    def status(sid, person, content, **extra):
        value = {"id": str(sid), "account": person, "content": f"<p>{content}</p>",
                 "created_at": f"2026-11-28T09:{int(sid) % 60:02}:00Z",
                 "uri": f"{base}/users/{person['username']}/statuses/{sid}",
                 "url": f"{base}/@{person['username']}/{sid}", "visibility": "public",
                 "spoiler_text": "", "media_attachments": [], "favourites_count": 17,
                 "reblogs_count": 4, "replies_count": 0, "tags": []}
        value.update(extra)
        return value

    def media(kind, filename, alt):
        return {"id": filename, "type": kind, "url": f"{base}/media/{filename}",
                "preview_url": f"{base}/media/protocol.jpg", "description": alt}

    original = status(100, PEOPLE[3], "Our small instance now has a tested restore procedure. A backup only counts after a restore. #SelfHosting #Fediverse")
    return [
        status(106, PEOPLE[1], "Inspecting an ISO 8583 message in the terminal. Bitmap fields are easier to review side by side. #CLI #Protocols",
               media_attachments=[media("image", "protocol.jpg", "ISO 8583 message fields in iso8583tool")]),
        status(105, PEOPLE[1], "A short iso8583tool walkthrough: decode the message before debugging the transport. #ISO8583",
               media_attachments=[media("video", "protocol.mp4", "iso8583tool decodes a payment protocol message")]),
        status(104, PEOPLE[2], "The staging certificate expired during a queue replay. We restored delivery after rotating it; no production traffic was involved. #SRE",
               spoiler_text="Staging incident postmortem"),
        status(103, ME, "Which maintenance task should our instance document next? #SelfHosting",
               poll={"id": "8", "expired": False, "votes_count": 42, "multiple": False,
                     "options": [{"title": "Restore from backup", "votes_count": 24},
                                 {"title": "Rotate signing keys", "votes_count": 12},
                                 {"title": "Inspect delivery queues", "votes_count": 6}]}),
        status(102, PEOPLE[2], "", reblog=original),
        status(101, PEOPLE[3], "@inez@protocols.example the packet capture helped. Federation debugging starts with one delivery ID. #ActivityPub",
               tags=[{"name": "ActivityPub", "url": f"{base}/tags/ActivityPub"}]),
    ]


class Server(ThreadingHTTPServer):
    def server_bind(self):
        # Avoid HTTPServer's reverse DNS lookup, including at startup.
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send(self, code, body, mime="application/json"):
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        url = urlsplit(self.path)
        path, query = url.path, parse_qs(url.query)
        posts = timeline(self.server.base)
        if path.startswith("/media/"):
            name = path.removeprefix("/media/")
            if name in ("protocol.jpg", "protocol.mp4"):
                mime = "image/jpeg" if name.endswith(".jpg") else "video/mp4"
                return self.send(200, (MEDIA / name).read_bytes(), mime)
        if path == "/api/v1/accounts/verify_credentials":
            return self.send(200, ME)
        if path in ("/api/v1/timelines/home", "/api/v1/timelines/public") or path.startswith("/api/v1/timelines/tag/"):
            return self.send(200, [] if "max_id" in query else posts)
        if path == "/api/v1/markers":
            return self.send(200, {"notifications": {"last_read_id": "0", "version": 1}})
        if path in ("/api/v1/notifications", "/api/v1/lists", "/api/v1/followed_tags"):
            return self.send(200, [])
        if path == "/api/v1/notifications/unread_count":
            return self.send(200, {"count": 0})
        if path == "/api/v1/accounts/relationships":
            return self.send(200, [{"id": uid, "following": True} for uid in query.get("id[]", [])])
        if path.startswith("/api/v1/accounts/"):
            uid = path.split("/")[4]
            person = next((p for p in PEOPLE if p["id"] == uid), None)
            if person:
                return self.send(200, [p for p in posts if p["account"]["id"] == uid] if path.endswith("/statuses") else person)
        if path.startswith("/api/v1/statuses/"):
            sid = path.split("/")[4]
            post = next((p for p in posts if p["id"] == sid), None)
            if post:
                return self.send(200, {"ancestors": [], "descendants": []} if path.endswith("/context") else post)
        if path == "/api/v2/search":
            term = query.get("q", [""])[0].casefold().lstrip("#")
            return self.send(200, {"statuses": [p for p in posts if term in p["content"].casefold()],
                                   "accounts": [], "hashtags": []})
        print(f"Unhandled GET {self.path}", file=sys.stderr)
        self.send(404, {"error": "Not found in the demo"})


def main():
    portfile = Path(sys.argv[1]).resolve()
    server = Server(("127.0.0.1", 0), Handler)
    server.base = f"http://127.0.0.1:{server.server_port}"
    cfg = portfile.parent / "cfg"
    (cfg / "accounts").mkdir(parents=True, exist_ok=True)
    host = f"127.0.0.1:{server.server_port}"
    login = cfg / "accounts" / f"127.0.0.1_{server.server_port}.json"
    login.write_text(json.dumps({"service": server.base, "userId": "1", "username": "sora",
                                 "token": "demo-only", "scopes": ["read"]}))
    login.chmod(0o600)
    (cfg / "accounts.json").write_text(json.dumps({"current": host}))
    (cfg / "settings.json").write_text(json.dumps({"theme": "nord", "language": "en"}))
    ready = portfile.with_suffix(".tmp")
    ready.write_text(str(server.server_port))
    ready.replace(portfile)
    server.serve_forever()


if __name__ == "__main__":
    main()
