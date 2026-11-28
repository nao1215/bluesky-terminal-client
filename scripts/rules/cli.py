#!/usr/bin/env python3
"""The commands and flags of each client, kept small and tested.

Reads every client's `--help`, and the help of each of its commands, and
checks three things:

surface   doc/cli-surface/<client>.txt lists every command, alias and flag
          the client has. A change to them shows in the pull request's
          diff; `--update` writes the lists again.
usecases  every command, alias or flag added since the base branch has, in
          doc/cli-surface/usecases.toml, what it is for and what happens
          without it. A flag is added for a use someone has (an ego search,
          debimate's announcements), not because it might be handy. Taking
          one away always passes.
e2e       every command is run by at least one atago spec in the crate's
          e2e/atago/, and a client that opens a full-screen client without
          a command has a spec that drives it in a pty. The exceptions are in
          scripts/rules/e2e-allow.toml, each with its reason.

Usage: cli.py [--bin-dir DIR] [--base REF] [--update] [surface|usecases|e2e ...]

DIR holds the built clients (default target/debug). REF is what "added"
is counted from (default: $RULES_BASE, else origin/main); without the
lists there, nothing counts as added.
"""

import argparse
import os
import re
import shlex
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SURFACE_DIR = ROOT / "doc" / "cli-surface"
USECASES = SURFACE_DIR / "usecases.toml"
CLIENTS = Path(__file__).with_name("clients.toml")
E2E_ALLOW = Path(__file__).with_name("e2e-allow.toml")

# Flags clap adds to every program; they are not a choice anyone made.
BUILTIN_FLAGS = {"--help", "--version"}


def help_text(binary, path):
    env = dict(os.environ, LC_ALL="C.UTF-8", LANG="C.UTF-8", NO_COLOR="1", COLUMNS="200")
    out = subprocess.run([binary, *path, "--help"], capture_output=True, text=True, env=env, check=False)
    if out.returncode != 0:
        sys.exit(f"cli: `{Path(binary).name} {' '.join(path)} --help` exited {out.returncode}: {out.stderr.strip()}")
    return out.stdout


def sections(text):
    """The indented lines under each `Heading:` of a help text.

    Long help (`--help` rather than `-h`) puts a blank line between options,
    so a section ends at the next line that is not indented, not at a blank.
    """
    found, current = {}, None
    for line in text.splitlines():
        if re.fullmatch(r"[A-Z][A-Za-z ]*:", line):
            current = line[:-1]
            found[current] = []
        elif not line.strip():
            continue
        elif not line[0].isspace():
            current = None
        elif current:
            found[current].append(line)
    return found


def parse_help(text):
    """(commands, flags): commands as {name: [aliases]}, flags as {long or short: (short, takes a value)}."""
    parts = sections(text)
    commands = {}
    for line in parts.get("Commands", []):
        m = re.match(r"  (\S+)(?:\s{2,}(.*))?$", line)
        if not m or m.group(1) == "help":
            continue
        aliases = []
        a = re.search(r"\[alias(?:es)?: ([^\]]+)\]", m.group(2) or "")
        if a:
            aliases = [x.strip() for x in a.group(1).split(",")]
        commands[m.group(1)] = aliases
    flags = {}
    for line in parts.get("Options", []):
        # clap indents a flag by 2 (or 6 when it has no short form) and its
        # description, possible values included, by 10.
        indent = len(line) - len(line.lstrip())
        if indent > 6 or not line.lstrip().startswith("-"):
            continue
        head = re.split(r"\s{2,}", line.strip(), maxsplit=1)[0]
        takes = "<" in head
        bare = re.sub(r"<[^>]*>|\[[^\]]*\]|\.\.\.", " ", head)
        names = re.findall(r"(?<![\w-])(--?[A-Za-z0-9][\w-]*)", bare)
        long = next((n for n in names if n.startswith("--")), None)
        short = next((n for n in names if not n.startswith("--")), None)
        key = long or short
        if key and key not in BUILTIN_FLAGS and key not in ("-h", "-V"):
            flags[key] = (short if long else None, takes)
    return commands, flags


def source_aliases(crate):
    """Every name a `#[command(alias = ...)]` gives a command in the crate.

    clap leaves an alias that is not visible out of --help, so the list of
    commands read from it would miss one; the source is read for them.
    """
    names = set()
    for src in (ROOT / crate / "src").rglob("*.rs"):
        for attr in re.findall(r"#\[command\(([^\]]*)\)\]", src.read_text(encoding="utf-8")):
            for key, value in re.findall(r"\b((?:visible_)?alias(?:es)?)\s*=\s*(\[[^\]]*\]|\"[^\"]*\")", attr):
                names.update(re.findall(r'"([^"]+)"', value))
    return names


