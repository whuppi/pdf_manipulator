#!/bin/bash
# ────────────────────────────────────────────────────────────────────
# release.sh — All release logic in one file. The workflow is just
# job orchestration (checkout, compile, upload, publish); every
# decision and mutation lives here.
#
# Eight modes:
#
#   --gate               Check if this push should trigger a release.
#   --discover           Find version, create stamped tag + GitHub Release.
#   --github-notes       Print GitHub Release notes to stdout.
#   --update-tag-hashes  Stamp asset hashes into the tag and update it.
#   --stamp-changelog    Build filtered CHANGELOG.md for pub.dev tarball.
#   --add-git-install    Append git install snippet to release notes.
#   --add-pub-install    Append pub.dev install snippet to release notes.
#   --stamp-tag          Prepare tree for the release tag commit (internal).
#
# Pipeline flow (matches workflow jobs):
#   1. gate:      --gate (should this push trigger anything?)
#   2. discover:  --discover (find version, stamp tag, create release)
#   3. compile:   (workflow — checkout tag, build per-platform)
#   4. upload:    (workflow — upload binaries) → --add-git-install → --update-tag-hashes
#   5. publish:   --stamp-changelog → dart pub publish → --add-pub-install
#
# After step 4, the tag has: stamped version + raw vendor source +
# asset hashes. Anyone doing `git: ref: <tag>` in pubspec gets a
# working build with verified binaries.
#
# Idempotency: every mode is safe to rerun. Existing releases are
# skipped, duplicate snippets are detected, unchanged hashes don't
# create new commits.
#
# Requires: gh CLI authenticated (GH_TOKEN or gh auth login)
# Run from: package root
# ────────────────────────────────────────────────────────────────────
set -euo pipefail


# ═══════════════════════════════════════════════════════════════════
# § 1 — Args + globals
# ═══════════════════════════════════════════════════════════════════

MODE="${1:---help}"

if [[ "$MODE" == "--help" || "$MODE" == "-h" ]]; then
  echo "Usage: $0 <mode> [tag]"
  echo ""
  echo "Modes:"
  echo "  --gate               Check if push should trigger release (needs BRANCH, BEFORE, AFTER env)"
  echo "  --discover           Find version + create release (needs BRANCH env)"
  echo "  --github-notes TAG   Print release notes to stdout"
  echo "  --update-tag-hashes TAG  Stamp asset hashes into the tag"
  echo "  --stamp-changelog TAG    Build filtered CHANGELOG.md for pub.dev"
  echo "  --add-git-install TAG    Append git install snippet to release notes"
  echo "  --add-pub-install TAG    Append pub.dev install snippet to release notes"
  exit 0
fi

TAG="${2:-}"
VERSION="${TAG:+${TAG#v}}"
REPO="${GITHUB_REPOSITORY:-chaudharydeepanshu/pdf_manipulator}"
REPO_URL="https://github.com/$REPO"
PKG_NAME="pdf_manipulator"


# ═══════════════════════════════════════════════════════════════════
# § 2 — Helpers
# ═══════════════════════════════════════════════════════════════════

# Write a key=value pair to $GITHUB_OUTPUT (or stdout if not in CI).
gh_output() {
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "$1=$2" >> "$GITHUB_OUTPUT"
  fi
  echo "  output: $1=$2"
}

# Configure git identity for CI commits.
git_ci_identity() {
  git config user.name "github-actions[bot]"
  git config user.email "github-actions[bot]@users.noreply.github.com"
}

# Extract one version's entry from a changelog file.
extract_entry() {
  local file="$1" version="$2"
  awk "/^## ${version//./\\.}($| )/{found=1; next} /^## /{found=0} found" "$file"
}

# List all ## version headings in a changelog file, one per line.
get_changelog_versions() {
  sed -n 's/^## \([^ ]*\).*/\1/p' "$1" 2>/dev/null || true
}

# Pick changelog source file based on version type.
pick_source_file() {
  local ver="$1"
  if [[ "$ver" == *-* ]]; then
    echo "CHANGELOG.pre.md"
  else
    echo "CHANGELOG.md"
  fi
}

