#!/usr/bin/env bash
# The release version, as llmman's packaging/version.sh decides it:
#
#   packaging/version.sh            # print it, e.g. 0.1.42
#   packaging/version.sh --apply    # also write it into Cargo.toml + Cargo.lock
#
# Every commit on main is a release: MAJOR.MINOR from Cargo.toml (keep its
# PATCH 0), PATCH the number of commits reachable from HEAD. Needs a full
# clone (fetch-depth: 0).

set -euo pipefail
cd -- "$(dirname -- "$0")/.."

die() {
	printf 'version.sh: %s\n' "$*" >&2
	exit 1
}

base="$(sed -n 's/^version = "\([0-9]*\.[0-9]*\)\.[0-9]*"$/\1/p' Cargo.toml | head -n1)"
[ -n "$base" ] || die "no MAJOR.MINOR.PATCH version in Cargo.toml"
[ "$(git rev-parse --is-shallow-repository)" = false ] || die "shallow clone: the commit count would be wrong"
version="$base.$(git rev-list --count HEAD)"

if [ "${1:-}" = --apply ]; then
	# The first `version =` in Cargo.toml is [package]'s; in Cargo.lock,
	# the one right after our own name.
	awk -v v="$version" '!done && /^version = / { sub(/"[^"]*"/, "\"" v "\""); done = 1 } { print }' \
		Cargo.toml >Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	awk -v v="$version" 'hit && /^version = / { sub(/"[^"]*"/, "\"" v "\""); hit = 0 }
		/^name = "llmman-desktop"$/ { hit = 1 } { print }' Cargo.lock >Cargo.lock.tmp && mv Cargo.lock.tmp Cargo.lock
	grep -qx "version = \"$version\"" Cargo.toml || die "failed to write $version to Cargo.toml"
elif [ -n "${1:-}" ]; then
	die "unknown argument: $1"
fi

printf '%s\n' "$version"