def hidden_items(crate):
    """Places in the crate's source that hide a command or a flag from --help."""
    found = []
    for src in sorted((ROOT / crate / "src").rglob("*.rs")):
        for n, line in enumerate(src.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"#\[(?:command|arg)\([^\]]*\bhide\s*=\s*true", line):
                found.append(f"{src.relative_to(ROOT).as_posix()}:{n}")
    return found


def resolve_alias(binary, path, alias):
    """The command `alias` names under `path`, from the usage line of its help, or None."""
    env = dict(os.environ, LC_ALL="C.UTF-8", LANG="C.UTF-8", NO_COLOR="1", COLUMNS="200")
    out = subprocess.run([binary, *path, alias, "--help"], capture_output=True, text=True, env=env, check=False)
    if out.returncode != 0:
        return None
    m = re.search(r"^Usage: \S+ (.*)$", out.stdout, re.M)
    if not m:
        return None
    words = m.group(1).split()
    if words[: len(path)] != list(path) or len(words) <= len(path):
        return None
    return words[len(path)]


def walk(binary, crate):
    """{command path tuple: (aliases, flags, commands, hidden aliases)} for the program and every command under it."""
    tree = {}

    def visit(path, aliases):
        commands, flags = parse_help(help_text(binary, list(path)))
        tree[path] = (aliases, flags, commands, [])
        for name, sub_aliases in commands.items():
            visit(path + (name,), sub_aliases)

    visit((), [])
    for alias in sorted(source_aliases(crate)):
        for path, (_, _, commands, _) in list(tree.items()):
            if not commands or alias in commands:
                continue
            target = resolve_alias(binary, path, alias)
            if target in commands and alias not in commands[target]:
                tree[path + (target,)][3].append(alias)
                break
    return tree


def surface(tree):
    root_flags = tree[()][1]
    items = []
    for path, (aliases, flags, _, hidden) in tree.items():
        cmd = " ".join(path)
        if path:
            items.append(cmd)
            items.extend(f"{cmd} [alias {a}]" for a in aliases)
            items.extend(f"{cmd} [hidden alias {a}]" for a in hidden)
        for name, (short, _) in flags.items():
            # A global flag shows in every command's help; it is listed once.
            if path and name in root_flags:
                continue
            flag = f"{name} ({short})" if short else name
            items.append(f"{cmd} {flag}".strip())
    return sorted(set(items))


def surface_file(client):
    return SURFACE_DIR / f"{client}.txt"


def render(client, items):
    head = (
        f"# The commands, aliases and flags of {client}, from its --help.\n"
        "# Written by `python3 scripts/rules/cli.py --update`; do not edit by hand.\n"
        "# What each is for is in usecases.toml.\n"
    )
    return head + "".join(f"{i}\n" for i in items)


def read_items(text):
    return {line for line in text.splitlines() if line and not line.startswith("#")}


def base_items(client, base):
    rel = surface_file(client).relative_to(ROOT).as_posix()
    out = subprocess.run(["git", "-C", str(ROOT), "show", f"{base}:{rel}"], capture_output=True, text=True, check=False)
    return read_items(out.stdout) if out.returncode == 0 else None


def check_surface(client, items, update, errors):
    want = render(client, items)
    path = surface_file(client)
    if update:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(want, encoding="utf-8")
        return
    have = path.read_text(encoding="utf-8") if path.exists() else ""
    if have == want:
        return
    added = sorted(set(items) - read_items(have))
    gone = sorted(read_items(have) - set(items))
    errors.append(
        f"surface: {path.relative_to(ROOT)} is not what {client} --help says"
        f" (added: {', '.join(added) or 'none'}; gone: {', '.join(gone) or 'none'});"
        " run `python3 scripts/rules/cli.py --update` and say in usecases.toml what each addition is for"
    )


def check_usecases(client, items, base, usecases, errors, warnings):
    entries = usecases.get(client, {})
    before = base_items(client, base)
    added = set() if before is None else set(items) - before
    for item in sorted(items):
        entry = entries.get(item)
        if entry is None:
            errors.append(f'usecases: [{client}] has no entry for "{item}"; add it to doc/cli-surface/usecases.toml')
            continue
        if item in added:
            for field in ("usecase", "without"):
                if not str(entry.get(field, "")).strip():
                    errors.append(
                        f'usecases: "{client} {item}" is new, so its {field} must say'
                        f" {'who uses it, for what' if field == 'usecase' else 'what happens without it'}"
                        " (a flag that might be handy is not added)"
                    )
    for item in sorted(set(entries) - set(items)):
        warnings.append(f'usecases: [{client}] "{item}" is no longer in {client}; remove its entry')


