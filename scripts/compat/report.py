#!/usr/bin/env python3
"""Print measured tier counts and speed scenarios from a retained atago report.

This summarizes evidence only; it cannot turn unimplemented catalog checks into
passes or infer feature support from a scenario name.
"""
import json
from pathlib import Path
import sys


def main():
    directory = Path(sys.argv[1])
    seed = json.loads((directory / 'seed.json').read_text())
    report = json.loads((directory / 'atago.json').read_text())
    if report.get('load_failures') or not report.get('suites'):
        raise SystemExit('compat: the specs did not load; inspect atago.json before reporting coverage')
    print(f"{seed['system']} {seed['instance']['version']}")
    print()
    print('| Tier | Passed | Failed/error | Skipped |')
    print('| --- | --- | --- | --- |')
    scenarios = [scenario for suite in report['suites'] for scenario in suite['scenarios']]
    for tier in ('usable', 'heavy', 'speed', 'behavior'):
        selected = [s for s in scenarios if s['name'].startswith(tier + ':')]
        passed = sum(s['status'] == 'passed' for s in selected)
        skipped = sum(s['status'] == 'skipped' for s in selected)
        print(f'| {tier} | {passed} | {len(selected) - passed - skipped} | {skipped} |')
    print()
    print('| Speed scenario | Scenario duration (ms) | Result |')
    print('| --- | --- | --- |')
    for scenario in scenarios:
        if scenario['name'].startswith('speed:'):
            print(f"| {scenario['name']} | {scenario['duration_ms']} | {scenario['status']} |")


if __name__ == '__main__':
    main()
