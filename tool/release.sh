#!/usr/bin/env bash
# ════════════════════════════════════════════════════════════════════
# release.sh — all release logic in one file.
#
# The CI workflow only does job orchestration (checkout, compile,
# upload, publish); every decision and mutation lives here.
#
# Seven modes
# ───────────
#   --gate               Check if this push should trigger a release.
#   --discover           Find the version, create the stamped tag and
#                        the GitHub Release.
#   --github-notes       Print GitHub Release notes to stdout.
#   --update-tag-hashes  Stamp asset hashes into the tag and update it.
#   --stamp-changelog    Build the filtered CHANGELOG.md for the pub.dev
#                        tarball.
#   --add-git-install    Append the git-install snippet to release notes.
#   --add-pub-install    Append the pub.dev-install snippet to notes.
#
# (Tree stamping — version bump, submodule de-registration, vendor
# source — happens inside --discover; it is not a separate mode.)
#
# Pipeline flow (matches the workflow jobs)
# ─────────────────────────────────────────
#   1. gate      → --gate            should this push trigger anything?
#   2. discover  → --discover        find version, stamp tag, create release
#   3. compile   → (workflow)        checkout tag, build per platform
#   4. upload    → (workflow)        upload binaries
#                  --add-git-install
#                  --update-tag-hashes
#   5. publish   → --stamp-changelog
#                  dart pub publish
#                  --add-pub-install
#
# After step 4 the tag carries: stamped version + raw vendor source +
# asset hashes. Anyone using `git: ref: <tag>` in their pubspec gets a
# working build with verified binaries.
#
# Idempotency: every mode is safe to rerun. Existing releases are
# skipped, duplicate snippets are detected, unchanged hashes do not
# create new commits.
#
# Requires : gh CLI authenticated (GH_TOKEN or `gh auth login`)
# Run from : the package root
# ════════════════════════════════════════════════════════════════════

set -euo pipefail


# ════════════════════════════════════════════════════════════════════
# § 1 — Arguments and globals
# ════════════════════════════════════════════════════════════════════

MODE="${1:---help}"
TAG="${2:-}"
VERSION="${TAG:+${TAG#v}}"

REPO="${GITHUB_REPOSITORY:-whuppi/pdf_manipulator}"
REPO_URL="https://github.com/$REPO"
PKG_NAME="pdf_manipulator"


usage() {
  cat <<EOF
Usage: $0 <mode> [tag]

Modes:
  --gate                    Check if a push should trigger a release.
                            Env: BRANCH (optionally BEFORE, AFTER).
                            Outputs: should_run, version.

  --discover                Find the version and create the release.
                            Env: BRANCH.
                            Outputs: tag, version, has_release.

  --github-notes TAG        Print GitHub Release notes to stdout.
  --update-tag-hashes TAG   Stamp asset hashes into the tag (post-upload).
  --stamp-changelog TAG     Build the filtered CHANGELOG.md for pub.dev.
  --add-git-install TAG     Append the git-install snippet to the notes.
  --add-pub-install TAG     Append the pub.dev-install snippet to the notes.

Tags must start with 'v' (e.g. v1.2.3).
EOF
}

# Modes that operate on a specific release require a valid tag.
require_tag() {
  if [ -z "$TAG" ]; then
    echo "Error: $MODE requires a tag argument (e.g. $0 $MODE v1.0.0)" >&2
    exit 1
  fi
  if [[ "$TAG" != v* ]]; then
    echo "Error: tag must start with 'v' (got '$TAG')" >&2
    exit 1
  fi
}


# ════════════════════════════════════════════════════════════════════
# § 2 — Generic helpers
# ════════════════════════════════════════════════════════════════════

# Write a key=value pair to $GITHUB_OUTPUT (and echo it for the log).
gh_output() {
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "$1=$2" >> "$GITHUB_OUTPUT"
  fi
  echo "  output: $1=$2"
}

# Configure the git identity used for CI commits.
git_ci_identity() {
  git config user.name  "github-actions[bot]"
  git config user.email "github-actions[bot]@users.noreply.github.com"
}

# Extract one version's entry (body only, heading excluded) from a
# changelog file. The version is regex-escaped before matching.
extract_entry() {
  local file="$1" version="$2"
  local escaped
  escaped=$(printf '%s' "$version" | sed 's/[][\\.^$*+?(){}|]/\\&/g')
  awk "/^## ${escaped}($| )/{found=1; next} /^## /{found=0} found" "$file"
}

