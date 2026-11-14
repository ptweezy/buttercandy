#!/usr/bin/env bash
# Decide whether this run releases, and compute the version it builds.
#
# Reads (from the workflow's `env:`):
#   EVENT       github.event_name
#   REF         github.ref
#   BEFORE      github.event.before (the tip this push replaced)
#   AFTER       github.sha
#   INPUT_BUMP  workflow_dispatch input: patch, minor or major
#   INPUT_TAG   workflow_dispatch input: an exact X.Y.Z, which wins over bump
# Writes release, bump and new to $GITHUB_OUTPUT.
#
# Needs a full-history checkout with tags: the marker scan walks
# BEFORE..AFTER, and the next version is counted from the latest tag.
set -euo pipefail

release=false
bump="minor"

# Subjects of the commits this push adds, newest first. BEFORE is all zeros
# on a new branch, unset on a dispatch, and after a force push names a commit
# this checkout may not have: fall back to the tip in each case.
zero="0000000000000000000000000000000000000000"
if [ -z "${BEFORE:-}" ] || [ "$BEFORE" = "$zero" ] \
     || ! git rev-parse -q --verify "${BEFORE}^{commit}" >/dev/null; then
  subjects="$(git log -1 --pretty=%s "$AFTER")"
else
  subjects="$(git log --pretty=%s "${BEFORE}..${AFTER}")"
fi

if [ "$EVENT" = "workflow_dispatch" ]; then
  release=true
  bump="${INPUT_BUMP:-minor}"
elif [ "$EVENT" = "push" ] && [ "$REF" = "refs/heads/main" ]; then
  # A marker counts only at the start of a subject, and the newest marked
  # commit wins. Bodies are never read.
  marker="$(printf '%s\n' "$subjects" \
    | grep -oiE '^\[release(:(major|minor|patch))?\]' \
    | head -n1 \
    | tr '[:upper:]' '[:lower:]' || true)"
  if [ -n "$marker" ]; then
    release=true
    case "$marker" in
      "[release:major]") bump="major" ;;
      "[release:patch]") bump="patch" ;;
      *)                 bump="minor" ;;
    esac
  fi
fi
# Anything else (a pull request, a push to another branch) builds and tests
# only.

# Tags are bare X.Y.Z, no leading "v".
semver='^[0-9]+\.[0-9]+\.[0-9]+$'
latest="$(git tag -l | grep -E "$semver" | sort -V | tail -n1 || true)"
latest="${latest:-0.0.0}"
IFS='.' read -r major minor patch <<<"$latest"

if [ "$release" != "true" ]; then
  # A build-only run is stamped as a preview of the next patch release, so
  # the stamping a release depends on runs on every commit.
  new="${major}.${minor}.$((patch + 1))-dev"
  echo "::notice::Build-only at version $new (no release)."
elif [ -n "${INPUT_TAG:-}" ]; then
  new="$INPUT_TAG"
  if ! printf '%s' "$new" | grep -qE "$semver"; then
    echo "::error::Invalid tag '$new'; expected X.Y.Z (digits only, no leading 'v')." >&2
    exit 1
  fi
  echo "::notice::Releasing $new (explicit tag)."
else
  case "$bump" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
    *)
      echo "::error::Invalid bump '$bump'; expected patch, minor or major." >&2
      exit 1
      ;;
  esac
  new="${major}.${minor}.${patch}"
  echo "::notice::Releasing $new (previous $latest, bump=$bump)."
fi

# Never move an existing tag. A release that failed before publishing left
# no tag behind, so it can simply be run again.
if [ "$release" = "true" ] && git rev-parse -q --verify "refs/tags/$new" >/dev/null; then
  echo "::error::Tag $new already exists; refusing to release it again." >&2
  exit 1
fi

{
  echo "release=$release"
  echo "bump=$bump"
  echo "new=$new"
} >> "$GITHUB_OUTPUT"
