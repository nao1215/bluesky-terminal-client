#!/usr/bin/env python3
"""Local Nostr relay and media fixture. Usage: demo-server.py PORTFILE.

Reuses the E2E relay's WebSocket framing, filter matching and BIP-340 signing.
All events use new fictional identities. No signing key is given to the client.
"""

import importlib.util
import json
import socketserver
import sys
import threading
from http.server import BaseHTTPRequestHandler
from pathlib import Path
from urllib.parse import urlsplit

sys.dont_write_bytecode = True
DOC = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("fixture_relay", DOC.parent / "e2e/atago/testdata/relay.py")
relay = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(relay)


def npub(public):
    # NIP-19 uses Bech32 with an npub HRP and the 32 public-key bytes.
    alphabet = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
    bits = "".join(f"{byte:08b}" for byte in bytes.fromhex(public))
    bits += "0" * ((-len(bits)) % 5)
    words = [int(bits[i:i + 5], 2) for i in range(0, len(bits), 5)]
    values = [ord(c) >> 5 for c in "npub"] + [0] + [ord(c) & 31 for c in "npub"]
    checksum = 1
    for value in values + words + [0] * 6:
        top = checksum >> 25
        checksum = ((checksum & 0x1ffffff) << 5) ^ value
        for i, generator in enumerate((0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3)):
            if (top >> i) & 1:
                checksum ^= generator
    checksum ^= 1
    return "npub1" + "".join(alphabet[v] for v in words + [(checksum >> (5 * (5 - i))) & 31 for i in range(6)])


class Media(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        name = urlsplit(self.path).path.removeprefix("/media/")
        if name not in ("scene.jpg", "scene.mp4"):
            self.send_error(404)
            return
        data = (DOC / "demo" / name).read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", "video/mp4" if name.endswith(".mp4") else "image/jpeg")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


class DemoRelay(relay.Relay):
    def log(self, message):
        # This fixture does not write logs in the caller's working directory.
        pass

    def on_message(self, message):
        if message[0] == "EVENT":
            self.send(["OK", message[1]["id"], False, "restricted: read-only demo"])
        else:
            super().on_message(message)


def events(origin, address):
    secrets = [relay.secret_of("readme-" + name) for name in ("tess", "yuna", "malik", "orin")]
    public = [relay.public_of(s) for s in secrets]
    stamp = 1795856400
    make = relay.make
    names = [("tess", "Tess", "tess@keys.example"), ("yuna", "Yuna", "yuna@atelier.example"),
             ("malik", "Malik", "malik@relay.example"), ("orin", "Orin", "orin@archive.example")]
    result = [make(secrets[i], stamp, 0, [], json.dumps({
        "name": name, "display_name": display, "nip05": nip05,
        "about": f"NIP-05: {nip05}\nIndependent animation and portable identities.\n{npub(public[i])}",
    })) for i, (name, display, nip05) in enumerate(names)]
    result += [make(secrets[0], stamp + 1, 3, [["p", p] for p in public[1:]], ""),
               make(secrets[0], stamp + 2, 10002, [["r", address]], "")]

    def note(i, minute, content, tags=None):
        return make(secrets[i], stamp + minute * 60, 1, tags or [], content)

    def attachment(filename, mime, alt):
        return ["imeta", f"url {origin}/media/{filename}", f"m {mime}", f"alt {alt}"]

    result += [
        note(1, 60, "An animated scene for the people who collect small animations.\nNIP-05: yuna@atelier.example\nThanks for the 21 and 100 sat zaps on the previous sketch.",
             [attachment("scene.jpg", "image/jpeg", "A frame from the author's succubus animation")]),
        note(1, 59, "The animation loop, shared under the same public key on every relay.\nAn independent artist can keep an audience when a relay closes.",
             [attachment("scene.mp4", "video/mp4", "Succubus Live animated scene")]),
        note(2, 58, "Relay notebook\nI keep the signing key offline and carry its npub between clients.\n" + npub(public[2]) +
             "\nA NIP-05 name is a readable pointer to that key. My profile lists malik@relay.example. "
             "The key remains my identity if I move the name.\n\nI publish to two relays and keep a local archive of signed events. "
             "If one relay closes, I can give another relay the same events without rewriting their IDs. "
             "Replies still refer to those event IDs. Readers can choose a different set of relays from mine.\n\n"
             "Small zaps are a way for readers to support an artist directly. I would rather fund a few frames "
             "of animation than ask someone to chase an engagement score. A 21 sat thank-you and a thoughtful "
             "reply both tell the artist that somebody watched.\n\nThis demo uses one local relay, fictional "
             "names and no wallet. The zap amounts here are story text, not payment receipts.\nEnd of relay notebook."),
        note(3, 57, "The recovery drill used a disposable key. Never paste a signing key into a public note.",
             [["content-warning", "Key recovery exercise"]]),
        note(0, 56, "Archive complete. The local relay kept each event ID and signature. #nostr #relays", [["t", "nostr"], ["t", "relays"]]),
    ]
    return result, public[0]


def main():
    portfile = Path(sys.argv[1]).resolve()
    media = relay.LoopbackHTTPServer(("127.0.0.1", 0), Media)
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), DemoRelay)
    server.daemon_threads = True
    address = f"ws://127.0.0.1:{server.server_address[1]}/"
    origin = f"http://127.0.0.1:{media.server_port}"
    server.events, public = events(origin, address)
    server.notice = False
    server.reject = "restricted: read-only demo"
    server.information = {"name": "Studio demo relay", "supported_nips": [1, 11, 50]}
    cfg = portfile.parent / "cfg"
    cfg.mkdir(parents=True, exist_ok=True)
    (cfg / "config.json").write_text(json.dumps({"publicKey": npub(public), "relays": [address],
                                                "searchRelays": [address], "timeoutSeconds": 5}))
    (cfg / "settings.json").write_text(json.dumps({"theme": "gruvbox", "language": "en"}))
    threading.Thread(target=media.serve_forever, daemon=True).start()
    ready = portfile.with_suffix(".tmp")
    ready.write_text(str(server.server_address[1]))
    ready.replace(portfile)
    server.serve_forever()


if __name__ == "__main__":
    main()
