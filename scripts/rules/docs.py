#!/usr/bin/env python3
"""How the READMEs, the CHANGELOGs and doc/ are written.

- No bold (`**`) in any of them: a page that marks some words as important
  makes the rest read as filler.
- An English paragraph is one line. Lists, tables, code, headings, quotes
  and HTML are left alone.
- A README says what exists now: no "coming soon", "planned", "not yet",
  "same as ..." and the like (a CHANGELOG's Unreleased section is the place
  for what is on its way).
- The root README is the whole story on one page: it links to no other
  Markdown file of this repository.

Usage: docs.py [FILE.md ...]   (default: every file the rules cover)
Exit status 1 when a rule is broken, each on its own line on stderr.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Words a README writes about what does not exist yet, or to claim two
# things are the same. Matched as whole words, case-insensitive. "yet" alone
# is left out: "not yet" says the same thing, and "yet" also means "but".
FORWARD = [
    r"coming soon",
    r"later (?:on|versions?|releases?)",
    r"in (?:a|the) future",
    r"planned",
    r"not yet",
    r"will be (?:added|supported|available)",
    r"to ?do",
    r"wip",
    r"work in progress",
    r"roadmap",
    r"same as",
    r"identical to",
]
FORWARD_RE = re.compile(r"\b(?:" + "|".join(FORWARD) + r")\b", re.I)


def covered_files():
    out = subprocess.run(["git", "-C", str(ROOT), "ls-files", "*.md"], capture_output=True, text=True, check=True)
    files = []
    for rel in out.stdout.split():
        p = Path(rel)
        name = p.name
        if name.startswith("README") or name == "CHANGELOG.md" or "doc" in p.parts:
            files.append(ROOT / p)
    return files


def prose(lines):
    """(line number, text, in a paragraph) for each line outside code blocks."""
    fence = None
    for n, line in enumerate(lines, 1):
        m = re.match(r"\s*(```+|~~~+)", line)
        if m:
            mark = m.group(1)[0] * 3
            if fence is None:
                fence = mark
            elif mark == fence:
                fence = None
            continue
        if fence is None:
            yield n, line


def is_block_line(line):
    s = line.strip()
    return (
        not s
        or s.startswith(("#", "|", ">", "<", "-", "*", "+", "![", "[!", "    "))
        or re.match(r"\d+[.)]\s", s)
        or line.startswith(("    ", "\t"))
        or re.fullmatch(r"\[[^\]]+\]:\s*\S+.*", s)
    )


def unreleased_lines(lines):
    """The line numbers of a CHANGELOG's `## [Unreleased]` section."""
    inside, found = False, set()
    for n, line in enumerate(lines, 1):
        if line.startswith("## "):
            inside = "unreleased" in line.lower()
        elif inside:
            found.add(n)
    return found


def splits_readme(target):
    """Whether a link of the root README hands its story to another page here.

    A crate's README or a page under doc/ would; CONTRIBUTING.md, SECURITY.md
    and the license are where every repository keeps them, and stay allowed.
    """
    path = target.split("#")[0]
    m = re.match(r"https?://github\.com/nao1215/social-terminal-clients/(?:blob|tree)/[^/]+/(.*)", path)
    if m:
        path = m.group(1)
    elif re.match(r"[a-z]+:", path) or not path:
        return False
    path = path.lstrip("./")
    return path.endswith(".md") and (Path(path).name.startswith("README") or path.startswith(("crates/", "doc/")))


def check(path, errors):
    # A file outside the checkout (the tests' samples) is read as if it were
    # at the root.
    rel = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.name
    lines = path.read_text(encoding="utf-8").splitlines()
    is_readme = path.name.startswith("README") and len(Path(rel).parts) <= 3
    is_root_readme = rel == "README.md"
    skip_forward = unreleased_lines(lines) if path.name == "CHANGELOG.md" else set()
    prev_para = False
    for n, line in prose(lines):
        bare = re.sub(r"`[^`]*`", "", line)
        if "**" in bare:
            errors.append(f"{rel}:{n}: bold (**) is not used; say why it matters in words, or cut it")
        para = not is_block_line(line) and bool(re.search(r"[A-Za-z]", line))
        if para and prev_para:
            errors.append(f"{rel}:{n}: a paragraph is one line; join this line to the one before it")
        prev_para = para
        if is_readme and n not in skip_forward:
            # A link's address is not prose: drop it before looking for words.
            m = FORWARD_RE.search(re.sub(r"\]\([^)]*\)", "]", bare))
            if m:
                errors.append(f'{rel}:{n}: "{m.group(0)}": a README says what exists now, not what is on its way or the same as something else')
        if is_root_readme:
            for target in re.findall(r"\]\(([^)\s]+)", bare) + re.findall(r'href="([^"]+)"', bare):
                if splits_readme(target):
                    errors.append(f"{rel}:{n}: the root README is one page; it does not send the reader to {target}")


def main():
    files = [Path(a).resolve() for a in sys.argv[1:]] or covered_files()
    errors = []
    for f in files:
        if f.suffix == ".md" and f.exists():
            check(f, errors)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
