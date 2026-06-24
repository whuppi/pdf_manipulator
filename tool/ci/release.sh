#!/usr/bin/env bash
# ════════════════════════════════════════════════════════════════════
# release.sh — all release logic in one file.
#
# The CI workflow only does job orchestration (checkout, compile,
# upload, publish); every decision and mutation lives here.
#
# Nine modes
# ───────────
#   --gate               Check if this push should trigger a release.
#   --discover           Find the version, create the stamped tag and
#                        the GitHub Release.
#   --check-versions     Fail if the changelog adds >1 unreleased version.
#   --github-notes       Print GitHub Release notes to stdout.
#   --update-tag-hashes  Stamp asset hashes into the tag and update it.
#   --stamp-changelog    Build the filtered CHANGELOG.md for the pub.dev
#                        tarball.
#   --stamp-readme       Flatten the README <picture> banner to a single
#                        <img> for the pub.dev tarball.
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
#   3. compile   → (workflow)        checkout tag, build per target
#   4. upload    → (workflow)        upload binaries
#                  --add-git-install
#                  --update-tag-hashes
#   5. publish   → --stamp-changelog
#                  --stamp-readme
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

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ════════════════════════════════════════════════════════════════════
# § 1 — Arguments and globals
# ════════════════════════════════════════════════════════════════════

MODE="${1:---help}"
TAG="${2:-}"
VERSION="${TAG:+${TAG#v}}"

# lib.sh is shared with the build scripts in tool/ — one level up from tool/ci/.
source "$SCRIPT_DIR/../lib.sh"
ensure_jq  # this script parses pub.dev + build.json JSON via jq

REPO="${GITHUB_REPOSITORY:-$(json_get '.repo')}"
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

  --check-versions          Fail if the changelog adds >1 unreleased
                            version. Only the top may be untagged; every
                            heading below must have its git tag, or a
                            '<!-- release: no-tag -->' directive AND still
                            be published on pub.dev.
                            Env: BRANCH. Exit 1 on a faulty changelog.

  --github-notes TAG        Print GitHub Release notes to stdout.
  --update-tag-hashes TAG   Stamp asset hashes into the tag (post-upload).
  --stamp-changelog TAG     Build the filtered CHANGELOG.md for pub.dev.
  --stamp-readme            Flatten the README <picture> banner for pub.dev.
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

# Configure the git identity + push auth used for CI commits.
git_ci_identity() {
  git config user.name  "github-actions[bot]"
  git config user.email "github-actions[bot]@users.noreply.github.com"

  # The release checkout uses persist-credentials:false (zizmor hardening leaves
  # no token in git config for later steps to leak), so these pushes have no auth.
  # Wire gh's token in as a credential helper — the same GH_TOKEN `gh release`
  # already uses. Skipped on a local dry run, where the push uses your own creds.
  if [ -n "${GH_TOKEN:-}" ]; then
    gh auth setup-git
  fi
}

# A changelog version is owner-written, but it flows into seds, awk programs,
# and file paths, so a stray / or & would corrupt them. Accept only semver.
valid_semver() {  # X.Y.Z with optional -prerelease and +build
  printf '%s' "$1" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$'
}

# Extract one version's entry (body only, heading excluded) from a
# changelog file. The version is regex-escaped before matching.
extract_entry() {
  local file="$1" version="$2"
  local escaped
  escaped=$(printf '%s' "$version" | sed 's/[][\\.^$*+?(){}|]/\\&/g')
  # Drop reserved directive lines (<!-- release: ... -->) so they never
  # reach the published changelog — they are internal tooling markers.
  awk "/^## ${escaped}($| )/{found=1; next} /^## /{found=0} found" "$file" \
    | sed '/<!--[[:space:]]*release:.*-->/d'
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
  curl -sS --retry 3 --retry-delay 2 --connect-timeout 10 --max-time 30 "https://pub.dev/api/packages/$PKG_NAME" 2>/dev/null \
    | jq -r '.versions[].version // empty' 2>/dev/null \
    | sort -t. -k1,1n -k2,2n -k3,3n
}

