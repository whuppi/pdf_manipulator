#!/usr/bin/env bash
# shellcheck shell=bash
# Shell portability + correctness gate — the "zizmor for shell". Same run
# locally (make lint-shell) and in CI (pr-lint.yml). Four checks:
#   1. shellcheck     — correctness, quoting, broad portability.
#   2. bash 4.0+ scan — catches macOS-bash-3.2 breaks. macOS is frozen on bash
#        3.2, so a bash 4.0+ feature under `shell: bash` there is a fatal "bad
#        substitution". shellcheck's bash mode treats them as valid and
#        actionlint lints run: blocks in bash mode, so neither catches them —
#        this scan does, in scripts AND run: blocks. Keep it — shellcheck does
#        not replace it.
#   3. workflow shell — every workflow run step resolves to bash (its own
#        shell: or the workflow's defaults.run.shell), so a step can't land on
#        a Windows runner's pwsh default and break on bash syntax.
#   4. coreutils scan — GNU-only flags that break on macOS's BSD userland
#        (in-place sed, perl-regex grep, and friends). A curated blocklist,
#        not exhaustive: no static tool covers all of coreutils, so the macOS
#        CI leg (real BSD) stays the backstop. Add gotchas here as they bite.
#
# Both scans are plain grep: never paste a flagged construct verbatim into a
# comment (grep can't tell code from comment) — name it, as this header does.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
# shellcheck source=tool/lib.sh
source "tool/lib.sh"

require_present shellcheck \
  "macOS:   brew install shellcheck" \
  "Linux:   sudo apt-get install -y shellcheck" \
  "Windows: choco install shellcheck"
require_present yq \
  "macOS:   brew install yq" \
  "Linux:   snap install yq   (the Go yq from mikefarah, not the apt package)" \
  "Windows: choco install yq"

status=0

# ── 1. shellcheck every tracked shell script ─────────────────────────
# -x follows sourced files; warning severity (notes stay advisory).
echo "── shellcheck (tracked *.sh) ──"
sh_files=()
while IFS= read -r f; do
  sh_files+=("$f")
done < <(git ls-files '*.sh')
if [ "${#sh_files[@]}" -gt 0 ]; then
  if shellcheck -x -S warning "${sh_files[@]}"; then
    echo "  clean"
  else
    status=1
  fi
fi

# ── 2. bash 4.0+ feature scan (scripts + .github run: blocks) ─────────
echo "── bash 4.0+ portability scan (tool/ + .github/) ──"
hits=0
scan() {  # ERE  human-description
  local found
  found=$(grep -rnE "$1" tool .github \
    --include='*.sh' --include='*.yml' --include='*.yaml' 2>/dev/null || true)
  if [ -n "$found" ]; then
    echo "  bash 4.0+ feature — $2:" >&2
    printf '%s\n' "$found" | sed 's/^/    /' >&2
    hits=1
  fi
}
# Each ERE matches ONLY a 4.0+ construct, never a 3.2-safe bashism.
scan '\$\{[A-Za-z_][A-Za-z0-9_]*(\[[^]]*\])?(\^|,)'             'case modification (caret/comma)'
scan '\$\{[A-Za-z_][A-Za-z0-9_]*(\[[^]]*\])?@[a-zA-Z]'         'parameter transform (at-letter)'
scan '(^|[^[:alnum:]_])(declare|local|typeset|readonly)[[:space:]]+-[a-zA-Z]*A' 'associative array'
scan '(^|[^[:alnum:]_])(mapfile|readarray)([[:space:]]|$)'     'mapfile/readarray'
scan '(^|[^[:alnum:]_])coproc([[:space:]]|$)'                  'coproc'
scan '&>{2}'                                                   'append-both redirect'
if [ "$hits" -eq 0 ]; then
  echo "  clean — bash 3.2 compatible"
else
  status=1
fi

# ── 3. every workflow run step resolves to bash (Windows safety) ──────
# A run step with bash syntax that lands on a Windows runner (default shell
# pwsh) breaks. Each run step must be bash, via its own shell: or the
# workflow's defaults.run.shell. yq reads the effective shell properly.
echo "── workflow run steps are bash (Windows safety) ──"
nonbash=0
for wf in .github/workflows/*.yml; do
  [ -e "$wf" ] || continue
  bad=$(yq '(.defaults.run.shell // "") as $d | .jobs[] | select(.steps) | .steps[] | select(has("run")) | select((.shell // $d) != "bash") | (.name // .id // "unnamed")' "$wf" 2>/dev/null || true)
  if [ -n "$bad" ]; then
    echo "  ${wf#./} — run step(s) not on bash (set shell: bash or defaults.run.shell):" >&2
    printf '%s\n' "$bad" | sed 's/^/    /' >&2
    nonbash=1
  fi
done
if [ "$nonbash" -eq 0 ]; then
  echo "  clean — every workflow run step is bash"
else
  status=1
fi

# ── 4. GNU-only coreutils flags (break on macOS's BSD userland) ───────
# Curated blocklist. Descriptions name each flag in prose, never as the
# literal command-and-flag, so the scan can't match its own text.
echo "── BSD/GNU coreutils portability (tool/ + .github/) ──"
gnuism=0
gscan() {  # ERE  human-description
  local found
  found=$(grep -rnE "$1" tool .github \
    --include='*.sh' --include='*.yml' --include='*.yaml' 2>/dev/null || true)
  if [ -n "$found" ]; then
    echo "  GNU-only — $2:" >&2
    printf '%s\n' "$found" | sed 's/^/    /' >&2
    gnuism=1
  fi
}
gscan '\bsed\b +(-[a-zA-Z]+ )*-i([[:space:]]|$)'    'in-place sed with no suffix (BSD needs one; use a .bak suffix or a tmpfile)'
gscan '\bsed\b +-[a-zA-Z]*r([[:space:]]|$)'         'sed extended-regex via the GNU flag (use the portable E flag)'
gscan '\bgrep\b[^|;&]* -[a-zA-Z]*P([[:space:]]|$)'  'grep perl-regex flag'
gscan '\breadlink\b +-[a-zA-Z]*f'                   'readlink follow flag (absent on macOS; use a realpath helper)'
gscan '\bdate\b +(-d\b|--date)'                     'date relative flag (BSD uses the v flag)'
gscan '\bstat\b +(-c\b|--format|--printf)'          'stat format flag (BSD uses the f flag)'
gscan '\bfind\b[^|;&]*-printf'                       'find print-format action'
gscan '\bxargs\b +(-d\b|--delimiter)'               'xargs delimiter flag'
gscan '\bcp\b[^|;&]*--parents'                       'cp parents flag'
gscan '(^|[^[:alnum:]_./-])(tac|nproc|sponge)([[:space:]]|$)' 'GNU-only command (no BSD tool by that name)'
if [ "$gnuism" -eq 0 ]; then
  echo "  clean — portable across BSD + GNU"
else
  status=1
fi

echo ""
if [ "$status" -eq 0 ]; then
  echo "Shell lint passed."
else
  echo "Shell lint FAILED." >&2
fi
exit "$status"
