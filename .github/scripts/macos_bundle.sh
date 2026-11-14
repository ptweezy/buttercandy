#!/usr/bin/env bash
# Assemble buttercandy.app from the two macOS builds. Runs on macOS only.
#
#   macos_bundle.sh VERSION ARM64_BINARY X86_64_BINARY OUT_DIR
#
# Leaves OUT_DIR/buttercandy.app, unsigned: the workflow signs it next.
set -euo pipefail

version="${1:?usage: macos_bundle.sh VERSION ARM64_BINARY X86_64_BINARY OUT_DIR}"
arm64="${2:?missing the arm64 binary}"
x86_64="${3:?missing the x86_64 binary}"
out="${4:?missing the output directory}"

app="$out/buttercandy.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

# One binary that runs natively on Apple silicon and on Intel.
lipo -create -output "$app/Contents/MacOS/buttercandy" "$arm64" "$x86_64"
chmod 755 "$app/Contents/MacOS/buttercandy"
archs="$(lipo -archs "$app/Contents/MacOS/buttercandy")"
case "$archs" in
  *arm64*x86_64* | *x86_64*arm64*) ;;
  *)
    echo "macos_bundle.sh: expected arm64 and x86_64, got: $archs" >&2
    exit 1
    ;;
esac
# Apple silicon runs signed code only. Each build was signed by the linker,
# but give the merged file a signature of its own before running it below.
codesign --force --sign - "$app/Contents/MacOS/buttercandy"

# The bundle version is numbers only: a build-only run's "-dev" is dropped.
sed "s/@VERSION@/${version%%-*}/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

# The icon, drawn by the app itself at every size macOS asks for. The slice
# for this machine does the drawing.
iconset="$(mktemp -d)/buttercandy.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  double=$((size * 2))
  "$app/Contents/MacOS/buttercandy" --icon "$iconset/icon_${size}x${size}.png" --size "${size}x${size}"
  "$app/Contents/MacOS/buttercandy" --icon "$iconset/icon_${size}x${size}@2x.png" --size "${double}x${double}"
done
iconutil --convert icns --output "$app/Contents/Resources/buttercandy.icns" "$iconset"

# The licence travels with the program.
cp LICENSE THIRD-PARTY.md "$app/Contents/Resources/"

echo "assembled $app ($archs, version ${version%%-*})"
