#!/usr/bin/env python3
"""Serve generated test media on loopback without redirects or external fetches."""

import functools
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys


if __name__ == "__main__":
    root, ready = sys.argv[1:]
    handler = functools.partial(SimpleHTTPRequestHandler, directory=root)
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    Path(ready).write_text(f"http://127.0.0.1:{server.server_port}", encoding="utf-8")
    server.serve_forever()
