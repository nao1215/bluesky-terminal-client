#!/usr/bin/env python3
"""Lints the crates' source may not switch off.

clippy -D warnings fails on dead code, an unused import, a `pub` no other
crate reaches (unreachable_pub) and, in terminal-kit, an undocumented item
(missing_docs). An `allow` or `expect` of one of them in crates/*/src would
let it back in without a word, so it fails here. Code nothing uses is
removed; code only tests use goes under #[cfg(test)]. The fuzz targets
include source files by path and use a part of each, so fuzz/ is left out.

Usage: source.py   (exit status 1 with each place on stderr)
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SILENCED = re.compile(r"#!?\[\s*(?:allow|expect)\s*\(([^\]]*)\)\s*\]")
GUARDED = {"dead_code", "unused", "unused_imports", "unused_variables", "unused_mut", "unreachable_pub", "missing_docs", "unused_crate_dependencies"}


def main():
    errors = []
    for src in sorted(ROOT.glob("crates/*/src/**/*.rs")):
        for n, line in enumerate(src.read_text(encoding="utf-8").splitlines(), 1):
            for m in SILENCED.finditer(line):
                names = {x.strip() for x in re.split(r"[,\s]+", re.sub(r"reason\s*=\s*\"[^\"]*\"", "", m.group(1))) if x.strip()}
                hit = sorted(names & GUARDED)
                if hit:
                    errors.append(f"{src.relative_to(ROOT).as_posix()}:{n}: {', '.join(hit)} is not switched off; remove what nothing uses, or keep test-only code under #[cfg(test)]")
    for e in errors:
        print(f"error: source: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