# Stamp a version string into pubspec.yaml, version.dart, and README.md.
stamp_version() {
  local ver="$1"
  sed -i.bak "s/^version: .*/version: $ver/" pubspec.yaml && rm -f pubspec.yaml.bak
  echo "  pubspec.yaml → $ver"
  sed -i.bak "s/const packageVersion = '[^']*'/const packageVersion = '$ver'/" \
    lib/src/version.dart && rm -f lib/src/version.dart.bak
  echo "  version.dart → $ver"
  sed -i.bak "s/  ${PKG_NAME}:.*/  ${PKG_NAME}: ^$ver/" README.md && rm -f README.md.bak
  sed -i.bak "s/${PKG_NAME}: X\.Y\.Z/${PKG_NAME}: $ver/" README.md && rm -f README.md.bak
  echo "  README.md → $ver"
}

# Generate lib/src/hook/asset_hashes.dart from two sources:
#   1. GitHub Release API digests (native binaries + WASM build outputs)
#   2. Local SHA-256 of hand-written web assets (lane_worker.js)
# Returns 0 if the file was written, 1 if there was nothing to stamp.
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

  # All release assets with digests (native + WASM), excluding source archives.
  local release_entries=""
  local jq_filter
  read -r -d '' jq_filter <<'JQ' || true
