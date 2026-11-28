#!/usr/bin/env bash
#
# check-archive.sh checks one packed release archive: it holds exactly the
# executable at its root with mode 0755, and, with --exec on a runner that can
# run it, the executable reports the version of the tag.
#
# usage: [BIN=bsky|msky|mstdn|nstr] check-archive.sh <archive> <tag> [--exec]
set -euo pipefail

if [ "$#" -lt 2 ]; then
	echo "usage: $0 <archive> <tag> [--exec]" >&2
	exit 2
fi
# shellcheck source=client.sh
. "$(dirname "$0")/client.sh"
archive="$1"
tag="$2"
exec_check="${3:-}"
version="$(tag_version "$tag")"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
python_bin="$(command -v python3 || command -v python)"
src="$archive"
out="$work"
if command -v cygpath >/dev/null; then
	src="$(cygpath -w "$archive")"
	out="$(cygpath -w "$work")"
fi

# Python reads both formats the same way on all three runners.
"$python_bin" - "$src" "$out" "$bin" <<'PY'
import os, stat, sys, tarfile, zipfile
archive, out, exe = sys.argv[1:4]
problems = []
if archive.endswith(".zip"):
    with zipfile.ZipFile(archive) as z:
        infos = z.infolist()
        names = [i.filename for i in infos]
        modes = [(i.external_attr >> 16) & 0o777 for i in infos]
        extras = [i.extra for i in infos]
        z.extractall(out)
        # zipfile does not restore the mode it stored; do it so --exec runs.
        for i in infos:
            os.chmod(os.path.join(out, i.filename), (i.external_attr >> 16) & 0o777)
    if any(extras):
        problems.append("a ZIP entry carries extra fields")
else:
    with tarfile.open(archive, "r:gz") as t:
        members = t.getmembers()
        names = [m.name for m in members]
        modes = [m.mode & 0o777 for m in members]
        if any(m.uid or m.gid or m.uname or m.gname for m in members):
            problems.append("an entry names an owner other than 0 with empty names")
        if any(m.mtime != 946684800 for m in members):
            problems.append("an entry time is not 2000-01-01T00:00:00Z")
        t.extractall(out, filter="data") if hasattr(tarfile, "data_filter") else t.extractall(out)
if names not in ([exe], [exe + ".exe"]):
    problems.append(f"entries are {names}, expected only {exe} or {exe}.exe")
if modes and modes[0] != 0o755:
    problems.append(f"mode is {oct(modes[0])}, expected 0o755")
for p in problems:
    print(f"error: {p}", file=sys.stderr)
sys.exit(1 if problems else 0)
PY

if [ "$exec_check" = "--exec" ]; then
	exe="$work/$bin"
	[ -f "$exe" ] || exe="$work/$bin.exe"
	got="$("$exe" --version)"
	if [ "$got" != "$bin $version" ]; then
		echo "error: $exe --version printed '$got', expected '$bin $version'" >&2
		exit 1
	fi
fi
echo "ok: $archive"
