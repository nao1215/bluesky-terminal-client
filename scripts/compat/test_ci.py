#!/usr/bin/env python3
"""Check PR selection and failed/partial report handling without starting servers."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import ci

import matrix
import results


class Selection(unittest.TestCase):
    def test_nightly_and_manual_have_independent_heavy_jobs(self):
        for event in ("schedule", "workflow_dispatch"):
            jobs = matrix.select(event)
            self.assertEqual(len(jobs), 12)
            self.assertEqual(len({(j["client"], j["server"], j["group"]) for j in jobs}), 12)
            self.assertEqual({j["group"] for j in jobs}, {"core", "heavy"})

    def test_pr_paths_select_only_affected_fast_servers(self):
        cases = [(["crates/mastodon-terminal-client/src/main.rs"], {"gotosocial"}),
                 (["crates/nostr-terminal-client/e2e/compat/relays.atago.yaml"], {"strfry", "nostr-rs-relay"}),
                 (["crates/misskey-terminal-client/src/main.rs"], set()),
                 (["README.md"], set()),
                 (["Cargo.lock"], {"gotosocial", "strfry", "nostr-rs-relay"}),
                 (["crates/terminal-kit/src/lib.rs"], {"gotosocial", "strfry", "nostr-rs-relay"}),
                 (["scripts/compat/ci.py"], {"gotosocial", "strfry", "nostr-rs-relay"})]
        for paths, expected in cases:
            with self.subTest(paths=paths):
                jobs = matrix.select("pull_request", paths)
                self.assertEqual({j["server"] for j in jobs}, expected)
                self.assertTrue(all(j["group"] == "usable" for j in jobs))


class Retry(unittest.TestCase):
    def test_failed_retry_does_not_reuse_a_successful_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "target/compat/mstdn-gotosocial-core"
            output.mkdir(parents=True)
            (output / "atago.json").write_text('{"suites": []}')
            (output / "seed.json").write_text('{"instance": {"version": "stale"}}')
            with patch.object(ci, "ROOT", root), patch.object(ci.sys, "argv", ["ci.py", "mstdn", "gotosocial", "core"]), patch.object(ci.subprocess, "check_output", return_value="test\n"), patch.object(ci.subprocess, "run", return_value=ci.subprocess.CompletedProcess([], 1)):
                self.assertEqual(ci.main(), 1)
            self.assertFalse((output / "atago.json").exists())
            run = json.loads((output / "run.json").read_text())
            self.assertEqual(run["exit_code"], 1)
            self.assertNotIn("version", run)


class Reports(unittest.TestCase):
    def render(self, report=None, code=0):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = {"client": "msky", "server": "misskey", "version": "2025.12.0",
                   "group": "core", "started_at": "2026-11-28T12:00:00Z", "exit_code": code,
                   "revision": "abc123", "atago": "atago v0.28.0", "report": "atago.json"}
            (root / "run.json").write_text(json.dumps(run))
            if report is not None:
                (root / "atago.json").write_text(json.dumps(report))
            return results.render(root)

    def test_partial_failed_and_expected_failure_are_not_passes(self):
        report = {"suites": [{"scenarios": [
            {"name": "usable: read", "status": "passed"},
            {"name": "usable: renote", "status": "xfail"},
            {"name": "usable: missing", "status": "skipped"},
            {"name": "speed: timeline", "status": "failed", "duration_ms": 12345}], "failures": []}]}
        text = self.render(report, code=1)
        self.assertIn("1 passed, 1 skipped, 1 xfail", text)
        self.assertIn("heavy: not selected", text)
        self.assertIn("speed: 1 failed", text)
        self.assertIn("12345 | failed", text)
        self.assertIn("2025.12.0 | 2026-11-28 | core | 1", text)

    def test_missing_and_load_failure_reports_are_unverified(self):
        self.assertIn("unverified: missing or invalid report", self.render(code=1))
        self.assertIn("unverified: load failures", self.render({"load_failures": ["bad spec"], "suites": []}, code=1))

    def test_absent_job_artifacts_cannot_disappear_from_the_record(self):
        with tempfile.TemporaryDirectory() as temporary:
            text = results.render(Path(temporary), matrix.select("schedule"))
        self.assertEqual(text.count("unverified: no job artifact"), 12)

    def test_regeneration_preserves_history_and_is_idempotent(self):
        document = "Historical results\n" + results.START + "\nold\n" + results.END + "\nLimits\n"
        updated = results.update(document, "new results\n")
        self.assertTrue(updated.startswith("Historical results\n"))
        self.assertTrue(updated.endswith("\nLimits\n"))
        self.assertEqual(results.update(updated, "new results\n"), updated)


if __name__ == "__main__":
    unittest.main()
