#!/usr/bin/env bash
# shellcheck shell=bash
# Shell portability + correctness gate — the "zizmor for shell". Same run
# locally (make lint-shell) and in CI (pr-lint.yml). Three checks:
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
#
# Scope, honestly: this catches shell-LANGUAGE portability (bash version, the
# wrong default shell). It does NOT catch BSD-vs-GNU coreutils flag drift —
# sed -i, grep -P, readlink -f, date/stat formats — because no static tool
# does. The macOS CI leg (real BSD userland) is the backstop for those.
#
# The scan is plain grep: never paste a bash 4.0+ construct verbatim into a
# comment (it can't tell code from comment) — name it, as this header does.
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

echo ""
if [ "$status" -eq 0 ]; then
  echo "Shell lint passed."
else
  echo "Shell lint FAILED." >&2
fi
exit "$status"