.assets[]|select(.digest!=null and (.name|startswith("Source")|not))|"\(.name)\t\(.digest)"
JQ
  # Process substitution (not a `| while` subshell) so a bad digest can `return`
  # and fail the whole function, and assert the algo so a non-sha256 digest
  # fails loud instead of stamping a malformed hash.
  local name digest
  while IFS=$'\t' read -r name digest; do
    [ -z "$name" ] && continue
    case "$digest" in
      sha256:*) : ;;
      *) echo "::error::non-sha256 digest for $name: $digest" >&2; return 1 ;;
    esac
    release_entries+="  '$name': '${digest#sha256:}',"$'\n'
    echo "  $name ... ${digest:7:12}..." >&2
  done < <(gh api "repos/$REPO/releases/tags/$tag" --jq "$jq_filter" | sort)

  # Hand-written web assets — hash from the tag's web_assets/ directory.
  # Reads local filenames + asset names from build.json, skipping
  # wasmBuildOutputs (those get hashes from the Release API above).
  local web_entries=""
  while IFS=$'\t' read -r local_name asset_name; do
    local src="web_assets/$local_name"
    if [ -f "$src" ]; then
      local hash
      hash=$(sha256_file "$src")
      web_entries+="  '$asset_name': '$hash',"$'\n'
      echo "  $asset_name ... ${hash:0:12}..." >&2
    fi
  done < <(
    # Read web assets from build.json, skipping the wasmBuildOutputs.
    wasm_outputs=$(jq -r '.wasmBuildOutputs | join(" ")' build.json)
    jq -r '.web | to_entries[] | "\(.key)\t\(.value)"' build.json \
      | while IFS=$'\t' read -r local_name asset_name; do
          grep -qw "$local_name" <<< "$wasm_outputs" && continue
          echo "$local_name	$asset_name"
        done
  )

  local all_entries
  all_entries=$(printf '%s\n%s' "$release_entries" "$web_entries" | sed '/^$/d')

  if [ -z "$all_entries" ]; then
    echo "  ⚠ No assets to hash — skipping"
    return 1
  fi

  # Replace only the lines between the markers, preserve everything else.
  local start_marker="  // --- GENERATED HASHES START ---"
  local end_marker="  // --- GENERATED HASHES END ---"
  local tmp
  tmp=$(mktemp)
  # entries via ENVIRON, not awk -v, which would interpret a backslash in a
  # filename; the markers stay -v (fixed literals).
  sah_entries="$all_entries" awk -v start="$start_marker" -v end="$end_marker" '
    $0 == start { print; print ENVIRON["sah_entries"]; skip=1; next }
    $0 == end   { print; skip=0; next }
    !skip       { print }
  ' "$hash_file" > "$tmp"
  mv "$tmp" "$hash_file"

  local count
  count=$(grep -c . <<< "$all_entries" || true)
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
# This filters the source changelog for the pub.dev tarball. It is
# keyed PURELY on pub.dev publish status — NOT on git tags. (Whether a
# version is real at all — has a git tag / GitHub release — is a
# SEPARATE concern enforced before release by --check-versions; by the
# time we reach here every heading is already a real, tagged version.)
#
# The case it handles: a version can be real and git-tagged yet never
# pushed to pub.dev — you cut a GitHub release for issue-testing or a
# `git: ref:` install and skipped/deferred pub.dev. It must NOT show as
# a standalone pub.dev "## section" (pub.dev never had that version),
# but its notes still matter — so they fold into the next published
# version instead.
#
# Step 0 — Pick the source file.
#   Stable (no "-") → CHANGELOG.md.  Pre (has "-") → CHANGELOG.pre.md.
#
# Step 1 — Classify each "## heading" as YES or NO.
#   YES = already on pub.dev, OR the version being deployed right now
#         (about to land on pub.dev this run).
#   NO  = a real, tagged version that simply isn't on pub.dev.
#
#   YES → gets its own ## section in the output.
#   NO  → folds into a collapsible under the nearest YES ABOVE it (the
#         newer one). Why newer: the changelog is newest-first and
#         releases are cumulative, so the next-newer published version
#         already contains this NO version's changes — that's the
#         pub.dev release its notes belong under.
#
# Pass 1 — Top to bottom (newest→oldest), build the sections.
#   YES versions get ## headings. Consecutive NO versions buffer, then
#   attach to the previous YES (the newer one) when the next YES hits.
#
# Pass 2 — Bottom to top, add the commit lists.
#   Each YES version gets a <details>Commits since vPREV</details>,
#   where PREV is the YES version directly below it (older). The
#   bottom-most YES version uses "Commits since initial".
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
  count=$(grep -c . <<< "$commits" || true)
  list=$(sed 's/^/- /' <<< "$commits")
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
  for yv in ${yes_list[@]+"${yes_list[@]}"}; do
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
  if [ -z "${BEFORE:-}" ]; then
    echo "Gate: no BEFORE (dispatch, force-push, or new branch); assuming $target_file changed"
    changed_files="$target_file"
  elif ! changed_files=$(git diff --name-only "$BEFORE" "${AFTER:-HEAD}" 2>/dev/null); then
    echo "Gate: git diff failed; assuming $target_file changed"
    changed_files="$target_file"
  fi

  if grep -Fqx "$target_file" <<< "$changed_files"; then
    local versions_after new_version
    versions_after=$(get_changelog_versions "$target_file")

    if [[ -z "${BEFORE:-}" ]]; then
      # No before-SHA (workflow_dispatch, force-push, new branch).
      # Fall back to tag check: if the top version in the changelog
      # has no corresponding git tag, it hasn't been released yet.
      new_version=$(head -1 <<< "$versions_after")
      if [[ -n "$new_version" ]] && git rev-parse "v$new_version" &>/dev/null; then
        echo "Gate: top version $new_version already has tag v$new_version, skipping"
        new_version=""
      fi
    else
      # Normal push — diff-based: trigger only if a new header was added.
      local versions_before
      versions_before=$(git show "${BEFORE}:$target_file" 2>/dev/null \
        | sed -n 's/^## \([^ ]*\).*/\1/p' || true)
      new_version=$(comm -23 <(sort <<< "$versions_after") <(sort <<< "$versions_before") | head -1)
    fi

    if [[ -n "$new_version" ]]; then
      gh_output "should_run" "true"
      gh_output "version" "$new_version"
      echo "Gate: version $new_version found in $target_file — triggering release"
    else
      gh_output "should_run" "false"
      gh_output "version" ""
      echo "Gate: $target_file changed but no unreleased version found, skipping"
    fi
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
  if [ -z "$version" ] || ! valid_semver "$version"; then
    gh_output "has_release" "false"
    echo "No valid version heading in $file"
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
    if grep -q '^false_secrets:' pubspec.yaml; then
      awk '/^false_secrets:/{print; print "  - /vendor/**"; next} 1' pubspec.yaml > pubspec.yaml.tmp
    else
      { cat pubspec.yaml; printf 'false_secrets:\n  - /vendor/**\n'; } > pubspec.yaml.tmp
    fi
    mv pubspec.yaml.tmp pubspec.yaml
    grep -q '^  - /vendor/\*\*$' pubspec.yaml \
      || { echo "::error::failed to add false_secrets /vendor/** to pubspec.yaml"; exit 1; }
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
    "${flags[@]+"${flags[@]}"}"
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
  build_pubdev_changelog "$TAG" > /tmp/_changelog_pubdev.md
  mv /tmp/_changelog_pubdev.md CHANGELOG.md
  echo "  CHANGELOG.md built (filtered for pub.dev)"
}


