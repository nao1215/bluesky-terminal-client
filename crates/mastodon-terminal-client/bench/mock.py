#!/usr/bin/env python3
"""Run atago's HTTP mock with the E2E suite's status fixture on loopback."""

import json
import os
from pathlib import Path
import shlex
import sys
import time
from urllib.parse import urlsplit


def hold(work, origin):
    port = urlsplit(origin).port
    cfg = work / "cfg"
    (cfg / "accounts").mkdir(parents=True, exist_ok=True)
    (cfg / "accounts" / f"127.0.0.1_{port}.json").write_text(json.dumps({
        "service": origin, "userId": "7", "username": "bench", "token": "bench-token",
        "scopes": ["read"],
    }))
    (cfg / "accounts.json").write_text(json.dumps({"current": f"127.0.0.1:{port}"}))
    (work / "ready").touch()
    while True:
        time.sleep(1)


def main():
    work = Path(sys.argv[1])
    if sys.argv[2] == "hold":
        hold(work, sys.argv[3])
        return
    count, delay = map(int, sys.argv[2:])
    here = Path(__file__).resolve().parent
    statuses = json.loads((here / "../e2e/atago/testdata/public-100.json").read_text())[:count]
    if count == 1:
        statuses[0]["content"] = "<p>FIRSTPOSTMARK</p>"
    root = dict(statuses[0], id="200")
    routes = []
    for path, body in [
        ("/api/v1/timelines/home", statuses),
        ("/api/v1/timelines/public", statuses),
        ("/api/v1/notifications", [{"id": f"n{i}", "type": "mention", "created_at": s["created_at"],
                                     "account": s["account"], "status": s} for i, s in enumerate(statuses)]),
        ("/api/v1/statuses/200", root),
        ("/api/v1/statuses/200/context", {"ancestors": [], "descendants": statuses}),
        ("/api/v2/search", {"statuses": statuses, "accounts": [], "hashtags": []}),
        ("/api/v2/instance", {"configuration": {"statuses": {"max_characters": 500}}}),
        ("/api/v1/custom_emojis", []),
    ]:
        routes.append({"method": "GET", "path": path, "json": body, "delay": f"{delay}ms"})
    # The mock's own scenario owns its sockets; its child publishes the config
    # to himorime's work directory and keeps the scenario alive until cleanup.
    command = shlex.join([sys.executable, str(Path(__file__).resolve()), str(work), "hold"]) + ' "${api.url}"'
    spec = {"version": "1", "suite": {"name": "mstdn benchmark stand-in"}, "scenarios": [{
        "name": "serve", "mock_servers": [{"name": "api", "routes": routes}],
        "steps": [{"run": {"command": command, "timeout": "30m"}}],
    }]}
    path = work / "mock.atago.yaml"
    path.write_text(json.dumps(spec, ensure_ascii=False, indent=2))
    os.execvp("atago", ["atago", "run", "--parallel", "1", str(path)])


if __name__ == "__main__":
    main()
