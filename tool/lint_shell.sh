#!/usr/bin/env bash
# shellcheck shell=bash
# Shell portability + correctness gate — the "zizmor for shell". It lints the
# git repo at the CURRENT directory, so it runs both ways: `make check` in
# whuppi/ci (linting itself) and the reusable pr-checks workflow (cd'd into the
# consumer checkout, linting the consumer). Four checks:
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

# Self-contained: no lib.sh source (this gate uses none of its helpers), so it
# stamps into any package's tool/ without dragging a lib.sh along.
# Lints the git repo at the CURRENT directory — cd into the repo to lint first.
git rev-parse --show-toplevel >/dev/null 2>&1 || { echo "lint_shell: run inside a git repo" >&2; exit 2; }

# A missing tool skips ONLY its own check, with a loud warning — it never
# aborts the run, so an absent shellcheck or yq still lets the grep scans run
# and doesn't block the other checks. CI preinstalls both and enforces the full
# set, so nothing is silently lost there.
skip_note() {  # tool  install-hint
  echo "  ⚠ SKIPPED — '$1' not found ($2). CI enforces this; install to run it locally." >&2
}

status=0

# What the scans inspect: tracked shell scripts (repo-wide) and tracked .github
# YAML (workflow + composite-action run: blocks). git ls-files — not a `grep -r`
# directory walk — keeps it to tracked files, so a gitignored vendor/ or
# generated/ dir is never pulled in (no path excludes to maintain). If a
# *tracked* vendored tree ever needs skipping, add a `:!path` pathspec to the
# git ls-files calls below.
sh_files=()
while IFS= read -r f; do sh_files+=("$f"); done < <(git ls-files '*.sh')

# /dev/null sentinel: keeps the array non-empty so it expands safely under
# `set -u` on bash 3.2, and grep never falls through to reading stdin.
scan_files=(/dev/null)
[ "${#sh_files[@]}" -gt 0 ] && scan_files+=("${sh_files[@]}")
while IFS= read -r f; do scan_files+=("$f"); done < <(git ls-files '.github' | grep -E '\.ya?ml$')

# ── 1. shellcheck every tracked shell script ─────────────────────────
# -x follows sourced files; warning severity (notes stay advisory).
echo "── shellcheck (tracked *.sh) ──"
if ! command -v shellcheck >/dev/null 2>&1; then
  skip_note shellcheck "brew install shellcheck / apt-get install shellcheck"
elif [ "${#sh_files[@]}" -gt 0 ]; then
  if shellcheck -x -S warning "${sh_files[@]}"; then
    echo "  clean"
  else
    status=1
  fi
fi

