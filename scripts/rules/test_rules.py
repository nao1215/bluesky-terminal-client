#!/usr/bin/env python3
"""Tests of the rule scripts: each catches what it is for and lets the rest through."""

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cli  # noqa: E402
import docs  # noqa: E402
import trailers  # noqa: E402


def docs_errors(text, name="README.md"):
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / name
        path.write_text(textwrap.dedent(text), encoding="utf-8")
        errors = []
        docs.check(path, errors)
        return errors


class Docs(unittest.TestCase):
    def test_bold_and_a_broken_paragraph_fail(self):
        errors = docs_errors("This is **loud** and\nwraps.\n")
        self.assertEqual(len(errors), 2)
        self.assertIn("bold", errors[0])
        self.assertIn("one line", errors[1])

    def test_lists_tables_code_and_headings_pass(self):
        text = """\
        # Title

        - an item
          that goes on
        | a | b |
        |---|---|

        ```sh
        echo **x**
        ```

        One paragraph on one line with `**` in code.
        """
        self.assertEqual(docs_errors(text), [])

    def test_a_readme_says_what_exists(self):
        self.assertEqual(len(docs_errors("Videos are coming soon.\n")), 1)
        self.assertEqual(len(docs_errors("It works the same as bsky.\n")), 1)
        self.assertEqual(docs_errors("Videos play in the terminal.\n"), [])

    def test_unreleased_may_say_what_is_on_its_way(self):
        text = "# Changelog\n\n## [Unreleased]\n\n- Planned: a thing\n"
        self.assertEqual(docs_errors(text, "CHANGELOG.md"), [])

    def test_the_root_readme_links_to_no_crate_readme(self):
        self.assertEqual(len(docs_errors("See [bsky](crates/bluesky-terminal-client/README.md).\n")), 1)
        self.assertEqual(len(docs_errors("See [it](https://github.com/nao1215/social-terminal-clients/blob/main/crates/x/README.md).\n")), 1)
        self.assertEqual(docs_errors("See [CONTRIBUTING.md](CONTRIBUTING.md).\n"), [])


class TestingLinks(unittest.TestCase):
    def test_root_readme_allows_only_testing_evidence(self):
        self.assertEqual(docs_errors("See [scenarios](doc/e2e/) and [results](doc/compat.md).\n"), [])
        self.assertEqual(len(docs_errors("See [other](doc/other.md).\n")), 1)


class Trailers(unittest.TestCase):
    def scan(self, text):
        errors = []
        trailers.scan("msg", text, errors)
        return errors

    def test_co_author_and_session_lines_fail(self):
        msg = "fix: x\n\nCo-Authored-By: Someone <a@b>\nClaude-Session: https://claude.ai/code/session_1\n"
        self.assertEqual(len(self.scan(msg)), 3)

    def test_a_bots_own_co_author_line_passes(self):
        msg = "build(deps): bump x\n\nCo-authored-by: dependabot[bot] <1+dependabot[bot]@users.noreply.github.com>\n"
        self.assertEqual(self.scan(msg), [])


class Help(unittest.TestCase):
    HELP = textwrap.dedent(
        """\
        Usage: x [OPTIONS] [COMMAND]

        Commands:
          timeline  The timeline [alias: tl]
          help      Print this message

        Options:
          -n, --limit <LIMIT>
                  How many

                  - home: possible value that is not a flag
              --json
                  Print JSON
          -h, --help
                  Print help
        """
    )

    def test_commands_aliases_and_flags_are_read_from_long_help(self):
        commands, flags = cli.parse_help(self.HELP)
        self.assertEqual(commands, {"timeline": ["tl"]})
        self.assertEqual(flags, {"--limit": ("-n", True), "--json": (None, False)})

    def test_an_item_added_without_its_use_fails(self):
        errors, warnings = [], []
        usecases = {"x": {"old": {"usecase": "", "without": ""}, "new": {"usecase": "", "without": ""}}}
        orig = cli.base_items
        cli.base_items = lambda client, base: {"old", "gone"}
        try:
            cli.check_usecases("x", ["old", "new"], "base", usecases, errors, warnings)
        finally:
            cli.base_items = orig
        self.assertEqual(len(errors), 2)
        self.assertTrue(all('"x new"' in e for e in errors))

    def test_an_item_added_with_its_use_passes(self):
        errors, warnings = [], []
        usecases = {"x": {"new": {"usecase": "a script reads it", "without": "it reads the page by hand"}}}
        orig = cli.base_items
        cli.base_items = lambda client, base: set()
        try:
            cli.check_usecases("x", ["new"], "base", usecases, errors, warnings)
        finally:
            cli.base_items = orig
        self.assertEqual(errors, [])


class FastScript(unittest.TestCase):
    def test_fast_sh_fails_on_a_trailer(self):
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8") as f:
            f.write("fix: x\n\nClaude-Session: https://claude.ai/code/session_1\n")
        out = subprocess.run(["bash", str(HERE / "fast.sh"), f.name], capture_output=True, text=True, check=False)
        Path(f.name).unlink()
        self.assertEqual(out.returncode, 1)
        self.assertIn("Claude-Session", out.stderr)


if __name__ == "__main__":
    unittest.main()
