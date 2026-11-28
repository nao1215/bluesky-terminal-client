#!/usr/bin/env python3
"""Update the generated section of the compatibility record from retained CI reports."""
import argparse
from collections import Counter
import json
from pathlib import Path

START = "<!-- compat-reports:start -->"
END = "<!-- compat-reports:end -->"
TIERS = ("usable", "heavy", "speed", "behavior")


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ").replace("\r", " ")


def render(directory, expected=()):
    records = sorted(directory.rglob("run.json"))
    if not records and not expected:
        raise ValueError(f"no run.json records in {directory}")
    lines = ["## Reported runs", "", "These rows summarize the supplied atago JSON reports. Counts describe selected scenarios, not certification of the complete shared catalog. Missing or unreadable reports are recorded as unverified; historical observations elsewhere on this page are retained.", "", "| Client | Server | Reported version | UTC date | Selection | Runner exit | Tier outcomes |", "| --- | --- | --- | --- | --- | --- | --- |"]
    speeds, provenance, seen = [], [], set()
    for path in records:
        run = json.loads(path.read_text())
        seen.add((run["client"], run["server"], run["group"]))
        outcomes = []
        report = path.parent / run["report"]
        try:
            data = json.loads(report.read_text())
            scenarios = [scenario for suite in data["suites"] for scenario in suite["scenarios"]]
            if data.get("load_failures") or not scenarios:
                outcomes.append("unverified: load failures or no scenarios")
            if any(suite.get("failures") for suite in data["suites"]):
                outcomes.append("suite failures present")
            for tier in TIERS:
                selected = [s for s in scenarios if s["name"].startswith(tier + ":")]
                if selected:
                    counts = Counter(s["status"] for s in selected)
                    outcomes.append(tier + ": " + ", ".join(f"{count} {status}" for status, count in sorted(counts.items())))
                else:
                    outcomes.append(tier + ": not selected")
            for scenario in scenarios:
                if scenario["name"].startswith("speed:"):
                    speeds.append([run["client"], run["server"], run.get("version", "not reported"), run["started_at"][:10], scenario["name"], scenario.get("duration_ms", "not reported"), scenario["status"]])
        except (OSError, ValueError, KeyError, TypeError):
            outcomes = ["unverified: missing or invalid report"]
        row = [run["client"], run["server"], run.get("version", "not reported"), run["started_at"][:10], run["group"], run.get("exit_code", "not completed"), "; ".join(outcomes)]
        lines.append("| " + " | ".join(map(cell, row)) + " |")
        provenance.append([run["client"], run["server"], run["group"], run["revision"], run["atago"]])
    for job in expected:
        if (job["client"], job["server"], job["group"]) not in seen:
            row = [job["client"], job["server"], "not reported", "not reported", job["group"], "not reported", "unverified: no job artifact"]
            lines.append("| " + " | ".join(map(cell, row)) + " |")
    lines += ["", "Scenario durations include runner and assertion overhead; they do not isolate screen paint or HTTP latency.", "", "| Client | Server | Reported version | UTC date | Speed scenario | Duration (ms) | Result |", "| --- | --- | --- | --- | --- | --- | --- |"]
    lines += ["| " + " | ".join(map(cell, row)) + " |" for row in speeds]
    lines += ["", "| Client | Server | Selection | Revision | atago |", "| --- | --- | --- | --- | --- |"]
    lines += ["| " + " | ".join(map(cell, row)) + " |" for row in provenance]
    return "\n".join(lines) + "\n"


def update(document, section):
    if document.count(START) != 1 or document.count(END) != 1:
        raise ValueError("expected one compatibility report marker pair")
    before, rest = document.split(START)
    _, after = rest.split(END)
    return before + START + "\n\n" + section + "\n" + END + after


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", type=Path)
    parser.add_argument("--out", type=Path, default=Path("doc/compat.md"))
    parser.add_argument("--matrix", help="expected matrix JSON; missing jobs are recorded as unverified")
    args = parser.parse_args()
    expected = json.loads(args.matrix)["include"] if args.matrix else ()
    args.out.write_text(update(args.out.read_text(), render(args.reports, expected)))


if __name__ == "__main__":
    main()
