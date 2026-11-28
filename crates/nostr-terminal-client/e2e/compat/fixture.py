#!/usr/bin/env python3
"""Seed real loopback relays with generated keys, using only the standard library."""

import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import socket
import struct
import sys
import time
import urllib.request
from urllib.parse import urlsplit

SOURCE = Path(__file__).resolve().parents[1] / "atago/testdata/relay.py"
spec = importlib.util.spec_from_file_location("standin", SOURCE)
standin = importlib.util.module_from_spec(spec)
spec.loader.exec_module(standin)


def address(value):
    parsed = urlsplit(value)
    if (parsed.scheme != "ws" or parsed.hostname != "127.0.0.1" or not parsed.port
            or parsed.username or parsed.password or parsed.fragment):
        raise ValueError("compat relays must be ws://127.0.0.1:PORT")
    return parsed


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError("relay information must not redirect")


def wait(relay):
    address(relay)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    deadline = time.monotonic() + 90
    while True:
        try:
            request = urllib.request.Request("http" + relay[2:], headers={"Accept": "application/nostr+json"})
            with opener.open(request, timeout=2) as response:
                assert response.headers.get_content_type() in {"application/nostr+json", "application/json"}
                info = json.loads(response.read(65537))
                assert isinstance(info, dict) and 1 in info["supported_nips"]
                print(json.dumps(info))
                return
        except (OSError, ValueError, AssertionError, KeyError) as error:
            if time.monotonic() >= deadline:
                raise RuntimeError(f"NIP-11 not ready: {relay}") from error
            time.sleep(0.25)


def exact(stream, count):
    result = b""
    while len(result) < count:
        chunk = stream.read(count - len(result))
        if not chunk:
            raise ConnectionError("relay closed")
        result += chunk
    return result


def frame(sock, opcode, data):
    mask = secrets.token_bytes(4)
    length = len(data)
    header = bytes([0x80 | opcode])
    if length < 126:
        header += bytes([0x80 | length])
    elif length < 65536:
        header += b"\xfe" + struct.pack(">H", length)
    else:
        header += b"\xff" + struct.pack(">Q", length)
    sock.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))


def seed(relay, events):
    parsed = address(relay)
    with socket.create_connection((parsed.hostname, parsed.port), timeout=10) as sock:
        with sock.makefile("rb") as stream:
            key = base64.b64encode(secrets.token_bytes(16)).decode()
            path = parsed.path or "/"
            sock.sendall((f"GET {path} HTTP/1.1\r\nHost: {parsed.netloc}\r\nUpgrade: websocket\r\n"
                          f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
            assert stream.readline().split()[1] == b"101"
            headers = {}
            while (line := stream.readline()) != b"\r\n":
                if not line:
                    raise ConnectionError("incomplete upgrade")
                name, value = line.decode().split(":", 1)
                headers[name.lower()] = value.strip()
            expected = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            assert headers["sec-websocket-accept"] == expected
            for event in events:
                frame(sock, 1, json.dumps(["EVENT", event], ensure_ascii=False).encode())
                deadline = time.monotonic() + 10
                while True:
                    sock.settimeout(max(0.01, deadline - time.monotonic()))
                    if time.monotonic() >= deadline:
                        raise TimeoutError("seed OK not received")
                    first, second = exact(stream, 2)
                    assert first & 0x80 and not second & 0x80, "unexpected fragmented or masked server frame"
                    length = second & 127
                    if length == 126:
                        length = struct.unpack(">H", exact(stream, 2))[0]
                    elif length == 127:
                        length = struct.unpack(">Q", exact(stream, 8))[0]
                    assert length <= 2 * 1024 * 1024
                    payload = exact(stream, length)
                    opcode = first & 15
                    if opcode == 9:
                        frame(sock, 10, payload)
                        continue
                    if opcode == 8:
                        raise ConnectionError("relay closed before seed OK")
                    if opcode != 1:
                        continue
                    message = json.loads(payload)
                    if message[0] == "OK" and message[1] == event["id"]:
                        assert message[2] or message[3].startswith("duplicate:"), message
                        break
            frame(sock, 8, struct.pack(">H", 1000))


def prepare(directory, relays):
    from catalog import events as catalog_events
    for relay in relays:
        address(relay)
    directory = Path(directory)
    directory.mkdir(mode=0o700, exist_ok=True)
    seed_file = directory / "seed.json"
    if seed_file.exists():
        events = json.loads(seed_file.read_text())
        counts = json.loads((directory / "counts.json").read_text())
    else:
        keys = [secrets.randbelow(standin.N - 1) + 1 for _ in range(3)]
        media = os.environ["NSTR_COMPAT_MEDIA"]
        parsed = urlsplit(media)
        if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or not parsed.port:
            raise ValueError("test media must be served on 127.0.0.1")
        events, counts = catalog_events(standin, keys, int(time.time()), media)
        with open(directory / "key", "x", opener=lambda path, flags: os.open(path, flags, 0o600)) as out:
            out.write(f"{keys[0]:064x}\n")
        for i, name in enumerate(["", "pictures", "videos"]):
            target = directory / name
            target.mkdir(mode=0o700, exist_ok=True)
            config = {"publicKey": standin.public_of(keys[i]), "relays": relays, "timeoutSeconds": 20}
            if i == 0:
                config["keyFile"] = "key"
            (target / "config.json").write_text(json.dumps(config), encoding="utf-8")
        seed_file.write_text(json.dumps(events, ensure_ascii=False), encoding="utf-8")
        (directory / "counts.json").write_text(json.dumps(counts), encoding="utf-8")
    print("seed counts: " + json.dumps(counts, sort_keys=True), flush=True)
    for relay in relays:
        seed(relay, events)


if __name__ == "__main__":
    if sys.argv[1] == "wait":
        wait(sys.argv[2])
    elif sys.argv[1] == "prepare":
        prepare(sys.argv[2], json.loads(os.environ["NSTR_COMPAT_RELAYS"]))
    else:
        raise SystemExit("expected wait or prepare")
