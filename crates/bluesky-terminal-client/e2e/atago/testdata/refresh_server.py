"""Hold both old-token reads at a barrier, then accept only the refreshed token."""

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Barrier, BrokenBarrierError, Lock
from urllib.parse import urlsplit


class Handler(BaseHTTPRequestHandler):
    old_reads = Barrier(2, timeout=10)
    log_lock = Lock()
    reads = {
        "/xrpc/app.bsky.feed.getTimeline": ("timeline", {"feed": []}),
        "/xrpc/app.bsky.notification.listNotifications": (
            "notifications", {"notifications": []}
        ),
    }

    def record(self, name):
        auth = self.headers.get("Authorization", "")
        with self.log_lock:
            with open(f"{name}.log", "a", encoding="utf-8", newline="\n") as out:
                out.write(auth + "\n")
        return auth

    def reply(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        route = self.reads.get(urlsplit(self.path).path)
        if route is None:
            self.reply(404, {})
            return
        name, body = route
        auth = self.record(name)
        if auth == "Bearer acc":
            try:
                self.old_reads.wait()
            except BrokenBarrierError:
                self.reply(500, {"error": "Both old-token reads must arrive before refresh"})
                return
            self.reply(400, {"error": "ExpiredToken", "message": "Token has expired"})
        elif auth == "Bearer acc-2":
            self.reply(200, body)
        else:
            self.reply(401, {"error": "Unexpected access token"})

    def do_POST(self):
        if self.path != "/xrpc/com.atproto.server.refreshSession":
            self.reply(404, {})
        elif self.record("refresh") != "Bearer ref":
            self.reply(401, {"error": "Refresh token already used"})
        else:
            self.reply(200, {
                "did": "did:plc:me", "handle": "me.test",
                "accessJwt": "acc-2", "refreshJwt": "ref-2",
            })

    def log_message(self, format, *args):
        pass


if __name__ == "__main__":
    with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
        url = f"http://127.0.0.1:{server.server_port}"
        # The account names the chosen port; atago 0.27.0 does not expand a ready.store value inside fixture content.
        account = Path("cfg/accounts/did_plc_me.json")
        account.parent.mkdir(parents=True, exist_ok=True)
        account.write_text(json.dumps({
            "service": url, "did": "did:plc:me", "handle": "me.test",
            "accessJwt": "acc", "refreshJwt": "ref",
        }), encoding="utf-8")
        # atago reads the ready file as soon as it exists, so it appears whole through a rename.
        Path("ready.tmp").write_text(url, encoding="utf-8")
        Path("ready.tmp").replace("ready.txt")
        server.serve_forever()