# ════════════════════════════════════════════════════════════════════
# § 8b — Mode: --stamp-readme
#
# pub.dev strips <picture>/<source> when sanitizing the README, dropping
# the whole block so the banner renders blank. GitHub renders <picture>
# fine, so the repo keeps the dark/light version and we flatten it to the
# inner <img> only in the pub.dev tarball, here at publish time.
#
# Remove this mode (and its call in create-release.yml) once pub.dev
# renders <picture>. Tracking:
#   https://github.com/dart-lang/pub-dev/issues/5923
#   https://github.com/dart-lang/pub-dev/issues/6363
#   https://github.com/google/dart-neats/pull/383
# ════════════════════════════════════════════════════════════════════

cmd_stamp_readme() {
  echo "=== Flattening README <picture> banner for pub.dev ==="
  if ! grep -qE '^[[:space:]]*<picture>[[:space:]]*$' README.md; then
    echo "  no <picture> banner; nothing to flatten"
    return 0
  fi
  # Match <picture>/<source> only as a tag alone on its line (the banner),
  # never a mention inside comment prose.
  awk '
    /^[[:space:]]*<picture>[[:space:]]*$/   { inpic = 1; next }
    /^[[:space:]]*<\/picture>[[:space:]]*$/ { inpic = 0; next }
    inpic && /<source/ { next }
    { print }
  ' README.md > /tmp/_readme_pubdev.md
  mv /tmp/_readme_pubdev.md README.md
  echo "  README.md banner flattened to a single <img>"
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
# § 10 — Mode: --check-versions
#
# Guard against a faulty changelog that introduces MORE THAN ONE new
# (un-released) version at once. A release is cut from the TOP heading
# ONLY (see --discover), so any further un-released heading below it
# would silently collapse into that one release — a heading "faking" a
# release it never actually got. They should have been one version.
#
# Rule: the TOP heading is the release candidate (allowed to be
# untagged — it's about to be tagged on this merge). EVERY heading
# BELOW it must carry its own `v<version>` git tag (proof it was really
# released at some point), UNLESS it carries the no-tag directive AND
# still genuinely exists (see below).
#
# The no-tag directive — a reserved HTML comment on its own line
# directly under the heading:
#   ## 1.0.3
#   <!-- release: no-tag -->
# It says "this version intentionally has no git tag" (e.g. the tag was
# purged but the entry is kept for reference). It is invisible in
# rendered markdown and is stripped from the published changelog by
# extract_entry — it never reaches pub.dev.
#
# But the directive is NOT a blank cheque. A no-tag version is excused
# ONLY if <version> is still published on pub.dev — the one immutable,
# unambiguous proof it genuinely shipped. (A GitHub Release is no proof
# here: a *published* release always carries a git tag, which the tag
# check above already catches, so it never reaches this path; the only
# tagless release is a *draft*, which isn't a shipped version.) A no-tag
# version not on pub.dev is faking a release — rejected even with the
# directive.
#
# Lane falls out of the version string itself, no special-casing:
#   CHANGELOG.md     heading `2.0.2`        → looks for tag `v2.0.2`
#   CHANGELOG.pre.md heading `2.0.2-dev.0`  → looks for tag `v2.0.2-dev.0`
# So a stable heading is NOT satisfied by its `-dev.0` prerelease tag —
# they're different version strings, hence different tags.
#
# This is NOT the pub.dev fold (the one that asks "is it on pub.dev?");
# this one asks "does it exist as a release at all?". Orthogonal.
#
# Run it at PR time (block the bad merge) AND in the release workflow
# (catch a faulty/forced merge that skipped the PR check).
#
# Env  : BRANCH (dev|prod) — picks the lane/file.
# Exit : 0 = clean, 1 = faulty (offending versions printed).
# ════════════════════════════════════════════════════════════════════

cmd_check_versions() {
  local branch="${BRANCH:?--check-versions requires BRANCH env var}"
  local file
  if [[ "$branch" == "prod" ]]; then
    file="CHANGELOG.md"
  else
    file="CHANGELOG.pre.md"
  fi

  if [ ! -f "$file" ]; then
    echo "✓ no $file — nothing to check"
    return 0
  fi

  # Tags are the source of truth; a shallow CI checkout may lack them.
  git fetch --tags --quiet origin 2>/dev/null || true

  # Per version (newest first): "<version>\t<has-no-tag-directive 0|1>".
  # The directive is a `<!-- release: ... no-tag ... -->` comment line
  # anywhere in the version's block (convention: directly under heading).
  local parsed
  parsed=$(awk '
    /^## / { if (v != "") print v "\t" n; v = $2; n = 0; next }
    /<!--[[:space:]]*release:.*no-tag.*-->/ { n = 1 }
    END { if (v != "") print v "\t" n }
  ' "$file")

  # grep -c prints "0" and exits 1 on no match, so guard the empty case
  # explicitly rather than lean on that (|| echo 0 would double-count).
  local total=0
  if [ -n "$parsed" ]; then
    total=$(grep -c . <<< "$parsed")
  fi
  # The top heading is the release candidate that gets stamped; validate its
  # shape here so a malformed version fails the PR, not the release.
  if [ -n "$parsed" ]; then
    local rc_ver
    rc_ver=$(head -1 <<< "$parsed" | cut -f1)
    if ! valid_semver "$rc_ver"; then
      echo "✗ $file: top version '$rc_ver' is not valid semver" >&2
      return 1
    fi
  fi
  if [ "$total" -le 1 ]; then
    echo "✓ $file: $total version heading(s) — nothing below the top to check"
    return 0
  fi

  # Pub.dev versions — fetched once, and only if some heading carries the
  # no-tag directive (otherwise we never need them).
  local published=""
  if awk -F'\t' 'NR > 1 && $2 == 1 { f = 1 } END { exit !f }' <<< "$parsed"; then
    published=$(get_published_versions || true)
  fi

  local top_ver
  top_ver=$(head -1 <<< "$parsed" | cut -f1)
  echo "Top (release candidate): $top_ver — exempt"

  local -a faulty_untagged=() faulty_phantom=()
  local idx=0 ver notag
  while IFS=$'\t' read -r ver notag; do
    idx=$((idx + 1))
    [ "$idx" -eq 1 ] && continue   # skip the top (release candidate)
    [ -z "$ver" ] && continue

    if git rev-parse -q --verify "refs/tags/v$ver" >/dev/null 2>&1; then
      echo "  • $ver — tag v$ver ✓"
      continue
    fi

    if [ "$notag" != "1" ]; then
      faulty_untagged+=("$ver")
      echo "  ✗ $ver — no tag v$ver, no no-tag directive"
      continue
    fi

    # no-tag directive present: excused ONLY if still published on
    # pub.dev — the only proof of a genuine ship that a missing tag
    # doesn't already cover (a published GitHub Release implies a tag).
    if [ -n "$published" ] && grep -qxF "$ver" <<< "$published"; then
      echo "  • $ver — tagless, no-tag directive, still on pub.dev ✓"
    else
      faulty_phantom+=("$ver")
      echo "  ✗ $ver — no-tag directive but no tag and not on pub.dev"
    fi
  done <<< "$parsed"

  local bad=0
  if [ ${#faulty_untagged[@]} -gt 0 ]; then
    bad=1
    echo ""
    echo "::error::$file: below-top version(s) with no git tag and no no-tag directive — ${faulty_untagged[*]}"
    echo "A release is cut from the TOP heading only, so these would silently collapse into it."
    echo "Fix each: release it before adding a newer one, fold it, or remove it — or, ONLY if it genuinely shipped and is still published on pub.dev, add a '<!-- release: no-tag -->' line under its heading."
  fi
  if [ ${#faulty_phantom[@]} -gt 0 ]; then
    bad=1
    echo ""
    echo "::error::$file: version(s) marked no-tag that are not on pub.dev — ${faulty_phantom[*]}"
    echo "'<!-- release: no-tag -->' only excuses a version still published on pub.dev (the immutable proof it shipped). These are not — they are faking a release. Remove them or fold them."
  fi
  [ "$bad" -eq 1 ] && return 1

  echo ""
  echo "✓ $file: one new version at the top ($top_ver); every heading below is tagged or a verified no-tag"
  return 0
}


# ════════════════════════════════════════════════════════════════════
# § 11 — Dispatch
# ════════════════════════════════════════════════════════════════════

main() {
  case "$MODE" in
    --help | -h)         usage ;;
    --gate)              cmd_gate ;;
    --check-versions)    cmd_check_versions ;;
    --discover)          cmd_discover ;;
    --github-notes)      cmd_github_notes ;;
    --update-tag-hashes) cmd_update_tag_hashes ;;
    --stamp-changelog)   cmd_stamp_changelog ;;
    --stamp-readme)      cmd_stamp_readme ;;
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