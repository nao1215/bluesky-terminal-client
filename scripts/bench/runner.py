#!/usr/bin/env python3
"""Shared process lifetime and first-paint probe for the mstdn/nstr suites."""

import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

MARK = "FIRSTPOSTMARK"


def stop(work):
    pidfile = work / "server.pid"
    if pidfile.exists():
        try:
            os.killpg(int(pidfile.read_text()), signal.SIGTERM)
        except ProcessLookupError:
            pass
        pidfile.unlink()


def start(work, mock, count=100, delay=0):
    work.mkdir(parents=True, exist_ok=True)
    (work / "ready").unlink(missing_ok=True)
    with (work / "server.log").open("w") as log:
        proc = subprocess.Popen([sys.executable, str(mock), str(work), str(count), str(delay)],
                                cwd=work, stdout=log, stderr=log, start_new_session=True)
    (work / "server.pid").write_text(str(proc.pid))
    try:
        deadline = time.monotonic() + 30
        while not (work / "ready").exists():
            if proc.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError((work / "server.log").read_text()[-4000:] or "stand-in did not start in 30 s")
            time.sleep(0.01)
    except BaseException:
        stop(work)
        raise


def first_post(mock, binary, terminal_ms, server_ms):
    client = "MSTDN" if "mastodon" in str(mock) else "NSTR"
    with tempfile.TemporaryDirectory(prefix="stc-bench-") as directory:
        work = Path(directory)
        try:
            start(work, mock, 1, server_ms)
            pid, fd = pty.fork()
            if pid == 0:
                fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
                os.environ.pop("MSTDN_HOST", None)
                os.environ.pop("MSTDN_TOKEN", None)
                os.environ.update({f"{client}_CONFIG_DIR": str(work / "cfg"),
                                   f"{client}_CACHE_DIR": "off", "TERM": "xterm-256color",
                                   "LC_ALL": "C.UTF-8", "TZ": "UTC"})
                os.execv(binary, [binary])
            out = b""
            asked = None
            answered = shown = exited = False
            deadline = time.monotonic() + 10
            try:
                while time.monotonic() < deadline:
                    ready, _, _ = select.select([fd], [], [], 0.002)
                    if ready:
                        try:
                            chunk = os.read(fd, 65536)
                        except OSError:
                            break
                        if not chunk:
                            break
                        out += chunk
                    now = time.monotonic()
                    if asked is None and b"\x1b[5n" in out:
                        asked = now
                    if not answered and asked is not None and now - asked >= terminal_ms / 1000:
                        os.write(fd, b"\x1b[?62;4c\x1b[0n")
                        answered = True
                    if not shown and MARK.encode() in out:
                        shown = True
                        os.write(fd, b"q")
                waited, status = os.waitpid(pid, os.WNOHANG)
                if not waited:
                    # PTY EOF can precede waitpid readiness by a scheduler tick.
                    for _ in range(100):
                        time.sleep(0.01)
                        waited, status = os.waitpid(pid, os.WNOHANG)
                        if waited:
                            break
                exited = bool(waited)
                if not shown or not exited or os.waitstatus_to_exitcode(status) != 0:
                    raise RuntimeError(f"first paint or clean exit missing: {out[-4000:]!r}")
            finally:
                if not exited:
                    os.kill(pid, signal.SIGKILL)
                    os.waitpid(pid, 0)
                os.close(fd)
        finally:
            stop(work)


def main():
    action, mock, *args = sys.argv[1:]
    mock = Path(mock).resolve()
    if action == "start":
        start(Path(args[0]).resolve(), mock)
    elif action == "stop":
        stop(Path(args[0]).resolve())
    elif action == "first-post":
        first_post(mock, str(Path(args[0]).resolve()), int(args[1]), int(args[2]))
    else:
        raise ValueError(action)


if __name__ == "__main__":
    main()
