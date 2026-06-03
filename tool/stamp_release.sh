#!/bin/bash
# ────────────────────────────────────────────────────────────────────
# stamp_release.sh — Single source of truth for all release stamping.
#
# Five modes:
#
#   --stamp-tag          Prepare tree for the release tag commit:
#                         stamp version, convert submodule pointers
#                         to raw source, remove .gitmodules.
#                         Called by CI discover job BEFORE git commit.
#
#   --github-notes       Print GitHub Release notes to stdout:
#                         changelog entry + commit list since prev tag.
#                         Called by CI discover job for gh release create.
#
#   (default)            Stamp for pub.dev publish:
#                         re-stamp version, build filtered CHANGELOG.md
#                         (only published versions + current), generate
#                         asset hashes from GitHub Release API.
#                         Called by CI publish job AFTER approval.
#
#   --add-git-install    Append git tag install snippet to release notes.
#                         Called by CI after binary upload.
#
#   --add-pub-install    Append pub.dev install snippet to release notes.
#                         Called by CI after pub.dev publish.
#
# Pipeline flow:
#   1. discover:  --stamp-tag → commit → push → --github-notes → gh release
#   2. compile:   checkout tag (raw source, no submodules)
#   3. upload:    binaries → --add-git-install
#   4. publish:   (default) stamp → dart pub publish → --add-pub-install
#
# Requires: gh CLI authenticated (GH_TOKEN or gh auth login)
# Run from: package root
# ────────────────────────────────────────────────────────────────────
set -euo pipefail


# ═══════════════════════════════════════════════════════════════════
# Args + globals
# ═══════════════════════════════════════════════════════════════════

if [ -z "${1:-}" ]; then
  echo "Usage: $0 <tag> [mode]"
  echo "Modes: (none)=stamp, --stamp-tag, --github-notes, --add-git-install, --add-pub-install"
  echo "Example: $0 v1.0.0-dev.0"
  exit 1
fi

TAG="$1"
VERSION="${TAG#v}"
MODE="${2:---stamp}"
REPO="${GITHUB_REPOSITORY:-whuppi/pdf_manipulator}"
REPO_URL="https://github.com/$REPO"


# ═══════════════════════════════════════════════════════════════════
# Helpers
# ═══════════════════════════════════════════════════════════════════

# Stamp version into pubspec.yaml + version.dart.
stamp_version() {
  sed -i.bak "s/^version: .*/version: $VERSION/" pubspec.yaml && rm -f pubspec.yaml.bak
  echo "  pubspec.yaml → $VERSION"

  sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$VERSION'/" \
    lib/src/version.dart && rm -f lib/src/version.dart.bak
  echo "  version.dart → $VERSION"
}

# Get all published versions from pub.dev.
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

# Extract one version's content from a changelog file.
#   $1 = changelog file path
#   $2 = version string (e.g. "1.0.0")
extract_entry() {
  local file="$1" version="$2"
  awk "/^## ${version//./\\.}($| )/{found=1; next} /^## /{found=0} found" "$file"
}

# List all ## version headings in a changelog file.
get_changelog_versions() {
  grep -oP '^## \K\S+' "$1" 2>/dev/null || true
}


# ═══════════════════════════════════════════════════════════════════
# Mode: --stamp-tag
# ═══════════════════════════════════════════════════════════════════
# Prepare the working tree for an orphan tag commit. Stamps version,
# de-registers submodules so vendor/ becomes regular tracked files,
# and adds false_secrets for vendor test keys.

if [[ "$MODE" == "--stamp-tag" ]]; then
  if [[ -z "${CI:-}" && -z "${GITHUB_ACTIONS:-}" ]]; then
    echo "⚠ --stamp-tag modifies the working tree (version stamp + submodule de-registration)."
    echo "  This is intended for CI only. Ctrl+C to abort, or press Enter to continue."
    read -r
  fi

  echo "=== Stamping tag tree for $TAG ==="
  stamp_version

  # Convert submodule pointers to raw source files.
  for sub in vendor/pdf_oxide vendor/office_oxide; do
    if [ -d "$sub/.git" ] || [ -f "$sub/.git" ]; then
      git rm --cached "$sub" 2>/dev/null || true
      rm -rf "$sub/.git"
      # Force-add includes files excluded by vendor .gitignore
      # (Cargo.lock is gitignored in library crates but needed for
      # deterministic builds + wasm-bindgen-cli version detection).
      git add --force "$sub/"
      echo "  $sub → raw source (de-registered submodule)"
    fi
  done

  if [ -f .gitmodules ]; then
    git rm --cached .gitmodules 2>/dev/null || true
    rm -f .gitmodules
    echo "  .gitmodules removed"
  fi

  # Add false_secrets for vendor test PEM keys (pub.dev security scanner).
  if ! grep -q '/vendor/\*\*' pubspec.yaml; then
    sed -i.bak '/^false_secrets:/a\  - /vendor/**' pubspec.yaml && rm -f pubspec.yaml.bak
    echo "  pubspec.yaml += false_secrets /vendor/**"
  fi

  echo "=== Tag tree ready ==="
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# Mode: --add-git-install / --add-pub-install
# ═══════════════════════════════════════════════════════════════════
# Append install snippets to existing GitHub Release notes.

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


# ═══════════════════════════════════════════════════════════════════
# Guard: reject unknown modes
# ═══════════════════════════════════════════════════════════════════

case "$MODE" in
  --stamp|--stamp-tag|--github-notes|--add-git-install|--add-pub-install) ;;
  --*)
    echo "Unknown mode: $MODE"
    echo "Valid: (none), --stamp-tag, --github-notes, --add-git-install, --add-pub-install"
    exit 1
    ;;
esac