# ── 2. bash 4.0+ feature scan (scripts + .github run: blocks) ─────────
echo "── bash 4.0+ portability scan (tracked shell + .github YAML) ──"
hits=0
scan() {  # ERE  human-description
  local found
  found=$(grep -nE "$1" "${scan_files[@]}" 2>/dev/null || true)
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
if ! command -v yq >/dev/null 2>&1; then
  skip_note yq "brew install yq / snap install yq (mikefarah Go build) / choco install yq"
else
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
fi

# ── 4. GNU-only coreutils flags (break on macOS's BSD userland) ───────
# Curated blocklist. Descriptions name each flag in prose, never as the
# literal command-and-flag, so the scan can't match its own text.
echo "── BSD/GNU coreutils portability (tracked shell + .github YAML) ──"
gnuism=0
gscan() {  # ERE  human-description
  local found
  found=$(grep -nE "$1" "${scan_files[@]}" 2>/dev/null || true)
  if [ -n "$found" ]; then
    echo "  GNU-only — $2:" >&2
    printf '%s\n' "$found" | sed 's/^/    /' >&2
    gnuism=1
  fi
}
# These patterns use the portable (^|[^[:alnum:]_]) boundary idiom, never the
# backslash-b form: a GNU / modern-BSD grep extension that matches nothing on an
# old BSD grep (the userland this check protects). A gscan rule here flags any
# grep/sed reaching for the non-portable boundary forms — this file included.
gscan '(^|[^[:alnum:]_])sed +(-[a-zA-Z]+ )*-i([[:space:]]|$)' 'in-place sed with no suffix (BSD needs one; use a .bak suffix or a tmpfile)'
gscan '(^|[^[:alnum:]_])sed +-[a-zA-Z]*r([[:space:]]|$)' 'sed extended-regex via the GNU flag (use the portable E flag)'
gscan '(^|[^[:alnum:]_])grep([[:space:]][^|;&]*)? -[a-zA-Z]*P([[:space:]]|$)' 'grep perl-regex flag'
gscan '(^|[^[:alnum:]_])readlink +-[a-zA-Z]*f' 'readlink follow flag (absent on macOS; use a realpath helper)'
gscan '(^|[^[:alnum:]_])date +(-d([^[:alnum:]_]|$)|--date)' 'date relative flag (BSD uses the v flag)'
gscan '(^|[^[:alnum:]_])stat +(-c([^[:alnum:]_]|$)|--format|--printf)' 'stat format flag (BSD uses the f flag)'
gscan '(^|[^[:alnum:]_])find[[:space:]][^|;&]*-printf' 'find print-format action'
gscan '(^|[^[:alnum:]_])xargs +(-d([^[:alnum:]_]|$)|--delimiter)' 'xargs delimiter flag'
gscan '(^|[^[:alnum:]_])cp[[:space:]][^|;&]*--parents' 'cp parents flag'
gscan '(^|[^[:alnum:]_./-])(tac|nproc|sponge)([[:space:]]|$)' 'GNU-only command (no BSD tool by that name)'
gscan '(^|[^[:alnum:]_])sort +-[a-zA-Z]*V' 'sort version-sort flag (older BSD sort lacks it)'
gscan '(^|[^[:alnum:]_])grep[[:space:]].*\\\|' 'grep BRE alternation via backslash-pipe (switch to -E with a plain pipe)'
gscan '(^|[^[:alnum:]_])sed[[:space:]].*\\\|' 'sed BRE alternation via backslash-pipe (switch to -E with a plain pipe)'
gscan '(^|[^[:alnum:]_])(sha256sum|sha512sum|sha1sum|md5sum|shasum)[[:space:]][^|<]*["$][^|]*\|[[:space:]]*(awk|cut|head)' 'checksum of a path argument piped to field extraction — coreutils escapes the digest line on a backslash/Windows path; feed the file on stdin'
gscan '(^|[^[:alnum:]_])(grep|sed)[[:space:]][^|;&]*(\\[b<>]|\[\[:[<>]:\]\])' 'non-portable word boundary in a grep/sed regex — backslash-b and backslash-angle break on old BSD grep, the bracket-colon-angle forms break on GNU; use the portable (^|[^[:alnum:]_]) class'
if [ "$gnuism" -eq 0 ]; then
  echo "  clean — portable across BSD + GNU"
else
  status=1
fi

# ── 5. versions.env single-writer (set_kv only) ──────────────────────
# versions.env is sourced (executed), so every write must pass set_kv's shape
# gate. set_kv writes through a temp file and a rename, never a literal redirect
# into the file, so a second writer (a redirect, a tee, or an in-place stream
# edit that names versions.env) would slip an unvalidated value into a sourced
# file. Flag any such writer. (Constructs are named in prose, not pasted, so
# this rule doesn't match itself.)
echo "── versions.env single-writer (set_kv only) ──"
vw_files=(/dev/null)
while IFS= read -r f; do vw_files+=("$f"); done < <(git ls-files '*.sh'; git ls-files '.github' | grep -E '\.ya?ml$')
vw=$(grep -nE '(>>?[[:space:]]*"?[^"[:space:]]*versions\.env|(^|[^[:alnum:]_])tee[[:space:]][^|]*versions\.env|(^|[^[:alnum:]_])sed[[:space:]]+(-[a-zA-Z]+[[:space:]]+)*-i[^|]*versions\.env)' "${vw_files[@]}" 2>/dev/null || true)
if [ -z "$vw" ]; then
  echo "  clean — only set_kv writes versions.env"
else
  echo "  versions.env written outside set_kv (sourced file; an unvalidated write executes):" >&2
  printf '%s\n' "$vw" | sed 's/^/    /' >&2
  status=1
fi

echo ""
if [ "$status" -eq 0 ]; then
  echo "Shell lint passed."
else
  echo "Shell lint FAILED." >&2
fi
exit "$status"
