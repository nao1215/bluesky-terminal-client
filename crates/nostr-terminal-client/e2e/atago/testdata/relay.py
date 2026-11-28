"""A Nostr relay stand-in for nstr's atago specs, on 127.0.0.1 only.

It speaks the part of RFC 6455 a relay needs (the opening handshake and
unfragmented text, ping and close frames) and the part of NIP-01 nstr uses:
REQ is answered with the stored events that match its filter and EOSE,
EVENT with OK, and CLOSE ends the subscription. Only the standard library is
used, so the specs need nothing but python3.

The events are signed when the relay starts, with BIP-340 over secp256k1
written out below, by throwaway keys derived from fixed labels: they belong
to no one, so the events, their IDs and their signatures are the same on
every run. An EVENT the relay receives is checked the same way (its ID and
signature) before it is accepted, and written to published.json.

In the current directory it writes cfg/config.json naming this relay (the
port is chosen at start, and atago does not expand a ready.store value inside
fixture content), cfg/key with alice's secret key (mode 0600), and last
ready.txt with this relay's address, through a rename so atago never reads
it half written. Every
message received is appended to requests.jsonl.

Usage: relay.py [--reject REASON] [--dead] [--no-contacts] [--no-key] [--many]
"""

import argparse
import base64
import hashlib
import json
import os
import http.server
import zlib
import socket
import socketserver
import struct
import threading
from pathlib import Path

# secp256k1 and BIP-340 (https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki).
P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (
    0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
    0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8,
)


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0:
        return None
    if a == b:
        lam = 3 * a[0] * a[0] * pow(2 * a[1], -1, P) % P
    else:
        lam = (b[1] - a[1]) * pow(b[0] - a[0], -1, P) % P
    x = (lam * lam - a[0] - b[0]) % P
    return (x, (lam * (a[0] - x) - a[1]) % P)


def mul(point, k):
    out = None
    while k:
        if k & 1:
            out = add(out, point)
        point = add(point, point)
        k >>= 1
    return out


def tagged(tag, data):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + data).digest()


def xonly(point):
    return point[0].to_bytes(32, "big")


