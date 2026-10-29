#!/usr/bin/env bash
# Fork test wall (docs/UPDATING.md, S2): our tests live in fork-only code
# (src/host/), never in upstream code or the fork's tests/ tree.
#
# Four checks per vendored engine:
#   1. no `#[test]` added to upstream code: a file of src/ outside host/, or
#      of tests/, holds no more `#[test]` than it did at the base tag
#   2. no PdfDocument / DocumentEditor inside a host/ `#[cfg(test)]` module,
#      except in src/host/engine_proofs/
#   3. every `#[test]` in src/host/engine_proofs/ has a doc comment, which
#      says why no Dart battery can observe what it proves
#   4. no file of ours added under tests/ (compared with the base tag the
#      patches branch is named after: pdf_manipulator/<base>-patches)
set -euo pipefail

cd "$(dirname "$0")/.."

bad=0

# Number of `#[test]` lines in stdin.
count_tests() {
  grep -c '#\[test\]' || true
}

check_engine() {
  local dir=$1 host=$2
  [ -d "$dir/src" ] || return 0

  local branch base have_base=0
  branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  base=${branch#*/}
  base=v${base%-patches}
  if git -C "$dir" rev-parse -q --verify "refs/tags/$base" >/dev/null 2>&1; then
    have_base=1
  fi

  # 1. tests added to upstream code
  if [ "$have_base" -eq 1 ]; then
    local file now was
    while IFS= read -r file; do
      case "$file" in src/"$host"/*) continue ;; esac
      [ -f "$dir/$file" ] || continue
      now=$(count_tests < "$dir/$file")
      was=$( { git -C "$dir" show "$base:$file" 2>/dev/null || true; } | count_tests)
      if [ "$now" -gt "$was" ]; then
        echo "$dir/$file: $((now - was)) #[test] added to upstream code — put it in $dir/src/$host/engine_proofs/ or in a Dart battery in test/ops/"
        bad=1
      fi
    done < <(git -C "$dir" ls-files -- src tests | grep '\.rs$')
  fi

  # 2. document-level types inside fork-only test modules. A plain #[cfg(test)]
  #    module is pure; a feature-negated test module is a trim probe run by
  #    tool/shake_audit.sh (which selects tests by the substring trim_probe),
  #    allowed a minimal PDF, and must carry trim_probe in its module name.
  #    src/host/engine_proofs/ is exempt: it exists to open documents.
  while IFS= read -r file; do
    case "$file" in "$dir/src/$host/engine_proofs/"*) continue ;; esac
    if awk '
        /#\[cfg\(all\(test, *not\(feature/ { probe = 1; next }
        probe == 1 { probe = 0; if ($0 !~ /^(pub(\(crate\))? )?mod [a-z_]*trim_probe/) { misnamed = 1 }; next }
        /#\[cfg\(.*test/ { inside = 1; depth = 0; opened = 0 }
        inside {
          if ($0 ~ /(PdfDocument|DocumentEditor)/) { found = 1 }
          n = gsub(/\{/, "{"); m = gsub(/\}/, "}")
          depth += n - m; if (n > 0) { opened = 1 }
          if (opened && depth <= 0) { inside = 0 }
        }
        END { exit (found || misnamed) ? 0 : 1 }' "$file"; then
      echo "$file: a fork-only test opens a document, or a feature-negated test module is not a trim_probe — put it in $dir/src/$host/engine_proofs/ or in a Dart battery in test/ops/"
      bad=1
    fi
  done < <(grep -rl '#\[cfg(.*test' "$dir/src/$host" 2>/dev/null || true)

  # 3. engine proofs say why a Dart battery cannot see them
  while IFS= read -r file; do
    awk -v f="$file" '
        /^[ \t]*#\[test\]/ && prev !~ /^[ \t]*\/\/\// {
          print f ":" NR ": a #[test] with no doc comment — say why no Dart battery can observe it"
          missing = 1
        }
        { prev = $0 }
        END { exit missing ? 1 : 0 }' "$file" || bad=1
  done < <(find "$dir/src/$host/engine_proofs" -name '*.rs' 2>/dev/null || true)

  # 4. files of ours under tests/
  if [ "$have_base" -eq 1 ]; then
    local added
    added=$(git -C "$dir" diff --name-only --diff-filter=A "$base" -- tests 2>/dev/null || true)
    if [ -n "$added" ]; then
      echo "$dir: test files added under tests/ since $base:"
      printf '  %s\n' $added
      bad=1
    fi
  fi
}

check_engine vendor/pdf_oxide host
check_engine vendor/office_oxide host

if [ "$bad" -ne 0 ]; then
  echo
  echo "docs/UPDATING.md, S2: upstream code gets no tests. Fork-only code (src/host/)"
  echo "unit-tests pure functions; a proof that opens a document goes in"
  echo "src/host/engine_proofs/ with a doc comment saying why no Dart battery"
  echo "can observe it; every other proof is a Dart battery in test/ops/."
  exit 1
fi
