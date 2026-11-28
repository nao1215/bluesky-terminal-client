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

# Each client of the repository: its executable, its tag prefix, its package.
clients=("bsky v bluesky-terminal-client" "msky msky-v misskey-terminal-client"
	"mstdn mstdn-v mastodon-terminal-client" "nstr nstr-v nostr-terminal-client")

for client in "${clients[@]}"; do
	read -r bin prefix package <<<"$client"
	export BIN="$bin"
	mkdir -p "$work/$bin"
	dir="$work/$bin"

	# A stand-in executable that answers --version like $bin 1.2.3.
	mkdir -p "$dir/bin"
	printf '#!/bin/sh\necho "%s 1.2.3"\n' "$bin" >"$dir/bin/$bin"
	chmod +x "$dir/bin/$bin"

	for ext in tar.gz zip; do
		a="$dir/a-$bin-v1.2.3-t.$ext"
		b="$dir/b-$bin-v1.2.3-t.$ext"
		"$here/pack-archive.sh" "$dir/bin/$bin" "$a" >/dev/null
		sleep 1
		"$here/pack-archive.sh" "$dir/bin/$bin" "$b" >/dev/null
		cmp -s "$a" "$b" || fail "$bin: $ext archives packed a second apart differ"
		exec_flag=--exec
		# A shell script is not executable on Windows.
		case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exec_flag= ;; esac
		"$here/check-archive.sh" "$a" "${prefix}1.2.3" $exec_flag >/dev/null || fail "$bin: $ext archive does not pass its check"
		if "$here/check-archive.sh" "$a" "${prefix}9.9.9" $exec_flag >/dev/null 2>"$work/err"; then
			[ -z "$exec_flag" ] || fail "$bin: $ext archive passed for the wrong version"
		fi
	done

	# An archive with a leading directory is refused.
	mkdir -p "$dir/nested/dir"
	cp "$dir/bin/$bin" "$dir/nested/dir/$bin"
	tar -czf "$dir/nested.tar.gz" -C "$dir/nested" dir
	if "$here/check-archive.sh" "$dir/nested.tar.gz" "${prefix}1.2.3" >/dev/null 2>"$work/err"; then
		fail "$bin: an archive with a directory entry passed"
	fi

	# The tag must be the client's prefix plus the version in Cargo.toml, and
	# the toolchain must be explicit and new enough.
	printf '[package]\nname = "%s"\nversion = "1.2.3"\nrust-version = "1.90"\n\n[dependencies]\nversion = "9.9.9"\n' "$package" >"$dir/Cargo.toml"
	printf '# Changelog\n\n## [1.2.3] - 2026-09-22\n\n- One thing.\n' >"$dir/CHANGELOG.md"
	"$here/check-version.sh" "${prefix}1.2.3" 1.98.0 "$dir" >/dev/null || fail "$bin: a matching tag was refused"
	for other_client in "${clients[@]}"; do
		read -r other_bin other_prefix _ <<<"$other_client"
		[ "$other_bin" = "$bin" ] && continue
		for script in check-version.sh check-archive.sh render-formula.sh; do
			case "$script" in
			check-version.sh) args=("${other_prefix}1.2.3" 1.98.0 "$dir") ;;
			check-archive.sh) args=("$a" "${other_prefix}1.2.3") ;;
			render-formula.sh) args=("${other_prefix}1.2.3" "$dir/checksums.txt" nao1215/social-terminal-clients) ;;
			esac
			if "$here/$script" "${args[@]}" >/dev/null 2>"$work/err"; then
				fail "$bin: $script accepted $other_bin's tag"
			fi
			grep -q 'error: tag ' "$work/err" || fail "$bin: $script failed for a reason other than the tag"
		done
	done
	for bad in "${prefix}1.2.4 1.98.0" "${prefix}1.2.3 stable" "${prefix}1.2.3 1.89.0" "1.2.3 1.98.0"; do
		# shellcheck disable=SC2086
		if "$here/check-version.sh" $bad "$dir" >/dev/null 2>"$work/err"; then
			fail "$bin: check-version accepted: $bad"
		fi
	done

	printf '# Changelog\n\n## [Unreleased]\n\n- Pending.\n' >"$dir/CHANGELOG.md"
	if "$here/check-version.sh" "${prefix}1.2.3" 1.98.0 "$dir" >/dev/null 2>"$work/err"; then
		fail "$bin: an Unreleased-only changelog passed"
	fi

	# The formula takes each archive's hash from checksums.txt, and refuses a gap.
	for t in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
		printf '%064d  %s-v1.2.3-%s.tar.gz\n' 1 "$bin" "$t"
	done >"$dir/checksums.txt"
	"$here/render-formula.sh" "${prefix}1.2.3" "$dir/checksums.txt" nao1215/social-terminal-clients >"$dir/$bin.rb"
	grep -q "class $(printf '%s' "${bin:0:1}" | tr '[:lower:]' '[:upper:]')${bin:1} < Formula" "$dir/$bin.rb" || fail "$bin: wrong formula class"
	grep -q "releases/download/${prefix}1.2.3/$bin-v1.2.3-" "$dir/$bin.rb" || fail "$bin: wrong archive URL"
	grep -q 'version "1.2.3"' "$dir/$bin.rb" || fail "$bin: the formula has no version"
	grep -q "bin.install \"$bin\"" "$dir/$bin.rb" || fail "$bin: the formula does not install $bin"
	[ "$(grep -c 'sha256 "0\{63\}1"' "$dir/$bin.rb")" -eq 4 ] || fail "$bin: the formula does not carry four hashes"
	if command -v ruby >/dev/null; then ruby -c "$dir/$bin.rb" >/dev/null || fail "$bin: the formula is not valid Ruby"; fi
	sed -i.bak '/aarch64-apple-darwin/d' "$dir/checksums.txt"
	if "$here/render-formula.sh" "${prefix}1.2.3" "$dir/checksums.txt" nao1215/social-terminal-clients >/dev/null 2>"$work/err"; then
		fail "$bin: a formula was rendered without every hash"
	fi
done
unset BIN

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

echo "release scripts: ok"
