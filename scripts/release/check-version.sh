#!/usr/bin/env bash
#
# check-version.sh checks what a release tag is about to publish:
#
#   - the tag is the client's prefix (v for bsky, <bin>-v otherwise; see
#     client.sh) plus the version in the package's Cargo.toml;
#   - the Rust toolchain the release is built with is an explicit version
#     (not stable, which changes under a tag) and not older than rust-version;
#   - CHANGELOG.md has a non-empty section for the version (release-notes.sh).
#
# usage: [BIN=bsky|msky|mstdn|nstr] check-version.sh <tag> <pinned toolchain> [package dir]
set -euo pipefail

if [ "$#" -lt 2 ]; then
	echo "usage: [BIN=bsky|msky|mstdn|nstr] $0 <tag> <toolchain> [package dir]" >&2
	exit 2
fi
# shellcheck source=client.sh
. "$(dirname "$0")/client.sh"
tag="$1"
toolchain="$2"
root="${3:-$package_dir}"

# A key of the [package] table; awk rather than sed, whose range syntax
# differs between GNU and BSD.
field() {
	awk -v key="$1" '
		/^\[/ { in_pkg = ($0 == "[package]"); next }
		in_pkg && $1 == key { sub(/^[^"]*"/, ""); sub(/".*$/, ""); print; exit }
	' "$root/Cargo.toml"
}
version="$(field version)"
rust_version="$(field rust-version)"
problems=0

if [ "$tag" != "$tag_prefix$version" ]; then
	echo "error: tag $tag does not match $root/Cargo.toml version $version (expected $tag_prefix$version)" >&2
	problems=1
fi
if ! [[ "$toolchain" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
	echo "error: the release toolchain '$toolchain' is not an explicit version" >&2
	problems=1
elif [ "$(printf '%s\n%s\n' "$rust_version" "$toolchain" | sort -V | head -1)" != "$rust_version" ]; then
	echo "error: the release toolchain $toolchain is older than rust-version $rust_version" >&2
	problems=1
fi
if ! "$(dirname "$0")/release-notes.sh" "$version" "$root/CHANGELOG.md" >/dev/null; then
	problems=1
fi
[ "$problems" -eq 0 ] && echo "ok: $tag, toolchain $toolchain (rust-version $rust_version)"
exit "$problems"
