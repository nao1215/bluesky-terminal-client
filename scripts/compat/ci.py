#!/usr/bin/env python3
"""Run one loopback deployment and retain public report provenance, even on failure."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys

from matrix import SERVERS

ROOT = Path(__file__).resolve().parents[2]
FILTERS = {"core": "usable:,behavior:,speed:", "heavy": "heavy:", "usable": "usable:"}


def main():
    client, server, group = sys.argv[1:]
    if client not in SERVERS or server not in SERVERS[client] or group not in FILTERS:
        raise SystemExit("invalid compatibility job")
    directory = ROOT / "target/compat" / f"{client}-{server}-{group}"
    directory.mkdir(parents=True, exist_ok=True)
    # A failed retry must never inherit a previous run's successful report.
    for filename in ("run.json", "atago.json", "seed.json", "responses.json", server + ".json", server + "-server.json"):
        (directory / filename).unlink(missing_ok=True)
    env = dict(os.environ, COMPAT_GROUP=group)
    record = {"client": client, "server": server, "group": group,
              "started_at": datetime.now(timezone.utc).isoformat(),
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "atago": subprocess.check_output(["atago", "version"], text=True).strip()}
    report_name = "atago.json" if client == "mstdn" else server + ".json"
    record["report"] = report_name
    record_path = directory / "run.json"
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    if client == "mstdn":
        env.update(MSTDN_COMPAT_FILTER=FILTERS[group], MSTDN_COMPAT_REPORT_DIR=str(directory))
        command = [str(ROOT / f"scripts/compat/{server}.sh")]
    elif client == "msky":
        env["MSKY_REPORT_DIR"] = str(directory)
        command = [str(ROOT / f"scripts/compat/{server}.sh"), "--filter", FILTERS[group]]
    else:
        env.update(NSTR_COMPAT_MODE=server, NSTR_COMPAT_REPORT_DIR=str(directory))
        command = [str(ROOT / "scripts/compat/nostr-relays.sh"), "--filter", FILTERS[group]]
    result = subprocess.run(command, cwd=ROOT, env=env)
    record["exit_code"] = result.returncode
    record["finished_at"] = datetime.now(timezone.utc).isoformat()
    if client == "mstdn" and (directory / "seed.json").exists():
        record["version"] = json.loads((directory / "seed.json").read_text())["instance"]["version"]
    elif (directory / f"{server}-server.json").exists():
        record["version"] = json.loads((directory / f"{server}-server.json").read_text()).get("version", "not reported")
    record_path.write_text(json.dumps(record, indent=2) + "\n")
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