def lift_x(x):
    if x >= P:
        return None
    y = pow((pow(x, 3, P) + 7) % P, (P + 1) // 4, P)
    if pow(y, 2, P) != (pow(x, 3, P) + 7) % P:
        return None
    return (x, y if y % 2 == 0 else P - y)


def sign(secret, msg):
    pub = mul(G, secret)
    d = secret if pub[1] % 2 == 0 else N - secret
    # The auxiliary randomness is fixed, so a signature is the same on every run.
    t = (d ^ int.from_bytes(tagged("BIP0340/aux", bytes(32)), "big")).to_bytes(32, "big")
    k0 = int.from_bytes(tagged("BIP0340/nonce", t + xonly(pub) + msg), "big") % N
    r = mul(G, k0)
    k = k0 if r[1] % 2 == 0 else N - k0
    e = int.from_bytes(tagged("BIP0340/challenge", xonly(r) + xonly(pub) + msg), "big") % N
    return xonly(r) + ((k + e * d) % N).to_bytes(32, "big")


def verify(pubkey, msg, sig):
    pub = lift_x(int.from_bytes(pubkey, "big"))
    r = int.from_bytes(sig[:32], "big")
    s = int.from_bytes(sig[32:], "big")
    if pub is None or r >= P or s >= N:
        return False
    e = int.from_bytes(tagged("BIP0340/challenge", sig[:32] + pubkey + msg), "big") % N
    point = add(mul(G, s), mul(pub, N - e))
    return point is not None and point[1] % 2 == 0 and point[0] == r


def secret_of(label):
    """A throwaway key for this suite only; it is no one's."""
    return int.from_bytes(hashlib.sha256(f"nstr-e2e {label}".encode()).digest(), "big") % N


def public_of(secret):
    return xonly(mul(G, secret)).hex()


def digest(event):
    canonical = [0, event["pubkey"], event["created_at"], event["kind"], event["tags"], event["content"]]
    # The same serialization as NIP-01 asks: no spaces, UTF-8 rather than \u escapes.
    data = json.dumps(canonical, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(data).digest()


def make(secret, created_at, kind, tags, content):
    event = {"pubkey": public_of(secret), "created_at": created_at, "kind": kind, "tags": tags, "content": content}
    msg = digest(event)
    event["id"] = msg.hex()
    event["sig"] = sign(secret, msg).hex()
    return event


ALICE, BOB, CAROL = secret_of("alice"), secret_of("bob"), secret_of("carol")
T = 1767225600  # 2026-01-01T00:00:00Z


def store(args):
    alice, bob = public_of(ALICE), public_of(BOB)
    events = [
        make(ALICE, T, 0, [], json.dumps({"name": "old name"})),
        make(ALICE, T + 10, 0, [], json.dumps({
            "name": "alice", "display_name": "Alice 🐈‍⬛", "about": "テスト用のアカウントです",
            "picture": "http://127.0.0.1:9/alice.png",
        }, ensure_ascii=False)),
    ]
    if not args.no_contacts:
        events.append(make(ALICE, T + 20, 3, [["p", bob], ["p", "not-a-key"]], ""))
    first = make(ALICE, T + 100, 1, [], "はじめての投稿 🐈‍⬛")
    events += [
        first,
        make(BOB, T + 200, 1, [], "bob の一つ目"),
        make(BOB, T + 300, 1, [["content-warning", "ネタバレ"]], "bob の二つ目 👨‍👩‍👧"),
        make(CAROL, T + 400, 1, [], "carol is not followed"),
        make(CAROL, T + 500, 1, [["p", alice]], "@alice こんにちは"),
        make(BOB, T + 600, 1, [["e", first["id"], "", "reply"], ["p", alice]], "返信です"),
    ]
    if args.many:
        # More than a pipe holds, as text and as JSON, so a reader that stops early closes it mid-write.
        events += [make(BOB, T + 1000 + i, 1, [], f"note {i} " + "猫" * 400) for i in range(120)]
    return events


def matches(f, event):
    return (
        event["kind"] in f.get("kinds", [])
        and (not f.get("ids") or event["id"] in f["ids"])
        and (not f.get("#e") or any(t[:1] == ["e"] and t[1] in f["#e"] for t in event["tags"] if len(t) > 1))
        and (not f.get("authors") or event["pubkey"] in f["authors"])
        and (not f.get("#p") or any(t[:1] == ["p"] and t[1] in f["#p"] for t in event["tags"] if len(t) > 1))
        and (not f.get("#t") or any(t[:1] == ["t"] and t[1] in f["#t"] for t in event["tags"] if len(t) > 1))
        and ("until" not in f or event["created_at"] <= f["until"])
        and (not f.get("search") or f["search"].casefold() in event["content"].casefold())
    )


class Relay(socketserver.BaseRequestHandler):
    lock = threading.Lock()

    def read_exact(self, n):
        data = b""
        while len(data) < n:
            chunk = self.request.recv(n - len(data))
            if not chunk:
                raise ConnectionError("closed")
            data += chunk
        return data

    def send_frame(self, opcode, payload):
        head = bytes([0x80 | opcode])
        if len(payload) < 126:
            head += bytes([len(payload)])
        elif len(payload) < 65536:
            head += bytes([126]) + struct.pack(">H", len(payload))
        else:
            head += bytes([127]) + struct.pack(">Q", len(payload))
        self.request.sendall(head + payload)

    def send(self, message):
        self.send_frame(1, json.dumps(message).encode())

    def handshake(self):
        data = b""
        while b"\r\n\r\n" not in data:
            chunk = self.request.recv(4096)
            if not chunk:
                raise ConnectionError("closed")
            data += chunk
        if b"accept: application/nostr+json" in data.lower():
            body = json.dumps(getattr(self.server, "information", {"supported_nips": [1,11]})).encode()
            self.request.sendall(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\n"
                + f"Content-Length: {len(body)}\r\nConnection: close\r\n\r\n".encode() + body
            )
            return False
        key = ""
        for line in data.decode("latin-1").split("\r\n")[1:]:
            name, _, value = line.partition(":")
            if name.strip().lower() == "sec-websocket-key":
                key = value.strip()
        accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.request.sendall(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            f"Sec-WebSocket-Accept: {accept}\r\n\r\n".encode()
        )

    def frames(self):
        while True:
            b0, b1 = self.read_exact(2)
            length = b1 & 0x7F
            if length == 126:
                length = struct.unpack(">H", self.read_exact(2))[0]
            elif length == 127:
                length = struct.unpack(">Q", self.read_exact(8))[0]
            mask = self.read_exact(4) if b1 & 0x80 else bytes(4)
            payload = bytes(b ^ mask[i % 4] for i, b in enumerate(self.read_exact(length)))
            yield b0 & 0x0F, payload

    def log(self, message):
        with self.lock:
            with open("requests.jsonl", "a", encoding="utf-8", newline="\n") as out:
                out.write(json.dumps(message, ensure_ascii=False) + "\n")

    def handle(self):
        try:
            if self.handshake() is False:
                return
            for opcode, payload in self.frames():
                if opcode == 8:
                    self.send_frame(8, payload[:2])
                    return
                if opcode == 9:
                    self.send_frame(10, payload)
                if opcode == 1:
                    self.on_message(json.loads(payload.decode()))
        except (ConnectionError, OSError):
            return

    def on_message(self, message):
        self.log(message)
        if message[0] == "REQ":
            sub, filters = message[1], message[2:]
            overlap = getattr(self.server, "mute_started", None)
            if overlap is not None:
                kinds = {kind for f in filters for kind in f.get("kinds", [])}
                if 10000 in kinds:
                    overlap.set()
                if 3 in kinds and not overlap.wait(4):
                    self.send(["CLOSED", sub, "mute request did not overlap contacts"])
                    return
            for f in filters:
                found = sorted((e for e in self.server.events if matches(f, e)), key=lambda e: -e["created_at"])
                for event in found[: f.get("limit", len(found))]:
                    self.send(["EVENT", sub, event])
            if self.server.notice:
                self.send(["NOTICE", "fixture relay warning"])
            self.send(["EOSE", sub])
        elif message[0] == "EVENT":
            event = message[1]
            msg = digest(event)
            if msg.hex() != event["id"] or not verify(bytes.fromhex(event["pubkey"]), msg, bytes.fromhex(event["sig"])):
                self.send(["OK", event["id"], False, "invalid: bad event id or signature"])
                return
            if self.server.reject:
                self.send(["OK", event["id"], False, self.server.reject])
                return
            with self.lock:
                Path("published.tmp").write_text(json.dumps(event, ensure_ascii=False), encoding="utf-8")
                Path("published.tmp").replace("published.json")
                self.server.events.append(event)
            self.send(["OK", event["id"], True, ""])



class LoopbackHTTPServer(http.server.ThreadingHTTPServer):
    def server_bind(self):
        # HTTPServer.server_bind calls getfqdn, which can stall on macOS's
        # resolver before ready.txt is written. These fixtures need no DNS.
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address


class Pictures(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
        data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 32, 32, 8, 2, 0, 0, 0))
        data += chunk(b"IDAT", zlib.compress((b"\0" + b"\xff\x40\x20" * 32) * 32)) + chunk(b"IEND", b"")
        with Relay.lock:
            with open("images.jsonl", "a", encoding="utf-8") as out:
                out.write(json.dumps(self.path) + "\n")
        self.send_response(200)
        self.send_header("Content-Type", "image/png")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--reject", help="answer every EVENT with OK false and this reason")
    ap.add_argument("--dead", action="store_true", help="also name a relay address nothing listens on")
    ap.add_argument("--no-contacts", action="store_true", help="store no kind-3 contact list")
    ap.add_argument("--no-key", action="store_true", help="leave keyFile out of the config")
    ap.add_argument("--many", action="store_true", help="store 120 more long notes by bob")
    ap.add_argument("--notice", action="store_true")
    ap.add_argument("--pictures", action="store_true")
    ap.add_argument("--parity", action="store_true")
    ap.add_argument("--overlap-mute", action="store_true", help="wait for concurrent mute lookup before contacts; mute bob")
    ap.add_argument("--pages", action="store_true", help="short notes with a timestamp boundary larger than one page")
    args = ap.parse_args()

    socketserver.ThreadingTCPServer.allow_reuse_address = False
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Relay)
    server.daemon_threads = True
    server.events = store(args)
    server.notice = args.notice
    if args.overlap_mute:
        server.mute_started = threading.Event()
        server.events.append(make(ALICE, T + 2, 10000, [["p", public_of(BOB)]], ""))
    if args.pages:
        server.events += [make(BOB, T + 1000, 1, [], f"boundary {i:02}") for i in range(25)]
        server.events += [make(BOB, T + 900, 1, [], "older page note")]
    if args.pictures:
        media = LoopbackHTTPServer(("127.0.0.1", 0), Pictures)
        threading.Thread(target=media.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{media.server_address[1]}"
        server.events.append(make(BOB, T + 700, 1, [["imeta", f"url {origin}/imeta", "m image/png"]], f"local picture {origin}/picture.png"))
    Path("seed.json").write_text(json.dumps(server.events, ensure_ascii=False), encoding="utf-8")
    server.reject = args.reject
    relays = [f"ws://127.0.0.1:{server.server_address[1]}/"]
    if args.dead:
        # A port taken and given back at once: nothing listens on it, so connecting is refused.
        probe = socket.socket()
        probe.bind(("127.0.0.1", 0))
        relays.append(f"ws://127.0.0.1:{probe.getsockname()[1]}/")
        probe.close()

    cfg = Path("cfg")
    cfg.mkdir(exist_ok=True)
    config = {"publicKey": public_of(ALICE), "relays": relays, "timeoutSeconds": 5}
    if args.parity:
        server.information = {"supported_nips": [1,11,50]}
        import blossom
        media = LoopbackHTTPServer(("127.0.0.1", 0), blossom.Blossom)
        threading.Thread(target=media.serve_forever, daemon=True).start()
        config.update(searchRelays=[relays[0]], dmRelays=[relays[0]], blossomServers=[f"http://127.0.0.1:{media.server_port}"])
        server.events += [make(k, T + 1, 10050, [["relay", relays[0]]], "") for k in (ALICE, BOB)]
        Path("seed.json").write_text(json.dumps(server.events, ensure_ascii=False), encoding="utf-8")
        Path("bob.txt").write_text(public_of(BOB))
        Path("alice.txt").write_text(public_of(ALICE))
    if not args.no_key:
        config["keyFile"] = "key"
        key = cfg / "key"
        key.write_text(f"{ALICE:064x}\n", encoding="utf-8")
        os.chmod(key, 0o600)
    (cfg / "config.json").write_text(json.dumps(config), encoding="utf-8")
    Path("ready.tmp").write_text(relays[0], encoding="utf-8")
    Path("ready.tmp").replace("ready.txt")
    server.serve_forever()


if __name__ == "__main__":
    main()
