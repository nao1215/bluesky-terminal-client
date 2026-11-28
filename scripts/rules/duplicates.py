#!/usr/bin/env python3
"""Code copied between or inside the crates' src/, found with jscpd.

.jscpd.json at the root says where to look and how big a copy counts: 10
lines and 100 tokens. Below that, the matches are the shape Rust gives
every match arm, builder chain and error conversion, not code anyone
copied. Tests are left out: a test spells out its case in full on purpose.

scripts/rules/jscpd-baseline.json holds the copies that were there when
the rule came in. Most are msky's code that mstdn and bsky took, kept apart
on purpose until the Mastodon client is done and the parts all three need
are drawn out at once. A copy the baseline does not list fails: take the
part both need into terminal-kit, or, when the copy is the right thing,
run with --update and say why in the pull request. A copy taken away
leaves its entry behind, and the run says to drop it (--update does).

The Mastodon media extraction removed the image pipeline and player copies.
The remaining cross-client HELP tables intentionally spell out matching keys
beside each client's translation marks; sharing them would put UI vocabulary
and translation policy into terminal-kit. The small video-state label match
stays with each view's fallback and caption layout. The input dispatch slice
stays in each event loop because its App, jobs, account switch and persistence
have different contracts. These matches are recorded in the baseline, without
changing the token or line thresholds.

The composer browser now lives in terminal-kit and is used by msky and
mstdn. Its remaining matches with bsky are relocated existing copies:
bsky's browser also chooses and creates download folders, and has its own
translation and folder-name editor. Moving those modes is outside this
Mastodon stage; expanding the shared API for them is deferred. The existing
bsky/msky viewer fit calculation also has a new fingerprint after moving
the adjacent byte-count formatter. None of these entries adds copied code.

The Nostr TUI extraction shares the theme inventory across all four clients
and locale parsing between mstdn and nstr. Removing mstdn's locale helpers
changed the fingerprint of its existing translation-table/process-state
match with msky; that unchanged code stays client-local because their
vocabulary and thread-local language selection belong to each application.
The existing bsky/msky legacy theme-name lookup and its adjacent tests also
have a new fingerprint after removing their local inventories; their old
name aliases stay in those clients to preserve existing settings behavior.
Their existing palette-readability tests now index the shared inventory
instead of local BLUESKY arrays; matches with terminal-kit are retained tests.
No Nostr TUI implementation is copied from another client.

The fourth Mastodon stage moved emoji cell substitution and drawing, dwell
tracking, and column geometry into terminal-kit. The changed HELP and terminal
startup fingerprints remain client-specific for the reasons above. Extending
Mastodon's multipart writer to avatar/header parts changed the adjacent existing
retry-notice fingerprint; that unchanged command-line network retry notice does
not belong in terminal-kit's drawing API. No new implementation copy is retained.

Needs npx (Node.js). Usage: duplicates.py [--update]
"""

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
JSCPD = "jscpd@5.4.3"
CONFIG = ROOT / ".jscpd.json"
BASELINE = ROOT / "scripts" / "rules" / "jscpd-baseline.json"


def jscpd(*args):
    npx = shutil.which("npx") or shutil.which("npx.cmd")
    if not npx:
        sys.exit("duplicates: npx (Node.js) is needed to run jscpd")
    return subprocess.run([npx, "--yes", JSCPD, "-c", str(CONFIG), *args], cwd=ROOT, capture_output=True, text=True, check=False)


def fingerprints(path):
    return set(json.loads(path.read_text(encoding="utf-8")).get("fingerprints", {}))


def where(f):
    path = Path(f["name"])
    rel = path.relative_to(ROOT).as_posix() if path.is_absolute() and path.is_relative_to(ROOT) else f["name"]
    return f"{rel}:{f['start']}-{f['end']}"


def main():
    if sys.argv[1:] == ["--update"]:
        out = jscpd("--baseline", str(BASELINE), "--update-baseline", "--silent")
        print(out.stdout.strip() or out.stderr.strip())
        return out.returncode
    errors, warnings = [], []
    with tempfile.TemporaryDirectory() as tmp:
        fresh = Path(tmp) / "baseline.json"
        shutil.copy(BASELINE, fresh)
        out = jscpd("--baseline", str(fresh), "--update-baseline", "--silent")
        if out.returncode != 0:
            sys.exit(f"duplicates: jscpd failed: {out.stderr.strip()}")
        gone = fingerprints(BASELINE) - fingerprints(fresh)
        if gone:
            warnings.append(f"duplicates: {len(gone)} copies in {BASELINE.relative_to(ROOT).as_posix()} are gone; run `python3 scripts/rules/duplicates.py --update` to drop them")
        out = jscpd("--baseline", str(BASELINE), "--absolute", "--reporters", "json", "--output", tmp, "--silent")
        if out.returncode != 0:
            sys.exit(f"duplicates: jscpd failed: {out.stderr.strip()}")
        report = json.loads((Path(tmp) / "jscpd-report.json").read_text(encoding="utf-8"))
    for d in report.get("duplicates", []):
        if d.get("isNew"):
            errors.append(
                f"duplicates: {where(d['firstFile'])} copies {where(d['secondFile'])} ({d['lines']} lines);"
                " share it (terminal-kit for what every client needs), or run `python3 scripts/rules/duplicates.py --update` and say why in the pull request"
            )
    for w in warnings:
        print(f"warning: {w}", file=sys.stderr)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
