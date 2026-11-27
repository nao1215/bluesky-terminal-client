#!/usr/bin/env bash
#
# release-notes.sh prints the body of CHANGELOG.md's section for a version,
# `## [X.Y.Z] - date`, which becomes the GitHub release text. It fails when the
# section is missing or empty, so a tag cannot publish without notes.
#
# usage: release-notes.sh <version without v> [CHANGELOG.md]
set -euo pipefail

if [ "$#" -lt 1 ]; then
	echo "usage: $0 <version> [changelog]" >&2
	exit 2
fi
version="$1"
changelog="${2:-CHANGELOG.md}"

body="$(awk -v v="$version" '
	index($0, "## [" v "]") == 1 { on = 1; next }
	on && /^## \[/ { exit }
	on { print }
' "$changelog" | sed -e '/./,$!d')"
# Trailing blank lines too.
body="$(printf '%s\n' "$body" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"

if [ -z "$body" ]; then
	echo "error: $changelog has no notes under '## [$version]'; move [Unreleased] to it before tagging" >&2
	exit 1
fi
printf '%s\n' "$body"
