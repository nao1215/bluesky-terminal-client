#!/usr/bin/env bash
#
# test.sh exercises the release scripts without a tag, so a change to them is
# caught on a pull request rather than on release day. CI runs it on Linux,
# macOS, and Windows (Git Bash), the three places the release packs archives.
#
# usage: scripts/release/test.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

# A stand-in executable that answers --version like bsky 1.2.3.
mkdir -p "$work/bin"
printf '#!/bin/sh\necho "bsky 1.2.3"\n' >"$work/bin/bsky"
chmod +x "$work/bin/bsky"

for ext in tar.gz zip; do
	a="$work/a-bsky-v1.2.3-t.$ext"
	b="$work/b-bsky-v1.2.3-t.$ext"
	"$here/pack-archive.sh" "$work/bin/bsky" "$a" >/dev/null
	sleep 1
	"$here/pack-archive.sh" "$work/bin/bsky" "$b" >/dev/null
	cmp -s "$a" "$b" || fail "$ext archives packed a second apart differ"
	exec_flag=--exec
	# A shell script is not executable on Windows.
	case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exec_flag= ;; esac
	"$here/check-archive.sh" "$a" v1.2.3 $exec_flag >/dev/null || fail "$ext archive does not pass its check"
	if "$here/check-archive.sh" "$a" v9.9.9 $exec_flag >/dev/null 2>"$work/err"; then
		[ -z "$exec_flag" ] || fail "$ext archive passed for the wrong version"
	fi
done

# An archive with a leading directory is refused.
mkdir -p "$work/nested/dir"
cp "$work/bin/bsky" "$work/nested/dir/bsky"
tar -czf "$work/nested.tar.gz" -C "$work/nested" dir
if "$here/check-archive.sh" "$work/nested.tar.gz" v1.2.3 >/dev/null 2>"$work/err"; then
	fail "an archive with a directory entry passed"
fi

# Release notes come from the version's CHANGELOG section only.
cat >"$work/CHANGELOG.md" <<'MD'
# Changelog

## [Unreleased]

## [1.2.3] - 2026-09-22

### Added

- One thing.

## [1.2.2] - 2026-09-01

- Older.
MD
notes="$("$here/release-notes.sh" 1.2.3 "$work/CHANGELOG.md")"
[ "$notes" = "$(printf '### Added\n\n- One thing.')" ] || fail "release notes were: $notes"
if "$here/release-notes.sh" 1.2.4 "$work/CHANGELOG.md" >/dev/null 2>"$work/err"; then
	fail "a version without a section gave notes"
fi
if "$here/release-notes.sh" Unreleased "$work/CHANGELOG.md" >/dev/null 2>"$work/err"; then
	fail "an empty section gave notes"
fi

# The tag must match Cargo.toml and the toolchain must be explicit and new enough.
cat >"$work/Cargo.toml" <<'TOML'
[package]
name = "bluesky-terminal-client"
version = "1.2.3"
rust-version = "1.90"

[dependencies]
version = "9.9.9"
TOML
"$here/check-version.sh" v1.2.3 1.98.0 "$work" >/dev/null || fail "a matching tag was refused"
for bad in "v1.2.4 1.98.0" "v1.2.3 stable" "v1.2.3 1.89.0"; do
	# shellcheck disable=SC2086
	if "$here/check-version.sh" $bad "$work" >/dev/null 2>"$work/err"; then
		fail "check-version accepted: $bad"
	fi
done

# The formula takes each archive's hash from checksums.txt, and refuses a gap.
for t in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
	printf '%064d  bsky-v1.2.3-%s.tar.gz\n' 1 "$t"
done >"$work/checksums.txt"
"$here/render-formula.sh" v1.2.3 "$work/checksums.txt" nao1215/bluesky-terminal-client >"$work/bsky.rb"
grep -q 'version "1.2.3"' "$work/bsky.rb" || fail "the formula has no version"
[ "$(grep -c 'sha256 "0\{63\}1"' "$work/bsky.rb")" -eq 4 ] || fail "the formula does not carry four hashes"
if command -v ruby >/dev/null; then ruby -c "$work/bsky.rb" >/dev/null || fail "the formula is not valid Ruby"; fi
sed -i.bak '/aarch64-apple-darwin/d' "$work/checksums.txt"
if "$here/render-formula.sh" v1.2.3 "$work/checksums.txt" nao1215/bluesky-terminal-client >/dev/null 2>"$work/err"; then
	fail "a formula was rendered without every hash"
fi

echo "release scripts: ok"
