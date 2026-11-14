#!/usr/bin/env bash
# Stamp a version into Cargo.toml and Cargo.lock, in the working tree only.
#
#   set_version.sh 1.2.3
#
# Releases are numbered by their tag, not by the version committed in
# Cargo.toml, so every build writes its number in before compiling. The lock
# file records the package's own version too; changing both keeps
# `cargo build --locked` satisfied.
set -euo pipefail

version="${1:?usage: set_version.sh X.Y.Z[-suffix]}"
if ! printf '%s' "$version" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'; then
  echo "set_version.sh: '$version' is not a version" >&2
  exit 1
fi

# The `version` key of the [package] table.
awk -v v="$version" '
  /^\[/ { in_package = ($0 == "[package]") }
  in_package && !done && /^version = "/ { print "version = \"" v "\""; done = 1; next }
  { print }
  END { if (!done) exit 1 }
' Cargo.toml > Cargo.toml.stamped || {
  echo "set_version.sh: no [package] version in Cargo.toml" >&2
  rm -f Cargo.toml.stamped
  exit 1
}
mv Cargo.toml.stamped Cargo.toml

# The `version` line that follows `name = "buttercandy"` in the lock file.
awk -v v="$version" '
  /^\[\[package\]\]/ { ours = 0 }
  /^name = "buttercandy"$/ { ours = 1; print; next }
  ours && /^version = "/ { print "version = \"" v "\""; ours = 0; done = 1; next }
  { print }
  END { if (!done) exit 1 }
' Cargo.lock > Cargo.lock.stamped || {
  echo "set_version.sh: no buttercandy package in Cargo.lock" >&2
  rm -f Cargo.lock.stamped
  exit 1
}
mv Cargo.lock.stamped Cargo.lock

echo "stamped version $version"
