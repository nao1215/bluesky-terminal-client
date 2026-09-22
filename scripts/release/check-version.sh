#!/usr/bin/env bash
#
# check-version.sh checks what a release tag is about to publish:
#
#   - the tag is v plus the version in Cargo.toml;
#   - the Rust toolchain the release is built with is an explicit version
#     (not stable, which changes under a tag) and not older than rust-version;
#   - CHANGELOG.md has a non-empty section for the version (release-notes.sh).
#
# usage: check-version.sh <tag> <pinned toolchain> [repo root]
set -euo pipefail

if [ "$#" -lt 2 ]; then
	echo "usage: $0 <tag> <toolchain> [repo root]" >&2
	exit 2
fi
tag="$1"
toolchain="$2"
root="${3:-.}"

field() {
	sed -n "/^\[package\]/,/^\[/{s/^$1 *= *\"\(.*\)\"/\1/p}" "$root/Cargo.toml" | head -1
}
version="$(field version)"
rust_version="$(field rust-version)"
problems=0

if [ "$tag" != "v$version" ]; then
	echo "error: tag $tag does not match Cargo.toml version $version (expected v$version)" >&2
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
