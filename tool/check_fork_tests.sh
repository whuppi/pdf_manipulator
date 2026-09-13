#!/usr/bin/env bash
# Fork test wall (docs/UPDATING.md, S2): our tests live in host/, test pure
# functions only, and never touch upstream files or the fork's tests/ tree.
#
# Three checks per vendored engine:
#   1. no `#[test]` inside a `── <name> patch ──` block of an upstream file
#   2. no PdfDocument / DocumentEditor inside a host/ `#[cfg(test)]` module
#   3. no file of ours added under tests/ (compared with the base tag the
#      patches branch is named after: pdf_manipulator/<base>-patches)
set -euo pipefail

cd "$(dirname "$0")/.."

bad=0

check_engine() {
  local dir=$1 marker=$2 host=$3
  [ -d "$dir/src" ] || return 0

  # 1. tests inside patch blocks of upstream files
  while IFS= read -r file; do
    case "$file" in "$dir/src/$host/"*) continue ;; esac
    if awk -v m="$marker" '
        index($0, "── " m " patch ──") { inside = 1 }
        index($0, "── end " m " patch ──") { inside = 0 }
        inside && /#\[test\]/ { found = 1 }
        END { exit found ? 0 : 1 }' "$file"; then
      echo "$file: a test inside a patch block — prove it from test/ops/ instead"
      bad=1
    fi
  done < <(grep -rl -- "$marker patch" "$dir/src" 2>/dev/null || true)

  # 2. document-level types inside host/ test modules. A plain #[cfg(test)]
  #    module is pure; a feature-negated test module is a trim probe run by
  #    tool/shake_audit.sh (which selects tests by the substring trim_probe),
#    allowed a minimal PDF, and must carry trim_probe in its module name.
  while IFS= read -r file; do
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
      echo "$file: a host/ test opens a document, or a feature-negated test module is not a trim_probe — consumer-visible behaviour belongs in test/ops/"
      bad=1
    fi
  done < <(grep -rl '#\[cfg(.*test' "$dir/src/$host" 2>/dev/null || true)

  # 3. files of ours under tests/
  local branch base
  branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  base=${branch#*/}
  base=v${base%-patches}
  if git -C "$dir" rev-parse -q --verify "refs/tags/$base" >/dev/null 2>&1; then
    local added
    added=$(git -C "$dir" diff --name-only --diff-filter=A "$base" -- tests 2>/dev/null || true)
    if [ -n "$added" ]; then
      echo "$dir: test files added under tests/ since $base:"
      printf '  %s\n' $added
      bad=1
    fi
  fi
}

check_engine vendor/pdf_oxide pdf_manipulator host
check_engine vendor/office_oxide office_kit host

if [ "$bad" -ne 0 ]; then
  echo
  echo "docs/UPDATING.md, S2: host/ may unit-test pure functions; every other"
  echo "proof is a Dart battery in test/ops/ over a fixture."
  exit 1
fi
