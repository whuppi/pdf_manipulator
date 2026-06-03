#!/bin/bash
# Single source of truth for all release stamping. Called by CI and usable locally.
#
# Usage:
#   ./tool/stamp_release.sh <tag> [mode]
#
# Modes (5):
#   --stamp-tag               Prepare tree for the release tag commit:
#                              - Stamp version into pubspec.yaml + version.dart
#                              - Convert submodule pointers to raw source files
#                              - Remove .gitmodules
#                              Called by CI discover job BEFORE git commit.
#
#   --github-notes            Print GitHub Release notes to stdout:
#                              - Extract changelog entry for this version
#                              - Append commit list since previous tag
#                              Called by CI discover job for gh release create --notes.
#
#   (default, no flag)        Stamp for pub.dev publish:
#                              - Re-stamp version (idempotent, tag already has it)
#                              - Build filtered CHANGELOG.md for pub.dev:
#                                only versions published on pub.dev + current,
#                                unpublished intermediate versions merged into
#                                collapsibles, commit list since last pub.dev version
#                              - Generate asset hashes from GitHub Release API
#                              Called by CI publish job AFTER approval.
#
#   --add-git-install         Append git tag install snippet to GitHub Release notes.
#                              Called by CI after binary upload.
#
#   --add-pub-install         Append pub.dev install snippet to GitHub Release notes.
#                              Called by CI after pub.dev publish.
#
# Release pipeline flow (for context):
#   1. discover:  --stamp-tag → git commit → push temp branch → --github-notes → gh release create
#   2. compile:   checkout tag (has raw source, no submodules needed)
#   3. upload:    binaries to release → --add-git-install
#   4. publish:   (default) stamp → dart pub publish → --add-pub-install
#
# Requires: gh CLI authenticated (GH_TOKEN env var or gh auth login)
# Run from package root.

set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: $0 <tag> [mode]"
  echo "Modes: (none)=stamp, --github-notes, --add-git-install, --add-pub-install"
  echo "Example: $0 v1.0.0-dev.0"
  exit 1
fi

TAG="$1"
VERSION="${TAG#v}"
MODE="${2:---stamp}"
REPO="${GITHUB_REPOSITORY:-whuppi/pdf_manipulator}"
REPO_URL="https://github.com/$REPO"

# ── Stamp tag mode: prepare the tree for a release tag commit ───────
# Stamps version + converts submodule pointers to raw source files.
# Called BEFORE git commit in the CI discover job.

if [[ "$MODE" == "--stamp-tag" ]]; then
  if [[ -z "${CI:-}" && -z "${GITHUB_ACTIONS:-}" ]]; then
    echo "⚠ --stamp-tag modifies the working tree (version stamp + submodule de-registration)."
    echo "  This is intended for CI only. Ctrl+C to abort, or press Enter to continue."
    read -r
  fi
  echo "=== Stamping tag tree for $TAG ==="

  # Stamp version
  sed -i.bak "s/^version: .*/version: $VERSION/" pubspec.yaml && rm -f pubspec.yaml.bak
  echo "  pubspec.yaml → $VERSION"

  sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$VERSION'/" lib/src/version.dart && rm -f lib/src/version.dart.bak
  echo "  version.dart → $VERSION"

  # Convert submodule pointers to raw source files.
  # The checkout already has submodules initialized (recursive checkout).
  # We de-register the submodules so git treats vendor/ as regular files.
  for sub in vendor/pdf_oxide vendor/office_oxide; do
    if [ -d "$sub/.git" ] || [ -f "$sub/.git" ]; then
      # Remove submodule registration so git tracks files directly
      git rm --cached "$sub" 2>/dev/null || true
      rm -rf "$sub/.git"
      # Force-add everything including files excluded by vendor .gitignore
      # (Cargo.lock is gitignored in library crates but we need it for
      # deterministic builds + wasm-bindgen-cli version detection)
      git add --force "$sub/"
      echo "  $sub → raw source (de-registered submodule)"
    fi
  done

  # Remove .gitmodules (not needed when source is inline)
  if [ -f .gitmodules ]; then
    git rm --cached .gitmodules 2>/dev/null || true
    rm -f .gitmodules
    echo "  .gitmodules removed"
  fi

  echo "=== Tag tree ready ==="
  exit 0
fi

# ── Quick modes that just edit the GitHub Release notes ─────────────

if [[ "$MODE" == "--add-git-install" ]]; then
  EXISTING=$(gh release view "$TAG" --repo "$REPO" --json body --jq '.body')
  SNIPPET="---

### Install (git tag)

