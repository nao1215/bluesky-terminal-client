#!/usr/bin/env python3
"""Print atago's own verdicts and scenario durations without rechecking behavior."""
import json
import sys

report = json.load(open(sys.argv[1], encoding="utf-8"))
for suite in report["suites"]:
    for scenario in suite["scenarios"]:
        print(f'{sys.argv[2]} | {scenario["name"]} | {scenario["status"]} | {scenario["duration_ms"]} ms')
    for failure in suite["failures"]:
        print(json.dumps(failure, ensure_ascii=False))
for failure in report.get("load_failures", []):
    print(json.dumps(failure))
