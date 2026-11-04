#!/usr/bin/env bash
# Migration wall: a release that breaks anything ships with its migration.
#
# For the newest entry of each changelog (CHANGELOG.md's `## X.Y.Z`,
# CHANGELOG.pre.md's `## X.Y.Z-dev.N`), when it holds a **Breaking:** bullet
# and its version is 6.0.0 or later, docs/MIGRATION.md must have the section
# `## From <previous> to <X.Y.Z>`, where <previous> is the newest release in
# CHANGELOG.md below X.Y.Z, and that section must name, in backticks, the
# first backticked name of every Breaking bullet of the entry.
#
# Every `##` heading of MIGRATION.md is such a section, and each names a
# version one of the changelogs holds: no stale or stray section.
#
# The floor is 6.0.0 because the guide starts fresh there; the releases
# before it are described by CHANGELOG.md alone.
set -euo pipefail

cd "$(dirname "$0")/.."

floor=6.0.0
guide=docs/MIGRATION.md
bad=0

# True when version $1 (X.Y.Z) is below $2, compared part by part.
below() {
  local a1 a2 a3 b1 b2 b3
  IFS=. read -r a1 a2 a3 <<< "$1"
  IFS=. read -r b1 b2 b3 <<< "$2"
  (( a1 != b1 )) && { (( a1 < b1 )); return; }
  (( a2 != b2 )) && { (( a2 < b2 )); return; }
  (( a3 < b3 ))
}

# The lines of the first `## ` entry of a changelog, heading excluded.
top_entry() { awk '/^## /{n++; next} n==1' "$1"; }

# The newest release in CHANGELOG.md below $1.
previous_of() {
  local v best=""
  while IFS= read -r v; do
    if below "$v" "$1" && { [ -z "$best" ] || below "$best" "$v"; }; then best=$v; fi
  done < <(sed -nE 's/^## ([0-9]+\.[0-9]+\.[0-9]+)$/\1/p' CHANGELOG.md)
  printf '%s' "$best"
}

# The lines of the guide's section under heading $1, heading excluded.
section_of() { awk -v h="$1" '$0==h{on=1; next} /^## /{on=0} on' "$guide"; }

for file in CHANGELOG.md CHANGELOG.pre.md; do
  heading=$(grep -m1 '^## ' "$file")
  target=$(printf '%s' "$heading" | sed -nE 's/^## ([0-9]+\.[0-9]+\.[0-9]+)(-dev\.[0-9]+)?$/\1/p')
  [ -n "$target" ] || continue
  below "$target" "$floor" && continue
  bullets=$(top_entry "$file" | grep '^- \*\*Breaking:\*\*' || true)
  [ -n "$bullets" ] || continue
  previous=$(previous_of "$target")
  if [ -z "$previous" ]; then
    echo "$file: no release below $target in CHANGELOG.md to migrate from"
    bad=1
    continue
  fi
  want="## From $previous to $target"
  if ! grep -qxF "$want" "$guide"; then
    echo "$guide: missing the section '$want' for $file's $heading"
    bad=1
    continue
  fi
  text=$(section_of "$want")
  while IFS= read -r bullet; do
    name=$(printf '%s' "$bullet" | sed -nE 's/^[^`]*`([^`]+)`.*/\1/p')
    if [ -z "$name" ]; then
      echo "$file ($heading): a Breaking bullet names nothing in backticks: ${bullet:0:80}"
      bad=1
    elif ! printf '%s\n' "$text" | grep -qF "\`$name\`"; then
      echo "$guide ($want): no entry names \`$name\` (from $file: ${bullet:0:80})"
      bad=1
    fi
  done <<< "$bullets"
done

while IFS= read -r heading; do
  if ! printf '%s' "$heading" | grep -qE '^## From [0-9]+\.[0-9]+\.[0-9]+ to [0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "$guide: '$heading' is not a 'From X to Y' section"
    bad=1
    continue
  fi
  v=${heading##* to }
  if ! grep -qE "^## ${v//./\\.}(-dev\.[0-9]+)?$" CHANGELOG.md CHANGELOG.pre.md; then
    echo "$guide: '$heading' names $v, which neither changelog holds"
    bad=1
  fi
done < <(grep '^## ' "$guide")

if [ "$bad" -ne 0 ]; then
  echo "docs/MIGRATION.md must cover every breaking change of the newest release (d-127)."
  exit 1
fi
echo "every breaking change of the newest release has its migration"