def spec_commands(crate):
    """(command line, whether it runs in a pty) for every `command:` of the crate's atago specs."""
    found = []
    for spec in sorted((ROOT / crate / "e2e" / "atago").rglob("*.yaml")):
        lines = spec.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            m = re.match(r"(\s*)(?:-\s+)?command:\s*(.*)$", line)
            if not m:
                continue
            value = m.group(2).strip()
            if value in ("|", ">", "|-", ">-"):
                indent = len(m.group(1))
                block = []
                for nxt in lines[i + 1 :]:
                    if nxt.strip() and len(nxt) - len(nxt.lstrip()) <= indent:
                        break
                    block.append(nxt.strip())
                value = " ".join(block)
            elif len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
                value = value[1:-1]
            near = " ".join(lines[max(0, i - 3) : i + 1])
            found.append((value, bool(re.search(r"\bpty:", near))))
    return found


def tokens(value):
    try:
        return shlex.split(value)
    except ValueError:
        return value.split()


def covered(binary, tree, crate):
    """The command paths some spec runs, and whether a spec opens the client in a pty."""
    paths, tui = set(), False
    for value, in_pty in spec_commands(crate):
        toks = tokens(value)
        for start, tok in enumerate(toks):
            if tok != binary and not tok.endswith("/" + binary):
                continue
            path, i = (), start + 1
            while i < len(toks):
                t = toks[i]
                flags = tree.get(path, ((), {}, {}, []))[1] | tree[()][1]
                if t.startswith("-"):
                    name = t.split("=", 1)[0]
                    spec = flags.get(name) or next((v for k, v in flags.items() if v[0] == name), None)
                    i += 2 if spec and spec[1] and "=" not in t else 1
                    continue
                commands = tree.get(path, ((), {}, {}, []))[2]
                name = next((c for c, al in commands.items() if t == c or t in al or t in tree[path + (c,)][3]), None)
                if name is None:
                    break
                path += (name,)
                paths.add(path)
                i += 1
            if not path and in_pty:
                tui = True
    return paths, tui


def check_e2e(client, conf, tree, allow, errors, warnings):
    crate = conf["crate"]
    paths, tui = covered(client, tree, crate)
    allowed = allow.get(client, {})
    for path in sorted(p for p in tree if p):
        cmd = " ".join(path)
        if path in paths:
            if cmd in allowed:
                warnings.append(f'e2e: "{client} {cmd}" is run by a spec now; remove it from scripts/rules/e2e-allow.toml')
            continue
        reason = str(allowed.get(cmd, "")).strip()
        if not reason:
            errors.append(f'e2e: no atago spec in {crate}/e2e/atago runs "{client} {cmd}"; add one (or a reason to scripts/rules/e2e-allow.toml)')
    if conf.get("tui") and not tui:
        errors.append(f"e2e: no atago spec in {crate}/e2e/atago drives `{client}` without a command in a pty")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin-dir", default=str(ROOT / "target" / "debug"))
    ap.add_argument("--base", default=os.environ.get("RULES_BASE", "origin/main"))
    ap.add_argument("--update", action="store_true", help="write doc/cli-surface/<client>.txt again")
    ap.add_argument("checks", nargs="*", default=["surface", "usecases", "e2e"])
    args = ap.parse_args()

    clients = tomllib.loads(CLIENTS.read_text(encoding="utf-8"))
    usecases = tomllib.loads(USECASES.read_text(encoding="utf-8")) if USECASES.exists() else {}
    allow = tomllib.loads(E2E_ALLOW.read_text(encoding="utf-8")) if E2E_ALLOW.exists() else {}
    errors, warnings = [], []
    for client, conf in clients.items():
        exe = Path(args.bin_dir) / (client + (".exe" if os.name == "nt" else ""))
        if not exe.exists():
            sys.exit(f"cli: {exe} is not built; run `cargo build --workspace` first")
        tree = walk(str(exe), conf["crate"])
        for place in hidden_items(conf["crate"]):
            errors.append(f"surface: {place} hides a command or flag from --help, so no list sees it; make it visible")
        items = surface(tree)
        if "surface" in args.checks:
            check_surface(client, items, args.update, errors)
        if "usecases" in args.checks:
            check_usecases(client, items, args.base, usecases, errors, warnings)
        if "e2e" in args.checks:
            check_e2e(client, conf, tree, allow, errors, warnings)
    for w in warnings:
        print(f"warning: {w}", file=sys.stderr)
    for e in errors:
        print(f"error: {e}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