# Fetch all published versions from pub.dev.
get_published_versions() {
  curl -sS "https://pub.dev/api/packages/$PKG_NAME" 2>/dev/null \
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

# Stamp version into pubspec.yaml + version.dart.
stamp_version() {
  local ver="$1"
  sed -i.bak "s/^version: .*/version: $ver/" pubspec.yaml && rm -f pubspec.yaml.bak
  echo "  pubspec.yaml → $ver"
  sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$ver'/" \
    lib/src/version.dart && rm -f lib/src/version.dart.bak
  echo "  version.dart → $ver"
}

# Generate asset_hashes.dart from GitHub Release API digests.
# Returns 0 if hashes were written, 1 if skipped.
stamp_asset_hashes() {
  local tag="$1"
  local hash_file="lib/src/hook/asset_hashes.dart"

  if ! command -v gh &>/dev/null; then
    echo "  ⚠ gh CLI not found — skipping asset hashes"; return 1
  fi
  if ! gh api "repos/$REPO/releases/tags/$tag" --silent 2>/dev/null; then
    echo "  ⚠ No GitHub Release for $tag — skipping asset hashes"; return 1
  fi

  local assets
  assets=$(gh api "repos/$REPO/releases/tags/$tag" \
    --jq '.assets[] | select(.digest != null and (.name | startswith("wasm-") | not) and (.name | startswith("Source") | not)) | "\(.name)\t\(.digest)"')

  if [ -z "$assets" ]; then
    echo "  ⚠ No assets with digests — skipping asset hashes"; return 1
  fi

  {
    echo "// Hashes of pre-built native binaries downloaded by the build hook."
    echo "// Generated by tool/release.sh from GitHub Release API digests."
    echo "//"
    echo "// The build hook verifies downloaded files against these hashes."
    echo "// Hash mismatch → re-download. Correct → use cached. Missing → download."
    echo ""
    echo "const Map<String, String> assetHashesSha256 = {"
    echo "$assets" | sort | while IFS=$'\t' read -r name digest; do
      hash="${digest#sha256:}"
      echo "  '$name': '$hash',"
      echo "  $name ... ${hash:0:12}..." >&2
    done
    echo "};"
  } > "$hash_file"

  local count
  count=$(echo "$assets" | grep -c . || true)
  echo "  $hash_file → $count hashes"
}


# ═══════════════════════════════════════════════════════════════════
# § 3 — Changelog builder
#
# Two entry points:
#   build_github_notes TAG   → stdout: entry + commits collapsible
#   build_pubdev_changelog TAG → stdout: full filtered changelog
#
# ── The pub.dev changelog algorithm ────────────────────────────
#
# Step 0 — Pick source file.
#   Stable (no "-") → CHANGELOG.md. Pre (has "-") → CHANGELOG.pre.md.
#
# Step 1 — YES/NO classification.
#   For each ## heading in the source file: is it on pub.dev OR is
#   it the one being deployed right now?
#     YES → gets its own ## section in the output.
#     NO  → folded into a collapsible under the nearest YES above it.
#
# Pass 1 — Top to bottom, build sections.
#   YES versions get ## headings. NO versions buffer into the
#   previous YES version's collapsible. Multiple consecutive NOs
#   stack in the same collapsible.
#
# Pass 2 — Bottom to top, add commit lists.
#   Each YES version gets <details>Commits since vPREV</details>
#   where PREV is the YES version directly below it. The bottom-most
#   YES version gets "Commits since initial".
#
# ── Commit range rule (same type only) ─────────────────────────
#
# Stable tags only diff against previous stable tags.
# Prerelease tags only diff against previous prerelease tags.
# Applies to BOTH github-notes and pub.dev changelog.
# ═══════════════════════════════════════════════════════════════════

commits_collapsible() {
  local from="$1" to="$2"
  local commits
  if [ -n "$from" ]; then
    commits=$(git log "$from".."$to" --oneline --no-decorate 2>/dev/null || true)
  else
    commits=$(git log "$to" --oneline --no-decorate 2>/dev/null || true)
  fi
  [ -z "$commits" ] && return
  local count
  count=$(echo "$commits" | grep -c . || true)
  local list
  list=$(echo "$commits" | sed 's/^/- /')
  printf '<details><summary>Commits since %s (%s)</summary>\n\n%s\n\n</details>\n' \
    "${from:-initial}" "$count" "$list"
}

prev_same_type_tag() {
  local current="$1"
  local ver="${current#v}"
  local tags
  tags=$(git tag --sort=v:refname 2>/dev/null) || true
  [ -z "$tags" ] && return
  local result="" tag
  for tag in $tags; do
    [[ "$tag" == "$current" ]] && break
    [[ "$tag" != v* ]] && continue
    if [[ "$ver" == *-* ]]; then
      [[ "$tag" == *-* ]] && result="$tag"
    else
      [[ "$tag" != *-* ]] && result="$tag"
    fi
  done
  echo "$result"
}

build_github_notes() {
  local tag="$1"
  local ver="${tag#v}"
  local source_file
  source_file=$(pick_source_file "$ver")
  local notes
  notes=$(extract_entry "$source_file" "$ver")
  local prev_tag
  prev_tag=$(prev_same_type_tag "$tag")
  echo "$notes"
  local csection
  csection=$(commits_collapsible "$prev_tag" "$tag")
  if [ -n "$csection" ]; then
    echo ""
    echo "$csection"
  fi
}

build_pubdev_changelog() {
  local tag="$1"
  local ver="${tag#v}"
  local source_file
  source_file=$(pick_source_file "$ver")
  local published
  published=$(get_published_versions)
  local versions
  versions=$(get_changelog_versions "$source_file")

  _is_yes() {
    [[ "$1" == "$ver" ]] && return 0
    if [ -n "$published" ]; then
      local pv
      while IFS= read -r pv; do
        [[ "$pv" == "$1" ]] && return 0
      done <<< "$published"
    fi
    return 1
  }

  local -a yes_list=()
  local v
  while IFS= read -r v; do
    [ -z "$v" ] && continue
    _is_yes "$v" && yes_list+=("$v")
  done <<< "$versions"

  # Pass 1 — per-version section files
  local workdir
  workdir=$(mktemp -d)
  local unpub_buf="" cur_yes=""
  while IFS= read -r v; do
    [ -z "$v" ] && continue
    local entry
    entry=$(extract_entry "$source_file" "$v")
    if _is_yes "$v"; then
      if [ -n "$unpub_buf" ] && [ -n "$cur_yes" ]; then
        {
          echo ""
          echo "<details><summary>Also includes unpublished changes</summary>"
          echo ""
          echo "$unpub_buf"
          echo ""
          echo "</details>"
        } >> "$workdir/$cur_yes"
      fi
      unpub_buf=""
      cur_yes="$v"
      { echo "## $v"; echo ""; echo "$entry"; } > "$workdir/$v"
    else
      if [ -n "$unpub_buf" ]; then
        unpub_buf+=$'\n\n'"### $v"$'\n'"$entry"
      else
        unpub_buf="### $v"$'\n'"$entry"
      fi
    fi
  done <<< "$versions"

  if [ -n "$unpub_buf" ] && [ -n "$cur_yes" ]; then
    {
      echo ""
      echo "<details><summary>Also includes unpublished changes</summary>"
      echo ""
      echo "$unpub_buf"
      echo ""
      echo "</details>"
    } >> "$workdir/$cur_yes"
  fi

  # Pass 2 — bottom to top, commit lists
  local prev_yes_tag=""
  local i
  for ((i=${#yes_list[@]}-1; i>=0; i--)); do
    local yv="${yes_list[$i]}"
    local ytag="v$yv"
    local csection
    csection=$(commits_collapsible "$prev_yes_tag" "$ytag")
    if [ -n "$csection" ]; then
      { echo ""; echo "$csection"; } >> "$workdir/$yv"
    fi
    prev_yes_tag="$ytag"
  done

  # Assemble
  awk '/^## /{exit} {print}' "$source_file"
  for yv in "${yes_list[@]}"; do
    cat "$workdir/$yv"
    echo ""
  done
  rm -rf "$workdir"
}


# ═══════════════════════════════════════════════════════════════════
# § 4 — Mode: --gate
#
# Checks if the right changelog file changed in this push.
# dev only reacts to CHANGELOG.pre.md, prod to CHANGELOG.md.
# Uses git diff across the full push range (not head_commit.modified,
# which misses force-pushes and ff-merges). Falls back to
# should_run=true when diff fails (new branch, orphaned before-SHA,
# workflow_dispatch where BEFORE is empty) — a wasted run is cheap;
# a missed release is not.
#
# Env: BRANCH, BEFORE, AFTER
# Outputs: should_run, version (via $GITHUB_OUTPUT)
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--gate" ]]; then
  BRANCH="${BRANCH:?--gate requires BRANCH env var}"

  if [[ "$BRANCH" == "prod" ]]; then
    TARGET_FILE="CHANGELOG.md"
  else
    TARGET_FILE="CHANGELOG.pre.md"
  fi

  CHANGED_FILES=$(git diff --name-only "${BEFORE:-}" "${AFTER:-HEAD}" 2>/dev/null || echo "$TARGET_FILE")

  if echo "$CHANGED_FILES" | grep -qx "$TARGET_FILE"; then
    FOUND_VERSION=$(sed -n 's/^## \([^ ]*\).*/\1/p' "$TARGET_FILE" 2>/dev/null | head -1 || true)
    gh_output "should_run" "true"
    gh_output "version" "${FOUND_VERSION:-unknown}"
    echo "Gate: $TARGET_FILE changed, version=$FOUND_VERSION"
  else
    gh_output "should_run" "false"
    gh_output "version" ""
    echo "Gate: $TARGET_FILE not in changeset, skipping"
  fi
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# § 5 — Mode: --discover
#
# Reads the changelog, extracts the latest version, validates
# branch/type match, creates a stamped tag commit (version stamped,
# submodules deregistered, vendor source raw), pushes it, creates
# a GitHub Release with notes. Idempotent: skips if the release
# already exists.
#
# Env: BRANCH
# Outputs: tag, version, has_release (via $GITHUB_OUTPUT)
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--discover" ]]; then
  BRANCH="${BRANCH:?--discover requires BRANCH env var}"

  if [[ "$BRANCH" == "prod" ]]; then
    FILE="CHANGELOG.md"
  else
    FILE="CHANGELOG.pre.md"
  fi

  if [ ! -f "$FILE" ]; then
    gh_output "has_release" "false"
    echo "No $FILE found"
    exit 0
  fi

  VERSION=$(sed -n 's/^## \([^ ]*\).*/\1/p' "$FILE" 2>/dev/null | head -1 || true)
  if [ -z "$VERSION" ]; then
    gh_output "has_release" "false"
    echo "No version heading in $FILE"
    exit 0
  fi

  TAG="v$VERSION"
  IS_PRE=false
  [[ "$VERSION" == *-* ]] && IS_PRE=true

  if [[ "$BRANCH" == "prod" && "$IS_PRE" == "true" ]]; then
    echo "Skipping prerelease $VERSION on prod."
    gh_output "has_release" "false"
    exit 0
  fi
  if [[ "$BRANCH" == "dev" && "$IS_PRE" == "false" ]]; then
    echo "Skipping stable $VERSION on dev."
    gh_output "has_release" "false"
    exit 0
  fi

  # Idempotent: skip if release already exists
  if gh release view "$TAG" --repo "$REPO" --json tagName >/dev/null 2>&1; then
    echo "Release $TAG already exists."
    gh_output "tag" "$TAG"
    gh_output "version" "$VERSION"
    gh_output "has_release" "true"
    exit 0
  fi

  # Stamp the tree: version + deregister submodules + false_secrets
  echo "=== Stamping tag tree for $TAG ==="
  stamp_version "$VERSION"

  for sub in vendor/pdf_oxide vendor/office_oxide; do
    if [ -d "$sub/.git" ] || [ -f "$sub/.git" ]; then
      git rm --cached "$sub" 2>/dev/null || true
      rm -rf "$sub/.git"
      git add --force "$sub/"
      echo "  $sub → raw source (de-registered submodule)"
    fi
  done

  if [ -f .gitmodules ]; then
    git rm --cached .gitmodules 2>/dev/null || true
    rm -f .gitmodules
    echo "  .gitmodules removed"
  fi

  if ! grep -q '/vendor/\*\*' pubspec.yaml; then
    sed -i.bak '/^false_secrets:/a\  - /vendor/**' pubspec.yaml && rm -f pubspec.yaml.bak
    echo "  pubspec.yaml += false_secrets /vendor/**"
  fi

  # Commit, push, create release
  git_ci_identity
  git add -A
  git commit -m "release: $TAG"
  STAMPED_SHA=$(git rev-parse HEAD)
  echo "  Stamped commit: $STAMPED_SHA"

  git push origin --force "$STAMPED_SHA:refs/heads/_release-staging"

  NOTES=$(build_github_notes "$TAG")
  FLAGS=""
  [[ "$IS_PRE" == "true" ]] && FLAGS="--prerelease"
  gh release create "$TAG" \
    --repo "$REPO" \
    --target "$STAMPED_SHA" \
    --title "$PKG_NAME $VERSION" \
    --notes "$NOTES" \
    $FLAGS
  echo "  Created release $TAG at $STAMPED_SHA"

  git push origin --delete refs/heads/_release-staging 2>/dev/null || true

  gh_output "tag" "$TAG"
  gh_output "version" "$VERSION"
  gh_output "has_release" "true"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# § 6 — Mode: --github-notes
#
# Print GitHub Release notes to stdout: the changelog entry for this
# version + a collapsible commit list since the previous tag of the
# same type (stable↔stable, pre↔pre).
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--github-notes" ]]; then
  build_github_notes "$TAG"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# § 7 — Mode: --update-tag-hashes
#
# After binaries are uploaded, stamp asset hashes into the tag so
# `git: ref: <tag>` users get verified binary downloads. Idempotent:
# skips if hashes are unchanged.
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--update-tag-hashes" ]]; then
  echo "=== Updating $TAG with asset hashes ==="
  stamp_asset_hashes "$TAG" || { echo "No hashes to stamp — tag unchanged."; exit 0; }

  git_ci_identity
  git add lib/src/hook/asset_hashes.dart

  if git diff --cached --quiet; then
    echo "  Asset hashes unchanged — tag already up to date"
    exit 0
  fi

  git commit -m "stamp: asset hashes for $TAG"
  NEW_SHA=$(git rev-parse HEAD)
  git tag -f "$TAG" "$NEW_SHA"
  git push origin --force "refs/tags/$TAG"
  echo "  Tag $TAG updated → $NEW_SHA"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# § 8 — Mode: --stamp-changelog
#
# Build the filtered CHANGELOG.md for the pub.dev tarball. The tag
# already has everything else (version, vendor source, asset hashes).
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--stamp-changelog" ]]; then
  echo "=== Building changelog for $TAG ==="
  build_pubdev_changelog "$TAG" > CHANGELOG.md
  echo "  CHANGELOG.md built (filtered for pub.dev)"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# § 9 — Mode: --add-git-install / --add-pub-install
#
# Append install snippets to existing GitHub Release notes.
# Idempotent: skips if snippet already present.
# ═══════════════════════════════════════════════════════════════════

if [[ "$MODE" == "--add-git-install" ]]; then
  EXISTING=$(gh release view "$TAG" --repo "$REPO" --json body --jq '.body')
  if echo "$EXISTING" | grep -qF "### Install (git tag)"; then
    echo "Git install snippet already present — skipping"
    exit 0
  fi
  SNIPPET="---

### Install (git tag)

\`\`\`yaml
dependencies:
  $PKG_NAME:
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
  if echo "$EXISTING" | grep -qF "### Install (pub.dev)"; then
    echo "Pub.dev install snippet already present — skipping"
    exit 0
  fi
  SNIPPET="### Install (pub.dev)

\`\`\`yaml
dependencies:
  $PKG_NAME: ^$VERSION
\`\`\`"
  gh release edit "$TAG" --repo "$REPO" --notes "$EXISTING"$'\n\n'"$SNIPPET"
  echo "Added pub.dev install snippet to $TAG release notes"
  exit 0
fi


# ═══════════════════════════════════════════════════════════════════
# Unknown mode
# ═══════════════════════════════════════════════════════════════════

echo "Unknown mode: $MODE"
echo "Run '$0 --help' for usage."
exit 1