# List every "## version" heading in a changelog file, one per line.
get_changelog_versions() {
  sed -n 's/^## \([^ ]*\).*/\1/p' "$1" 2>/dev/null || true
}

# Choose the changelog source file based on the version type.
# Prerelease versions (containing "-") live in CHANGELOG.pre.md.
pick_source_file() {
  local ver="$1"
  if [[ "$ver" == *-* ]]; then
    echo "CHANGELOG.pre.md"
  else
    echo "CHANGELOG.md"
  fi
}

# Fetch every published version of the package from pub.dev.
get_published_versions() {
  curl -sS "https://pub.dev/api/packages/$PKG_NAME" 2>/dev/null \
    | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    for v in data.get('versions', []):
        print(v['version'])
except Exception:
    pass
" 2>/dev/null || true
}

# Stamp a version string into pubspec.yaml and lib/src/version.dart.
stamp_version() {
  local ver="$1"
  sed -i.bak "s/^version: .*/version: $ver/" pubspec.yaml && rm -f pubspec.yaml.bak
  echo "  pubspec.yaml → $ver"
  sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$ver'/" \
    lib/src/version.dart && rm -f lib/src/version.dart.bak
  echo "  version.dart → $ver"
}

# Generate lib/src/hook/asset_hashes.dart from the GitHub Release API
# digests. Returns 0 if the file was written, 1 if there was nothing to
# stamp (so callers can short-circuit with `|| ...`).
stamp_asset_hashes() {
  local tag="$1"
  local hash_file="lib/src/hook/asset_hashes.dart"

  if ! command -v gh &>/dev/null; then
    echo "  ⚠ gh CLI not found — skipping asset hashes"
    return 1
  fi
  if ! gh api "repos/$REPO/releases/tags/$tag" --silent 2>/dev/null; then
    echo "  ⚠ No GitHub Release for $tag — skipping asset hashes"
    return 1
  fi

  # WASM assets are excluded — they're loaded via JS, not downloaded by
  # the Dart build hook. Only native binaries need hash verification.
  local assets
  assets=$(gh api "repos/$REPO/releases/tags/$tag" \
    --jq '.assets[]
          | select(.digest != null
                   and (.name | startswith("wasm-")  | not)
                   and (.name | startswith("Source") | not))
          | "\(.name)\t\(.digest)"')

  if [ -z "$assets" ]; then
    echo "  ⚠ No assets with digests — skipping asset hashes"
    return 1
  fi

  {
    echo "// Hashes of pre-built native binaries downloaded by the build hook."
    echo "// Generated by tool/release.sh from GitHub Release API digests."
    echo "//"
    echo "// The build hook verifies downloaded files against these hashes."
    echo "// Hash mismatch → re-download. Correct → use cached. Missing → download."
    echo ""
    echo "/// SHA-256 hashes for pre-built native binaries, verified by the build hook."
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


# ════════════════════════════════════════════════════════════════════
# § 3 — Changelog builders
#
# Two public builders:
#   build_github_notes     TAG → stdout: the entry + a commits collapsible
#   build_pubdev_changelog TAG → stdout: the full, filtered changelog
#
# ── pub.dev changelog algorithm ───────────────────────────────────
#
# Step 0 — Pick the source file.
#   Stable (no "-") → CHANGELOG.md.  Pre (has "-") → CHANGELOG.pre.md.
#
# Step 1 — Classify each "## heading" as YES or NO.
#   YES = it's already on pub.dev, OR it's the version being deployed.
#   NO  = anything else (unpublished intermediate work).
#
#   YES → gets its own ## section in the output.
#   NO  → folded into a collapsible under the nearest YES above it.
#
# Pass 1 — Top to bottom, build the sections.
#   YES versions get ## headings. Consecutive NO versions buffer into
#   the previous YES version's "unpublished changes" collapsible.
#
# Pass 2 — Bottom to top, add the commit lists.
#   Each YES version gets a <details>Commits since vPREV</details>,
#   where PREV is the YES version directly below it. The bottom-most
#   YES version uses "Commits since initial".
#
# ── Commit-range rule (same type only) ────────────────────────────
#
# Stable tags diff only against previous stable tags; prerelease tags
# diff only against previous prerelease tags. This applies to both the
# GitHub notes and the pub.dev changelog.
# ════════════════════════════════════════════════════════════════════

# Render a <details> block listing commits in (from, to]. Prints
# nothing when the range is empty.
commits_collapsible() {
  local from="$1" to="$2"

  # During --discover the tag doesn't exist yet (it's created after
  # the notes are generated). Fall back to HEAD in that case.
  if ! git rev-parse "$to" &>/dev/null; then
    to="HEAD"
  fi

  local commits
  if [ -n "$from" ]; then
    commits=$(git log "$from".."$to" --oneline --no-decorate 2>/dev/null || true)
  else
    commits=$(git log "$to" --oneline --no-decorate 2>/dev/null || true)
  fi
  [ -z "$commits" ] && return

  local count list
  count=$(echo "$commits" | grep -c . || true)
  list=$(echo "$commits" | sed 's/^/- /')
  printf '<details><summary>Commits since %s (%s)</summary>\n\n%s\n\n</details>\n' \
    "${from:-initial}" "$count" "$list"
}

# Find the previous tag of the same release type (stable vs prerelease)
# that precedes `current` in version order. Prints nothing if none.
prev_same_type_tag() {
  local current="$1"
  local ver="${current#v}"
  local tags
  tags=$(git tag --sort=version:refname 2>/dev/null) || true
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

  extract_entry "$source_file" "$ver"

  local prev_tag csection
  prev_tag=$(prev_same_type_tag "$tag")
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

  local published versions
  published=$(get_published_versions)
  versions=$(get_changelog_versions "$source_file")

  # Is this version published, or the one being deployed right now?
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

  # Fold the buffered NO-version entries into the current YES section.
  # Reads $unpub_buf / $cur_yes / $workdir from the enclosing scope.
  _flush_unpub() {
    [ -n "$unpub_buf" ] && [ -n "$cur_yes" ] || return 0
    {
      echo ""
      echo "<details><summary>Also includes unpublished changes</summary>"
      echo ""
      echo "$unpub_buf"
      echo ""
      echo "</details>"
    } >> "$workdir/$cur_yes"
  }

  # Collect the YES versions in document order (newest first).
  local -a yes_list=()
  local v
  while IFS= read -r v; do
    [ -z "$v" ] && continue
    _is_yes "$v" && yes_list+=("$v")
  done <<< "$versions"

  # ── Pass 1 — write one file per version section ──
  local workdir
  workdir=$(mktemp -d)

  local unpub_buf="" cur_yes="" entry
  while IFS= read -r v; do
    [ -z "$v" ] && continue
    entry=$(extract_entry "$source_file" "$v")
    if _is_yes "$v"; then
      _flush_unpub                # attach pending NOs to the previous YES
      unpub_buf=""
      cur_yes="$v"
      { echo "## $v"; echo ""; echo "$entry"; } > "$workdir/$v"
    elif [ -n "$unpub_buf" ]; then
      unpub_buf+=$'\n\n'"### $v"$'\n'"$entry"
    else
      unpub_buf="### $v"$'\n'"$entry"
    fi
  done <<< "$versions"
  _flush_unpub                    # attach any trailing NOs to the last YES

  # ── Pass 2 — bottom to top, append commit lists ──
  local prev_yes_tag="" i yv ytag csection
  for ((i = ${#yes_list[@]} - 1; i >= 0; i--)); do
    yv="${yes_list[$i]}"
    ytag="v$yv"
    csection=$(commits_collapsible "$prev_yes_tag" "$ytag")
    if [ -n "$csection" ]; then
      { echo ""; echo "$csection"; } >> "$workdir/$yv"
    fi
    prev_yes_tag="$ytag"
  done

  # ── Assemble: preamble, then each YES section newest-first ──
  awk '/^## /{exit} {print}' "$source_file"
  for yv in "${yes_list[@]}"; do
    cat "$workdir/$yv"
    echo ""
  done

  rm -rf "$workdir"
}


# ════════════════════════════════════════════════════════════════════
# § 4 — Mode: --gate
#
# Decide whether the right changelog file changed in this push: dev
# reacts to CHANGELOG.pre.md, prod to CHANGELOG.md. The diff spans the
# whole push range (not head_commit.modified, which misses force-pushes
# and ff-merges). If the diff can't be computed — a new branch, an
# orphaned before-SHA, or a workflow_dispatch with an empty BEFORE — we
# assume the file changed: a wasted run is cheap, a missed release is not.
#
# Env     : BRANCH (optionally BEFORE, AFTER)
# Outputs : should_run, version
# ════════════════════════════════════════════════════════════════════

cmd_gate() {
  local branch="${BRANCH:?--gate requires BRANCH env var}"
  local target_file
  if [[ "$branch" == "prod" ]]; then
    target_file="CHANGELOG.md"
  else
    target_file="CHANGELOG.pre.md"
  fi

  local changed_files
  if ! changed_files=$(git diff --name-only "${BEFORE:-}" "${AFTER:-HEAD}" 2>/dev/null); then
    echo "Gate: git diff failed (force-push, new branch, or workflow_dispatch) — assuming $target_file changed"
    changed_files="$target_file"
  fi

  if grep -Fqx "$target_file" <<< "$changed_files"; then
    local found_version
    found_version=$(get_changelog_versions "$target_file" | head -1 || true)
    gh_output "should_run" "true"
    gh_output "version" "${found_version:-unknown}"
    echo "Gate: $target_file changed, version=$found_version"
  else
    gh_output "should_run" "false"
    gh_output "version" ""
    echo "Gate: $target_file not in changeset, skipping"
  fi
}


# ════════════════════════════════════════════════════════════════════
# § 5 — Mode: --discover
#
# Read the changelog, take the latest version, validate that its type
# matches the branch, build a stamped tag commit (version stamped,
# submodules de-registered, vendor source left raw), push it to a
# staging branch, and create the GitHub Release from it. Idempotent:
# skips if the release already exists.
#
# Env     : BRANCH
# Outputs : tag, version, has_release
# ════════════════════════════════════════════════════════════════════

cmd_discover() {
  local branch="${BRANCH:?--discover requires BRANCH env var}"
  local file
  if [[ "$branch" == "prod" ]]; then
    file="CHANGELOG.md"
  else
    file="CHANGELOG.pre.md"
  fi

  if [ ! -f "$file" ]; then
    gh_output "has_release" "false"
    echo "No $file found"
    return 0
  fi

  local version
  version=$(get_changelog_versions "$file" | head -1 || true)
  if [ -z "$version" ]; then
    gh_output "has_release" "false"
    echo "No version heading in $file"
    return 0
  fi

  local tag="v$version"
  local is_pre=false
  [[ "$version" == *-* ]] && is_pre=true

  if [[ "$branch" == "prod" && "$is_pre" == "true" ]]; then
    echo "Skipping prerelease $version on prod."
    gh_output "has_release" "false"
    return 0
  fi
  if [[ "$branch" == "dev" && "$is_pre" == "false" ]]; then
    echo "Skipping stable $version on dev."
    gh_output "has_release" "false"
    return 0
  fi

  # Idempotent: skip if the release already exists.
  if gh release view "$tag" --repo "$REPO" --json tagName >/dev/null 2>&1; then
    echo "Release $tag already exists."
    gh_output "tag" "$tag"
    gh_output "version" "$version"
    gh_output "has_release" "true"
    return 0
  fi

  # ── Stamp the tree: version + de-register submodules + false_secrets ──
  echo "=== Stamping tag tree for $tag ==="
  stamp_version "$version"

  local sub
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

  if ! grep -q '^  - /vendor/\*\*$' pubspec.yaml; then
    sed -i.bak '/^false_secrets:/a\  - /vendor/**' pubspec.yaml && rm -f pubspec.yaml.bak
    echo "  pubspec.yaml += false_secrets /vendor/**"
  fi

  # ── Commit, push to staging, create the GitHub Release ──
  git_ci_identity
  git add -A
  if git diff --cached --quiet; then
    echo "  Tree already stamped — skipping commit"
  else
    git commit -m "release: $tag"
  fi

  local stamped_sha
  stamped_sha=$(git rev-parse HEAD)
  echo "  Stamped commit: $stamped_sha"

  local staging_branch="_release-staging-$version"
  git push origin --delete "refs/heads/$staging_branch" 2>/dev/null || true
  git push origin "$stamped_sha:refs/heads/$staging_branch"
  trap 'git push origin --delete "refs/heads/'"$staging_branch"'" 2>/dev/null || true' EXIT

  local notes_file
  notes_file=$(mktemp)
  build_github_notes "$tag" > "$notes_file"

  local -a flags=()
  [[ "$is_pre" == "true" ]] && flags+=(--prerelease)
  gh release create "$tag" \
    --repo "$REPO" \
    --target "$stamped_sha" \
    --title "$PKG_NAME $version" \
    --notes-file "$notes_file" \
    "${flags[@]}"
  rm -f "$notes_file"
  echo "  Created release $tag at $stamped_sha"

  # Staging branch no longer needed — the tag keeps the commit alive.
  trap - EXIT
  git push origin --delete "refs/heads/$staging_branch" 2>/dev/null || true

  gh_output "tag" "$tag"
  gh_output "version" "$version"
  gh_output "has_release" "true"
}


# ════════════════════════════════════════════════════════════════════
# § 6 — Mode: --github-notes
#
# Print GitHub Release notes to stdout: the changelog entry for this
# version plus a collapsible commit list since the previous tag of the
# same type (stable ↔ stable, pre ↔ pre).
# ════════════════════════════════════════════════════════════════════

cmd_github_notes() {
  require_tag
  build_github_notes "$TAG"
}


# ════════════════════════════════════════════════════════════════════
# § 7 — Mode: --update-tag-hashes
#
# After the binaries are uploaded, stamp their hashes into the tag so
# `git: ref: <tag>` users get verified downloads. Idempotent: skips if
# the hashes are unchanged.
# ════════════════════════════════════════════════════════════════════

cmd_update_tag_hashes() {
  require_tag
  echo "=== Updating $TAG with asset hashes ==="

  git fetch origin "refs/tags/$TAG:refs/tags/$TAG" 2>/dev/null || true

  stamp_asset_hashes "$TAG" || { echo "No hashes to stamp — tag unchanged."; return 0; }

  git_ci_identity
  git add lib/src/hook/asset_hashes.dart

  if git diff --cached --quiet; then
    echo "  Asset hashes unchanged — tag already up to date"
    return 0
  fi

  git commit -m "stamp: asset hashes for $TAG"
  local new_sha
  new_sha=$(git rev-parse HEAD)
  git tag -f "$TAG" "$new_sha"
  git push origin --force "refs/tags/$TAG"
  echo "  Tag $TAG updated → $new_sha"
}


# ════════════════════════════════════════════════════════════════════
# § 8 — Mode: --stamp-changelog
#
# Build the filtered CHANGELOG.md for the pub.dev tarball. The tag
# already carries everything else (version, vendor source, hashes).
# ════════════════════════════════════════════════════════════════════

cmd_stamp_changelog() {
  require_tag
  echo "=== Building changelog for $TAG ==="
  build_pubdev_changelog "$TAG" > CHANGELOG.md
  echo "  CHANGELOG.md built (filtered for pub.dev)"
}


# ════════════════════════════════════════════════════════════════════
# § 9 — Modes: --add-git-install / --add-pub-install
#
# Append an install snippet to the existing GitHub Release notes.
# Idempotent: skips if the snippet is already present.
# ════════════════════════════════════════════════════════════════════

# Append `section` to a release's notes unless `marker` already appears
# in the body.
append_release_note() {
  local tag="$1" marker="$2" section="$3"
  local existing
  existing=$(gh release view "$tag" --repo "$REPO" --json body --jq '.body')

  if grep -qF "$marker" <<< "$existing"; then
    echo "Snippet '$marker' already present — skipping"
    return 0
  fi

  local notes_file
  notes_file=$(mktemp)
  printf '%s\n\n%s\n' "$existing" "$section" > "$notes_file"
  gh release edit "$tag" --repo "$REPO" --notes-file "$notes_file"
  rm -f "$notes_file"
  echo "Added '$marker' to $tag release notes"
}

cmd_add_git_install() {
  require_tag
  local section
  section="---

### Install (git tag)

\`\`\`yaml
dependencies:
  $PKG_NAME:
    git:
      url: $REPO_URL.git
      ref: $TAG
\`\`\`"
  append_release_note "$TAG" "### Install (git tag)" "$section"
}

cmd_add_pub_install() {
  require_tag
  local section
  section="### Install (pub.dev)

\`\`\`yaml
dependencies:
  $PKG_NAME: ^$VERSION
\`\`\`"
  append_release_note "$TAG" "### Install (pub.dev)" "$section"
}


# ════════════════════════════════════════════════════════════════════
# § 10 — Dispatch
# ════════════════════════════════════════════════════════════════════

main() {
  case "$MODE" in
    --help | -h)         usage ;;
    --gate)              cmd_gate ;;
    --discover)          cmd_discover ;;
    --github-notes)      cmd_github_notes ;;
    --update-tag-hashes) cmd_update_tag_hashes ;;
    --stamp-changelog)   cmd_stamp_changelog ;;
    --add-git-install)   cmd_add_git_install ;;
    --add-pub-install)   cmd_add_pub_install ;;
    *)
      echo "Unknown mode: $MODE" >&2
      echo "Run '$0 --help' for usage." >&2
      exit 1
      ;;
  esac
}

main