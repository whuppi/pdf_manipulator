#!/usr/bin/env bash
# Check cargo warnings against git-changed lines only.
# Reads streaming JSON from cargo check on stdin.
# Args: $1 = diff output (from git diff --unified=0)
#
# Exits 0 if no warnings in changed lines, 1 if any found.

set -euo pipefail

diff_text="$1"

# Build changed file:line lookup from diff hunks
declare -A changed_files
cur_file=""
while IFS= read -r line; do
  case "$line" in
    "+++ b/"*)
      cur_file="${line#'+++ b/'}"
      ;;
    "@@"*)
      start=$(echo "$line" | sed -n 's/.*+\([0-9][0-9]*\).*/\1/p')
      count=$(echo "$line" | sed -n 's/.*+[0-9][0-9]*,\([0-9][0-9]*\).*/\1/p')
      count=${count:-1}
      [ "$count" -eq 0 ] && count=1
      for (( i=start; i<start+count; i++ )); do
        changed_files["${cur_file}:${i}"]=1
      done
      ;;
  esac
done <<< "$diff_text"

# Process cargo JSON output line by line
warns=0
skipped=0
while IFS= read -r json_line; do
  # Only compiler-message warnings
  echo "$json_line" | grep -q '"reason":"compiler-message"' || continue
  echo "$json_line" | grep -q '"level":"warning"' || continue

  # Extract primary span file_name and line_start
  span_file=$(echo "$json_line" | grep -oE '"file_name":"[^"]*"' | head -1 | sed 's/"file_name":"//;s/"//')
  span_line=$(echo "$json_line" | grep -oE '"line_start":[0-9]+' | head -1 | sed 's/"line_start"://')
  msg_text=$(echo "$json_line" | sed -n 's/.*"message":"\([^"]*\)".*/\1/p' | head -1)

  if [ -z "$span_file" ] || [ -z "$span_line" ]; then
    skipped=$((skipped + 1))
    continue
  fi

  if [[ -n "${changed_files[${span_file}:${span_line}]:-}" ]]; then
    echo "  ${span_file}:${span_line}: ${msg_text}"
    warns=$((warns + 1))
  fi
done

if [ "$skipped" -gt 0 ]; then
  echo "  ($skipped warning(s) skipped — could not parse span)" >&2
fi

if [ "$warns" -gt 0 ]; then
  echo ""
  echo "${warns} warning(s) in our changed lines"
  exit 1
fi