\`\`\`yaml
dependencies:
  pdf_manipulator:
    git:
      url: $REPO_URL.git
      ref: $TAG
\`\`\`"
  gh release edit "$TAG" --repo "$REPO" --notes "$EXISTING"$'\n\n'"$SNIPPET"
  echo "Added git install snippet to $TAG release notes"
  exit 0
fi

if [[ "$MODE" == "--add-pub-install" ]]; then
  EXISTING=$(gh release view "$TAG" --repo "$REPO" --json body --jq '.body')
  SNIPPET="### Install (pub.dev)

\`\`\`yaml
dependencies:
  pdf_manipulator: ^$VERSION
\`\`\`"
  gh release edit "$TAG" --repo "$REPO" --notes "$EXISTING"$'\n\n'"$SNIPPET"
  echo "Added pub.dev install snippet to $TAG release notes"
  exit 0
fi

# ── Guard: reject unknown modes before falling through to stamp ──────

case "$MODE" in
  --stamp|--stamp-tag|--github-notes|--add-git-install|--add-pub-install) ;;
  --*)
    echo "Unknown mode: $MODE"
    echo "Valid modes: (none), --stamp-tag, --github-notes, --add-git-install, --add-pub-install"
    exit 1
    ;;
esac

# ── Helper: get published versions from pub.dev ─────────────────────

get_published_versions() {
  curl -sS "https://pub.dev/api/packages/pdf_manipulator" 2>/dev/null \
    | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for v in data.get('versions', []):
        print(v['version'])
except:
    pass
" 2>/dev/null || true
}

# ── Helper: extract one version's content from a changelog ──────────

extract_entry() {
  local file="$1" version="$2"
  awk "/^## ${version//./\\.}($| )/{found=1; next} /^## /{found=0} found" "$file"
}

# ── Helper: get all ## versions from a changelog ────────────────────

get_changelog_versions() {
  local file="$1"
  grep -oP '^## \K\S+' "$file" 2>/dev/null || true
}

# ═════════════════════════════════════════════════════════════════════
# MODE: GitHub Release notes
# ═════════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--github-notes" ]]; then
  # Determine changelog file
  if [[ "$VERSION" == *-* ]]; then
    FILE="CHANGELOG.pre.md"
  else
    FILE="CHANGELOG.md"
  fi

  # Extract this version's entry
  NOTES=$(extract_entry "$FILE" "$VERSION")

  # Append commits since previous GitHub tag
  PREV_TAG=$(git tag --sort=-v:refname | grep '^v' | grep -v "^${TAG}$" | head -1 || true)
  if [ -n "$PREV_TAG" ]; then
    COMMITS=$(git log "$PREV_TAG"..HEAD --oneline --no-decorate 2>/dev/null || true)
  else
    COMMITS=$(git log --oneline --no-decorate 2>/dev/null || true)
  fi

  if [ -n "$COMMITS" ]; then
    COUNT=$(echo "$COMMITS" | grep -c . || true)
    COMMIT_LIST=$(echo "$COMMITS" | sed 's/^/- /')
    DETAILS_OPEN="<details><summary>Commits since ${PREV_TAG:-initial} ($COUNT)</summary>"
    DETAILS_CLOSE="</details>"
    printf -v NOTES '%s\n\n%s\n\n%s\n\n%s' "$NOTES" "$DETAILS_OPEN" "$COMMIT_LIST" "$DETAILS_CLOSE"
  fi

  echo "$NOTES"
  exit 0
fi

# ═════════════════════════════════════════════════════════════════════
# MODE: Stamp for pub.dev publish
# ═════════════════════════════════════════════════════════════════════

echo "=== Stamping $TAG (version: $VERSION) ==="

# ── 1. pubspec.yaml ─────────────────────────────────────────────────

sed -i.bak "s/^version: .*/version: $VERSION/" pubspec.yaml && rm -f pubspec.yaml.bak
echo "  pubspec.yaml → $VERSION"

# ── 2. version.dart ─────────────────────────────────────────────────

sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$VERSION'/" lib/src/version.dart && rm -f lib/src/version.dart.bak
echo "  version.dart → $VERSION"

# ── 3. Build CHANGELOG.md for pub.dev ───────────────────────────────

# Determine source changelog
if [[ "$VERSION" == *-* ]]; then
  SOURCE_CL="CHANGELOG.pre.md"
else
  SOURCE_CL="CHANGELOG.md"
fi

# Get versions published on pub.dev
PUBLISHED=$(get_published_versions)
echo "  pub.dev has: $(echo "$PUBLISHED" | wc -l | tr -d ' ') published versions"

# Get all versions in the changelog
CL_VERSIONS=$(get_changelog_versions "$SOURCE_CL")

# Build filtered changelog
{
  # Copy the header (everything before the first ## entry)
  awk '/^## /{exit} {print}' "$SOURCE_CL"

  PREV_PUBLISHED=""
  UNPUBLISHED_BUFFER=""

  for V in $CL_VERSIONS; do
    IS_CURRENT=false
    IS_PUBLISHED=false

    [[ "$V" == "$VERSION" ]] && IS_CURRENT=true
    echo "$PUBLISHED" | grep -qx "$V" 2>/dev/null && IS_PUBLISHED=true

    ENTRY=$(extract_entry "$SOURCE_CL" "$V")

    if [[ "$IS_CURRENT" == "true" || "$IS_PUBLISHED" == "true" ]]; then
      # Print this version's heading + content
      echo "## $V"
      echo ""
      echo "$ENTRY"

      # If there are unpublished versions buffered, add them as collapsible
      if [ -n "$UNPUBLISHED_BUFFER" ]; then
        echo ""
        echo "<details><summary>Also includes unpublished changes</summary>"
        echo ""
        echo "$UNPUBLISHED_BUFFER"
        echo "</details>"
      fi
      UNPUBLISHED_BUFFER=""

      # Track for commit range
      if [[ "$IS_CURRENT" == "true" ]]; then
        : # current version, commits appended below
      else
        PREV_PUBLISHED="$V"
      fi

      echo ""
    else
      # Buffer this unpublished version's content
      if [ -n "$UNPUBLISHED_BUFFER" ]; then
        UNPUBLISHED_BUFFER="$UNPUBLISHED_BUFFER"$'\n\n'"### $V"$'\n'"$ENTRY"
      else
        UNPUBLISHED_BUFFER="### $V"$'\n'"$ENTRY"
      fi
    fi
  done
} > /tmp/_changelog_filtered.md

# Determine commit range for pub.dev (since last PUBLISHED version, not last tag)
if [ -n "$PREV_PUBLISHED" ] && git rev-parse "v$PREV_PUBLISHED" &>/dev/null; then
  PUB_PREV_TAG="v$PREV_PUBLISHED"
elif [ -n "$PUBLISHED" ]; then
  # Use the most recent published version's tag
  LATEST_PUBLISHED=$(echo "$PUBLISHED" | head -1)
  if git rev-parse "v$LATEST_PUBLISHED" &>/dev/null; then
    PUB_PREV_TAG="v$LATEST_PUBLISHED"
  fi
fi

COMMITS=""
if [ -n "${PUB_PREV_TAG:-}" ]; then
  COMMITS=$(git log "$PUB_PREV_TAG".."$TAG" --oneline --no-decorate 2>/dev/null || true)
else
  COMMITS=$(git log "$TAG" --oneline --no-decorate 2>/dev/null || true)
fi

if [ -n "$COMMITS" ]; then
  COUNT=$(echo "$COMMITS" | grep -c . || true)
  COMMIT_LIST=$(echo "$COMMITS" | sed 's/^/- /')
  {
    printf '<details><summary>Commits since %s (%s)</summary>\n\n%s\n\n</details>\n' \
      "${PUB_PREV_TAG:-initial}" "$COUNT" "$COMMIT_LIST"
  } >> /tmp/_changelog_filtered.md
fi

cp /tmp/_changelog_filtered.md CHANGELOG.md
rm -f /tmp/_changelog_filtered.md
echo "  CHANGELOG.md built (filtered for pub.dev)"

# ── 4. Asset hashes from GitHub API ─────────────────────────────────

HASH_FILE="lib/src/hook/asset_hashes.dart"

if ! command -v gh &>/dev/null; then
  echo "  ⚠ gh CLI not found — skipping asset hashes"
elif ! gh api "repos/$REPO/releases/tags/$TAG" --silent 2>/dev/null; then
  echo "  ⚠ No GitHub Release for $TAG — skipping asset hashes"
else
  ASSETS=$(gh api "repos/$REPO/releases/tags/$TAG" \
    --jq '.assets[] | select(.digest != null and (.name | startswith("wasm-") | not) and (.name | startswith("Source") | not)) | "\(.name)\t\(.digest)"')

  if [ -z "$ASSETS" ]; then
    echo "  ⚠ No assets with digests — skipping asset hashes"
  else
    {
      echo "// Hashes of pre-built native binaries downloaded by the build hook."
      echo "// Generated by tool/stamp_release.sh from GitHub Release API digests."
      echo "//"
      echo "// The build hook verifies downloaded files against these hashes."
      echo "// Hash mismatch → re-download. Correct → use cached. Missing → download."
      echo ""
      echo "const Map<String, String> assetHashesSha256 = {"
      echo "$ASSETS" | sort | while IFS=$'\t' read -r name digest; do
        hash="${digest#sha256:}"
        echo "  '$name': '$hash',"
        echo "  $name ... ${hash:0:12}..." >&2
      done
      echo "};"
    } > "$HASH_FILE"
    HASH_COUNT=$(echo "$ASSETS" | grep -c . || true)
    echo "  $HASH_FILE → $HASH_COUNT hashes"
  fi
fi

echo ""
echo "=== Done. Ready for: dart pub publish --force ==="