# ═══════════════════════════════════════════════════════════════════
# Mode: --github-notes
# ═══════════════════════════════════════════════════════════════════
# Print release notes to stdout (changelog entry + commit list).

if [[ "$MODE" == "--github-notes" ]]; then
  # Pick the right changelog file based on version type.
  if [[ "$VERSION" == *-* ]]; then
    SOURCE_FILE="CHANGELOG.pre.md"
  else
    SOURCE_FILE="CHANGELOG.md"
  fi

  NOTES=$(extract_entry "$SOURCE_FILE" "$VERSION")

  # Find the previous tag for the commit range.
  # Stable releases skip prereleases; prereleases include any prior tag.
  if [[ "$VERSION" == *-* ]]; then
    PREV_TAG=$(git tag --sort=-v:refname | grep '^v' | grep -v "^${TAG}$" | head -1 || true)
  else
    PREV_TAG=$(git tag --sort=-v:refname | grep '^v' | grep -v "^${TAG}$" | grep -v '-' | head -1 || true)
  fi

  if [ -n "$PREV_TAG" ]; then
    COMMITS=$(git log "$PREV_TAG"..HEAD --oneline --no-decorate 2>/dev/null || true)
  else
    COMMITS=$(git log --oneline --no-decorate 2>/dev/null || true)
  fi

  if [ -n "$COMMITS" ]; then
    COUNT=$(echo "$COMMITS" | grep -c . || true)
    COMMIT_LIST=$(echo "$COMMITS" | sed 's/^/- /')
    printf -v NOTES '%s\n\n<details><summary>Commits since %s (%s)</summary>\n\n%s\n\n</details>' \
      "$NOTES" "${PREV_TAG:-initial}" "$COUNT" "$COMMIT_LIST"
  fi

  echo "$NOTES"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# Mode: (default) — Stamp for pub.dev publish
# ═══════════════════════════════════════════════════════════════════

echo "=== Stamping $TAG (version: $VERSION) ==="

# ── 1. Version files ────────────────────────────────────────────

stamp_version

# ── 2. Build filtered CHANGELOG.md for pub.dev ──────────────────
#
# Only include versions actually published on pub.dev + the current
# version. Unpublished intermediate versions get merged into a
# collapsible under the next published version. Append a commit
# list since the last published version of the same type.

if [[ "$VERSION" == *-* ]]; then
  SOURCE_CL="CHANGELOG.pre.md"
else
  SOURCE_CL="CHANGELOG.md"
fi

PUBLISHED=$(get_published_versions)
echo "  pub.dev has: $(echo "$PUBLISHED" | wc -l | tr -d ' ') published versions"

CL_VERSIONS=$(get_changelog_versions "$SOURCE_CL")

{
  # Copy the header (everything before the first ## entry).
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
      echo "## $V"
      echo ""
      echo "$ENTRY"

      # Flush any buffered unpublished versions as a collapsible.
      if [ -n "$UNPUBLISHED_BUFFER" ]; then
        echo ""
        echo "<details><summary>Also includes unpublished changes</summary>"
        echo ""
        echo "$UNPUBLISHED_BUFFER"
        echo "</details>"
      fi
      UNPUBLISHED_BUFFER=""

      if [[ "$IS_CURRENT" != "true" ]]; then
        PREV_PUBLISHED="$V"
      fi

      echo ""
    else
      # Buffer unpublished version content.
      if [ -n "$UNPUBLISHED_BUFFER" ]; then
        UNPUBLISHED_BUFFER="$UNPUBLISHED_BUFFER"$'\n\n'"### $V"$'\n'"$ENTRY"
      else
        UNPUBLISHED_BUFFER="### $V"$'\n'"$ENTRY"
      fi
    fi
  done
} > /tmp/_changelog_filtered.md

# Determine commit range for pub.dev.
# Stable → previous published stable. Prerelease → previous published prerelease.
PUB_PREV_TAG=""
if [ -n "$PREV_PUBLISHED" ] && git rev-parse "v$PREV_PUBLISHED" &>/dev/null; then
  PUB_PREV_TAG="v$PREV_PUBLISHED"
elif [ -n "$PUBLISHED" ]; then
  if [[ "$VERSION" == *-* ]]; then
    LATEST_PUBLISHED=$(echo "$PUBLISHED" | grep '-' | head -1 || true)
  else
    LATEST_PUBLISHED=$(echo "$PUBLISHED" | grep -v '-' | head -1 || true)
  fi
  if [ -n "${LATEST_PUBLISHED:-}" ] && git rev-parse "v$LATEST_PUBLISHED" &>/dev/null; then
    PUB_PREV_TAG="v$LATEST_PUBLISHED"
  fi
fi

if [ -n "$PUB_PREV_TAG" ]; then
  COMMITS=$(git log "$PUB_PREV_TAG".."$TAG" --oneline --no-decorate 2>/dev/null || true)
else
  COMMITS=$(git log "$TAG" --oneline --no-decorate 2>/dev/null || true)
fi

if [ -n "$COMMITS" ]; then
  COUNT=$(echo "$COMMITS" | grep -c . || true)
  COMMIT_LIST=$(echo "$COMMITS" | sed 's/^/- /')
  printf '<details><summary>Commits since %s (%s)</summary>\n\n%s\n\n</details>\n' \
    "${PUB_PREV_TAG:-initial}" "$COUNT" "$COMMIT_LIST" \
    >> /tmp/_changelog_filtered.md
fi

cp /tmp/_changelog_filtered.md CHANGELOG.md
rm -f /tmp/_changelog_filtered.md
echo "  CHANGELOG.md built (filtered for pub.dev)"

# ── 3. Asset hashes from GitHub Release API ─────────────────────

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
