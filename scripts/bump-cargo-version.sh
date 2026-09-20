#!/usr/bin/env bash
# Bump the crate version in Cargo.toml and Cargo.lock to the given version.
# Used by semantic-release (prepare step) so GitHub tags and crates.io stay
# in sync. Pure text edit: no cargo invocation, works offline.
set -euo pipefail

VERSION="${1:?Usage: bump-cargo-version.sh <version>}"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
    echo "error: '$VERSION' is not a valid semver version" >&2
    exit 1
fi

# Cargo.toml: the top-level `version = "..."` line belongs to [package].
sed -i.bak -E "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
rm -f Cargo.toml.bak

# Cargo.lock: update the version in our own package stanza.
perl -0pi -e "s/name = \"lua-mutation-test\"\nversion = \"[^\"]+\"/name = \"lua-mutation-test\"\nversion = \"${VERSION}\"/" Cargo.lock

echo "Bumped lua-mutation-test to ${VERSION}:"
grep -A1 '^name = "lua-mutation-test"$' Cargo.lock
