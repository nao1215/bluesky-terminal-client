# shellcheck shell=bash
#
# client.sh is sourced by the release scripts. It reads BIN, the executable a
# release ships (bsky when unset), and sets what differs between the clients
# of this repository:
#
#   bin           the executable, and the prefix of every release file name
#   tag_prefix    what a release tag starts with before the version
#   package_dir   the package's directory, relative to the repository root
#   formula_class the Homebrew formula's class name
#   formula_desc  the Homebrew formula's description
#
# bsky is tagged vX.Y.Z and msky msky-vX.Y.Z, so the two releases never take
# each other's tags.
bin="${BIN:-bsky}"
case "$bin" in
bsky)
	tag_prefix="v"
	package_dir="crates/bluesky-terminal-client"
	formula_class="Bsky"
	formula_desc="Bluesky client for the terminal that shows pictures and videos inline"
	;;
msky)
	tag_prefix="msky-v"
	package_dir="crates/misskey-terminal-client"
	formula_class="Msky"
	formula_desc="Misskey client for the terminal that draws pictures and videos inline"
	;;
*)
	echo "error: BIN is '$bin', expected bsky or msky" >&2
	exit 2
	;;
esac
# Used by the scripts that source this file.
export bin tag_prefix package_dir formula_class formula_desc

# tag_version <tag> prints the version a tag of this client names, and fails
# for a tag of the other client or one without the prefix.
tag_version() {
	case "$1" in
	"$tag_prefix"[0-9]*) printf '%s\n' "${1#"$tag_prefix"}" ;;
	*)
		echo "error: tag $1 is not $tag_prefix<version>, the form of $bin's tags" >&2
		return 1
		;;
	esac
}
