#!/usr/bin/env bash
#
# render-formula.sh writes the Homebrew formula for a release from the
# checksums.txt the release publishes, so Homebrew cannot disagree with it.
#
# usage: render-formula.sh <tag> <checksums.txt> <owner/repo>
set -euo pipefail

if [ "$#" -ne 3 ]; then
	echo "usage: $0 <tag> <checksums.txt> <owner/repo>" >&2
	exit 2
fi
tag="$1"
sums="$2"
repo="$3"
version="${tag#v}"

sha() {
	local file="bsky-$tag-$1.tar.gz"
	local sum
	sum="$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1 }' "$sums")"
	if ! [[ "$sum" =~ ^[0-9a-f]{64}$ ]]; then
		echo "error: $sums has no SHA-256 for $file" >&2
		exit 1
	fi
	printf '      url "https://github.com/%s/releases/download/%s/%s"\n      sha256 "%s"' "$repo" "$tag" "$file" "$sum"
}

# Computed before the heredoc: a failure inside $(...) there would not stop it.
mac_intel="$(sha x86_64-apple-darwin)"
mac_arm="$(sha aarch64-apple-darwin)"
linux_intel="$(sha x86_64-unknown-linux-gnu)"
linux_arm="$(sha aarch64-unknown-linux-gnu)"

cat <<RUBY
# typed: false
# frozen_string_literal: true

class Bsky < Formula
  desc "Bluesky client for the terminal that shows pictures and videos inline"
  homepage "https://github.com/$repo"
  version "$version"
  license "MIT"

  on_macos do
    if Hardware::CPU.intel?
$mac_intel
    end

    if Hardware::CPU.arm?
$mac_arm
    end
  end

  on_linux do
    if Hardware::CPU.intel? && Hardware::CPU.is_64_bit?
$linux_intel
    end

    if Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
$linux_arm
    end
  end

  def install
    bin.install "bsky"
  end

  test do
    assert_match "bsky #{version}", shell_output("#{bin}/bsky --version")
  end
end
RUBY
