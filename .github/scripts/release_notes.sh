#!/usr/bin/env bash
# Print the body of a GitHub Release.
#
#   release_notes.sh VERSION WINDOWS_SIGNED MACOS_SIGNED > release-notes.md
#
# If HISTORY.md has a "## VERSION" section it leads the notes. The download
# table follows, with a line for each system whose build is not signed.
# GitHub keeps the line breaks of a release body, so each paragraph below is
# one long line.
set -euo pipefail

version="${1:?usage: release_notes.sh VERSION WINDOWS_SIGNED MACOS_SIGNED}"
windows_signed="${2:-false}"
macos_signed="${3:-false}"

if [ -f HISTORY.md ]; then
  # The section runs to the next "## " heading. Comparing the second field
  # with the version accepts a dated heading and keeps 1.2.1 from matching
  # 1.2.10.
  history="$(awk -v ver="$version" '
    $1 == "##" && $2 == ver { cap = 1; next }
    cap && /^## / { exit }
    cap { print }
  ' HISTORY.md | sed '/./,$!d')"
  if [ -n "$history" ]; then
    printf '%s\n\n' "$history"
  else
    echo "::warning::HISTORY.md has no section for $version; the notes list the downloads only." >&2
  fi
fi

cat <<'EOF'
## Downloads

| System | File |
| --- | --- |
| Windows, x64 | `buttercandy-windows-amd64.zip` |
| Windows, ARM64 | `buttercandy-windows-arm64.zip` |
| macOS 14.2 or newer, Apple silicon and Intel | `buttercandy-macos-universal.dmg` |
| Linux, x64 | `buttercandy-linux-amd64.tar.gz` |
| Linux, ARM64 | `buttercandy-linux-arm64.tar.gz` |

`SHA256SUMS` holds the checksum of each file.

- **Windows:** unzip anywhere and run `buttercandy.exe`.
- **macOS:** open the disk image and drag buttercandy to Applications.
- **Linux:** unpack, then run `./buttercandy --install-desktop` once to add it to the applications menu. It needs a Vulkan driver and glibc 2.39 or newer (Ubuntu 24.04, Fedora 40, Debian 13).
EOF

if [ "$windows_signed" != "true" ]; then
  cat <<'EOF'

The Windows builds are not code-signed. SmartScreen will say it protected your PC: choose **More info**, then **Run anyway**.
EOF
fi

if [ "$macos_signed" != "true" ]; then
  cat <<'EOF'

The macOS build is not signed by an identified developer, so macOS will refuse to open it the first time. Try to open it once, then go to **System Settings › Privacy & Security** and choose **Open Anyway**.
EOF
fi
