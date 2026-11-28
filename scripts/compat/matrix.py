#!/usr/bin/env python3
"""Select optional compatibility jobs from changed paths without running PR code."""
import json
import os
import subprocess

SERVERS = {"mstdn": ("gotosocial", "mastodon"), "msky": ("misskey", "sharkey"), "nstr": ("strfry", "nostr-rs-relay")}
CRATES = {"mstdn": "mastodon", "msky": "misskey", "nstr": "nostr"}


def select(event, paths=()):
    jobs = []
    for client, servers in SERVERS.items():
        if event == "pull_request":
            shared = ("crates/terminal-kit/", "scripts/compat/", ".github/workflows/compat.yml", ".cargo/")
            if not any(p in {"Cargo.toml", "Cargo.lock"} or p.startswith(shared + (f"crates/{CRATES[client]}-terminal-client/",)) for p in paths):
                continue
            servers = tuple(s for s in servers if s in {"gotosocial", "strfry", "nostr-rs-relay"})
        for server in servers:
            for group in (("usable",) if event == "pull_request" else ("core", "heavy")):
                jobs.append({"client": client, "server": server, "group": group})
    return jobs


def main():
    event = os.environ["GITHUB_EVENT_NAME"]
    paths = []
    if event == "pull_request":
        base, head = os.environ["BASE_SHA"], os.environ["HEAD_SHA"]
        paths = subprocess.check_output(["git", "diff", "--name-only", "--no-renames", "-z", base + "..." + head], text=True).split("\0")
    jobs = select(event, paths)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write("matrix=" + json.dumps({"include": jobs}) + "\n")
        output.write("has_jobs=" + str(bool(jobs)).lower() + "\n")


if __name__ == "__main__":
    main()
