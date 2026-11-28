#!/usr/bin/env python3
"""No co-author or session trailers in commit messages and pull requests.

A commit is its author's: no Co-Authored-By line (but the one a bot such
as dependabot writes for itself), and no Claude-Session line or
claude.ai/code link that a tool adds.

Usage: trailers.py [FILE ...]          read each file (a commit message, a PR body)
       trailers.py --git BASE..HEAD    read the message of every commit in the range
       trailers.py -                   read stdin
Exit status 1 when one is found, each on its own line on stderr.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Dependabot signs the commits it makes with a co-author line of its own
# account; a bot's line is its own doing and is let through.
BANNED = re.compile(r"^[ \t]*co-authored-by:(?![^\n]*\[bot\])|^[ \t]*claude-session:|claude\.ai/code", re.I | re.M)


def scan(name, text, errors):
    # Lines git drops from a message (the editor's comments) are not checked.
    body = "\n".join(line for line in text.splitlines() if not line.startswith("#"))
    for m in BANNED.finditer(body):
        line = body[: m.start()].count("\n") + 1
        errors.append(f"{name}:{line}: '{m.group(0).strip()}': commits and pull requests carry no co-author or session trailers")


def main():
    args = sys.argv[1:]
    errors = []
    if args[:1] == ["--git"] and len(args) == 2:
        out = subprocess.run(["git", "-C", str(ROOT), "log", "--format=%H%x00%B%x01", args[1]], capture_output=True, text=True, check=True)
        for entry in out.stdout.split("\x01"):
            if "\x00" in entry:
                sha, msg = entry.strip("\n").split("\x00", 1)
                scan(sha[:12], msg, errors)
    else:
        for name in args or ["-"]:
            text = sys.stdin.read() if name == "-" else open(name, encoding="utf-8").read()
            scan("stdin" if name == "-" else name, text, errors)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
