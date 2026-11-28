#!/usr/bin/env python3
"""Benchmark data served by the E2E suite's loopback WebSocket relay."""

import importlib.util
import json
import os
from pathlib import Path
import socketserver
import sys
import time

source = Path(__file__).resolve().parent / "../e2e/atago/testdata/relay.py"
spec = importlib.util.spec_from_file_location("relay_fixture", source)
relay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relay)


class Relay(relay.Relay):
    def log(self, message):
        pass  # File logging is an E2E assertion, not relay work to measure.

    def on_message(self, message):
        time.sleep(self.server.delay)
        super().on_message(message)


def main():
    work = Path(sys.argv[1])
    count, delay = map(int, sys.argv[2:])
    os.chdir(work)
    alice, bob = relay.public_of(relay.ALICE), relay.public_of(relay.BOB)
    events = [relay.make(relay.ALICE, relay.T, 3, [["p", bob]], ""),
              relay.make(relay.ALICE, relay.T, 0, [], json.dumps({"name": "bench"}))]
    texts = ["今日は山に登りました 🏔️ https://example.invalid/trip", "Rust and relays 👨‍👩‍👧", "駅前のパン屋 🥐 été 1️⃣ ❤️", "Reading list 📚🇯🇵"]
    events += [relay.make(relay.BOB, relay.T + i + 1, 1, [["p", alice]],
                          "FIRSTPOSTMARK" if count == 1 else texts[i % len(texts)] * (1 + i % 3)) for i in range(count)]
    with socketserver.ThreadingTCPServer(("127.0.0.1", 0), Relay) as server:
        server.daemon_threads = True
        server.events, server.notice, server.reject = events, False, None
        server.delay = delay / 1000
        cfg = work / "cfg"
        cfg.mkdir(exist_ok=True)
        (cfg / "config.json").write_text(json.dumps({"publicKey": alice, "relays": [f"ws://127.0.0.1:{server.server_address[1]}/"], "timeoutSeconds": 5}))
        (work / "ready").touch()
        server.serve_forever()


if __name__ == "__main__":
    main()
