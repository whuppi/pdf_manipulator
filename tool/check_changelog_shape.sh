#!/usr/bin/env bash
# Changelog shape wall, the part of the standard (the comment block at the
# top of CHANGELOG.md) a reviewer would otherwise have to count by hand:
#   - a bullet is at most 60 words (80 for a **Breaking** bullet, whose
#     migration is inline);
#   - in each entry the **Action:** bullets come first, then **Breaking:**,
#     then the rest;
#   - an entry's lead paragraph is at most 80 words, its migration-guide
#     sentence not counted.
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

# Bullet order and the lead's length, per entry. The comment block at the
# top of a file is the standard itself, so it is skipped.
for file in CHANGELOG.md CHANGELOG.pre.md; do
  found=$(awk '
    function words(t,   n, a) {
      gsub(/\]\([^)]*\)/, "]", t)
      gsub(/^[ \t]+|[ \t]+$/, "", t)
      return t == "" ? 0 : split(t, a, /[ \t]+/)
    }
    function close_lead() {
      if (lead == "") return
      gsub(/Migrating from [^:]*: the \[migration guide\]\([^)]*\) has the before and after code [^.]*\./, "", lead)
      gsub(/(^|[ ])>/, " ", lead)
      if (words(lead) > 80) printf "%s:%d: the lead paragraph has %d words (limit 80, the migration sentence not counted)\n", FILENAME, head, words(lead)
      lead = ""
    }
    /^<!--/ && !/-->/ { incomment = 1; next }
    incomment { if (/-->/) incomment = 0; next }
    /^## / { close_lead(); head = NR; rank = 0; bullets = 0; next }
    head == 0 { next }
    /^- / {
      if (!bullets) close_lead()
      bullets = 1
      r = 2
      if (/^- \*\*Action:\*\*/) r = 0
      else if (/^- \*\*Breaking/) r = 1
      if (r < rank) printf "%s:%d: an %s bullet after a later kind; the order is Action, Breaking, then the rest\n", FILENAME, NR, (r == 0 ? "Action" : "Breaking")
      if (r > rank) rank = r
      next
    }
    !bullets && !/^<!--/ && NF { lead = lead " " $0 }
    END { close_lead() }
  ' "$file")
  if [ -n "$found" ]; then
    printf '%s\n' "$found"
    bad=1
  fi
done

if [ "$bad" -ne 0 ]; then
  echo
  echo "A changelog bullet is one sentence: what changed and what you do about"
  echo "it. The cause and the proof go in the PR (the entry's commit list"
  echo "reaches it). Cut the bullet or the lead; do not raise the limit. Put"
  echo "the bullets in the order the standard gives."
  exit 1
fi
