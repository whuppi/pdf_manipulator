#!/usr/bin/env bash
# Changelog bullet wall: one change per bullet, one sentence, at most 60
# words (80 for a **Breaking** bullet, whose migration is inline). The
# standard is in the comment block at the top of CHANGELOG.md; this is the
# part of it a reviewer would otherwise have to count by hand.
#
# Words are counted after dropping link targets, so a long GitHub URL
# never pushes a short bullet over the line.
set -euo pipefail

cd "$(dirname "$0")/.."

limit=60
breaking_limit=80
bad=0
for file in CHANGELOG.md CHANGELOG.pre.md; do
  line_no=0
  while IFS= read -r line; do
    line_no=$((line_no + 1))
    case "$line" in
      "- "*) ;;
      *) continue ;;
    esac
    body=${line#- }
    # Strip markdown link targets: "](https://…)" → "]".
    stripped=$(printf '%s' "$body" | sed -E 's/\]\([^)]*\)/]/g')
    read -ra words <<< "$stripped"
    count=${#words[@]}
    max=$limit
    case "$body" in
      "**Breaking"*) max=$breaking_limit ;;
    esac
    if [ "$count" -gt "$max" ]; then
      echo "$file:$line_no: $count words (limit $max): ${body:0:80}…"
      bad=1
    fi
  done < "$file"
done

if [ "$bad" -ne 0 ]; then
  echo
  echo "A changelog bullet is one sentence: what changed and what you do about"
  echo "it. The cause and the proof go in the PR (the entry's commit list"
  echo "reaches it). Cut the bullet; do not raise the limit."
  exit 1
fi
