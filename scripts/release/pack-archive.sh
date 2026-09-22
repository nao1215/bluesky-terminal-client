#!/usr/bin/env bash
#
# pack-archive.sh packs one release binary into a normalized archive.
#
# The release workflow and scripts/release/test.sh both go through this
# script so the two cannot drift. What the archive records about the
# container is pinned here:
#
#   - exactly one entry, the executable, at the archive root (`bsky`, or
#     `bsky.exe` on Windows), with no directory entry or build-tree path;
#   - mode 0755, so extracting on Unix always gives an executable;
#   - owner and group 0 with empty names, so no runner account leaks in and
#     `tar x` as root does not try to chown to a user that does not exist;
#   - the entry time is 2000-01-01T00:00:00Z, and gzip stores no time or name;
#   - a ZIP carries no extra fields, so no packer's timestamps or zone ride along.
#
# The archives are therefore byte-for-byte reproducible for a given binary.
#
# usage: pack-archive.sh <binary-path> <archive-path (.tar.gz or .zip)>
set -euo pipefail

if [ "$#" -ne 2 ]; then
	echo "usage: $0 <binary-path> <archive-path>" >&2
	exit 2
fi
binary_path="$1"
archive_path="$2"
[ -f "$binary_path" ] || { echo "error: binary not found: $binary_path" >&2; exit 1; }
binary_name="$(basename "$binary_path")"

case "$archive_path" in
*.tar.gz) format=tar.gz ;;
*.zip) format=zip ;;
*) echo "error: cannot infer the archive format from $archive_path" >&2; exit 2 ;;
esac

archive_dir="$(cd "$(dirname "$archive_path")" && pwd)"
archive_abs="$archive_dir/$(basename "$archive_path")"
rm -f "$archive_abs"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
cp "$binary_path" "$staging/$binary_name"
chmod 0755 "$staging/$binary_name"
# touch -t reads local time, so the zone is pinned too. 2000 rather than the
# epoch because ZIP cannot hold times before 1980.
TZ=UTC0 touch -t 200001010000 "$staging/$binary_name"

case "$format" in
tar.gz)
	flags=(--format=ustar)
	if tar --version | head -1 | grep -q "GNU tar"; then
		flags+=(--owner=0 --group=0 --numeric-owner --mtime=@946684800)
	else
		# bsdtar (macOS) has no --mtime; the staged file's time is set above.
		flags+=(--uid 0 --gid 0 --uname "" --gname "" --numeric-owner)
	fi
	tar "${flags[@]}" -cf - -C "$staging" "$binary_name" | gzip -9 -n >"$archive_abs"
	;;
zip)
	# Python's zipfile writes exactly the entry it is given: a fixed time,
	# Unix mode 0755, and no extra fields. On Windows the paths Git Bash hands
	# a native program are translated first.
	src="$staging/$binary_name"
	dst="$archive_abs"
	if command -v cygpath >/dev/null; then
		src="$(cygpath -w "$src")"
		dst="$(cygpath -w "$dst")"
	fi
	python_bin="$(command -v python3 || command -v python)"
	"$python_bin" - "$src" "$dst" "$binary_name" <<'PY'
import sys, zipfile
src, dst, name = sys.argv[1:4]
info = zipfile.ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
info.create_system = 3  # Unix, so the mode below is honored when extracted there
info.external_attr = (0o100755 & 0xFFFF) << 16
info.compress_type = zipfile.ZIP_DEFLATED
with open(src, "rb") as f, zipfile.ZipFile(dst, "w") as z:
    z.writestr(info, f.read(), compresslevel=9)
PY
	;;
esac
echo "$archive_abs"
