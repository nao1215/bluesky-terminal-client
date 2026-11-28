"""Generate the official lockfile while retaining reviewed full-SHA workflows."""
import argparse
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def workflow_refs(originals):
    """Collect repository-level action keys; action subpaths share one pin."""
    result = {}
    for path, data in originals.items():
        refs = set()
        for raw in re.findall(r'^\s*(?:-\s*)?uses:\s*[\'\"]?([^\s\'\"#]+)', data.decode(), re.M):
            if raw.startswith('./'):
                continue
            # Reusable workflows are outside the caller's action lockfile, and
            # the SLSA generator only accepts a tag reference.
            if re.search(r'/\.github/workflows/[^/@]+\.ya?ml@', raw):
                continue
            match = re.fullmatch(r'([^/]+/[^/@]+)(?:/[^@]+)?@([0-9a-f]{40})', raw)
            if not match:
                raise ValueError(f'{path}: expected a full-SHA external reference')
            refs.add(match[1].lower()+'@'+match[2])
        result[path.relative_to(ROOT).as_posix()] = refs
    return result


def lock_sections(text):
    """Read only the pinned generator's layout and reject unknown formats."""
    if "version: 'v0.0.3'\nworkflows:\n" not in text or '\ndependencies:\n' not in text:
        raise ValueError('unsupported actions-lock layout; review the generator upgrade')
    workflows_text, dependencies_text = text.split('\ndependencies:\n', 1)
    workflows = {}
    for key, block in re.findall(r"^    '([^']+)':\n((?:        .*\n)*)", workflows_text+'\n', re.M):
        workflows[key] = set(re.findall(r"^        - '([^']+)'$", block, re.M))
    dependencies = {}
    for key, block in re.findall(r"^    '([^']+)':\n((?:        .*\n)*)", dependencies_text, re.M):
        commit = re.search(r"^        commit: 'sha1-([0-9a-f]{40})'$", block, re.M)
        if not commit or key in dependencies:
            raise ValueError(f'invalid dependency entry: {key}')
        dependencies[key] = commit[1]
    if not workflows or not dependencies:
        raise ValueError('empty or unsupported actions-lock sections')
    return workflows, dependencies


def check_exact_refs(text, expected):
    workflows, dependencies = lock_sections(text)
    # A workflow that only calls local ./ workflows is written as `[]`.
    nonempty = lambda refs: {k: v for k, v in refs.items() if v}
    if nonempty(workflows) != nonempty(expected):
        raise ValueError('lockfile workflow references differ from the full-SHA workflows')
    for refs in expected.values():
        for key in refs:
            if dependencies.get(key) != key.rsplit('@', 1)[1]:
                raise ValueError(f'lockfile lacks the exact SHA dependency: {key}')


def retain_sha_keys(text, expected):
    """Translate only aliases whose official resolution equals the reviewed SHA."""
    _, dependencies = lock_sections(text)
    aliases = {}
    for refs in expected.values():
        for key in refs:
            repo, sha = key.rsplit('@', 1)
            matches = [name for name, commit in dependencies.items()
                       if name.rsplit('@', 1)[0] == repo and commit == sha]
            if not matches:
                raise ValueError(f'official generator did not resolve the reviewed pin: {key}')
            for name in matches:
                if name != key:
                    if key in dependencies:
                        raise ValueError(f'ambiguous aliases for {key}; review the lockfile')
                    aliases[name] = key
    # Only complete quoted keys and list entries are replaced. Metadata (ref,
    # commit, owner/repository IDs) and every transitive edge remain intact.
    text = re.sub(r"'([^']+)'", lambda m: "'"+aliases.get(m[1], m[1])+"'", text)
    check_exact_refs(text, expected)
    return text


def refresh_workflows(text, expected):
    """Reuse reviewed dependency metadata when no action pin has changed."""
    _, dependencies = lock_sections(text)
    for refs in expected.values():
        for key in refs:
            if dependencies.get(key) != key.rsplit('@', 1)[1]:
                raise ValueError(f'new or changed action pin needs the official generator: {key}')
    header, rest = text.split('workflows:\n', 1)
    _, metadata = rest.split('\ndependencies:\n', 1)
    entries = []
    for path, refs in sorted(expected.items()):
        entries.append(f"    '{path}':" + ("" if refs else " []"))
        entries.extend(f"        - '{ref}'" for ref in sorted(refs))
    refreshed = header + 'workflows:\n' + '\n'.join(entries) + '\ndependencies:\n' + metadata
    check_exact_refs(refreshed, expected)
    return refreshed


def main(check=False, refresh=False):
    originals = {path: path.read_bytes() for path in (ROOT/'.github/workflows').glob('*.y*ml')}
    expected = workflow_refs(originals)
    lock = ROOT/'.github/workflows/actions.lock'
    if check:
        check_exact_refs(lock.read_text(), expected)
        print(f'exact SHA lockfile coverage: {len(expected)} workflows')
        return
    if refresh:
        lock.write_text(refresh_workflows(lock.read_text(), expected))
        print(f'reused locked dependencies for {len(expected)} workflows')
        return
    try:
        subprocess.run(['gh', 'actions-lock'], cwd=ROOT, check=True)
    finally:
        # v0.4.1 renders tags for SHA inputs; Scorecard still reads workflows.
        for path, data in originals.items():
            path.write_bytes(data)
    lock.write_text(retain_sha_keys(lock.read_text(), expected))
    subprocess.run(['gh', 'actions-lock', '--verify'], cwd=ROOT, check=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--refresh-workflows', action='store_true', help='update workflow coverage offline using only already locked action pins')
    mode.add_argument('--check', action='store_true', help='check exact SHA coverage without writes or network')
    args = parser.parse_args()
    main(check=args.check, refresh=args.refresh_workflows)
