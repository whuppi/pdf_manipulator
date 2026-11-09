#!/usr/bin/env bash
# Validate PDF fixtures used by the pdf_oxide benchmark / regression
# harness. Every file under `fixtures/` with a `.pdf` extension must
# start with the `%PDF-` magic header. Anything else is either a
# corrupted download (ISP DNS hijack, CMS wrapper, 404 page) or a
# polyglot file that confuses downstream tooling.
#
# See https://github.com/yfedoseev/pdf_oxide/issues/16 (tracking) for
# the corruption modes observed in the Kreuzberg corpus during v0.3.38
# regression validation. The canonical skip-list is `.skip` next to
# each fixture directory.
#
# Usage:
#   ./tools/benchmark-harness/validate_fixtures.sh           # scan + report
#   ./tools/benchmark-harness/validate_fixtures.sh --strict  # exit 1 on any bad fixture NOT in .skip
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES="$ROOT/fixtures"
STRICT=0
[ "${1:-}" = "--strict" ] && STRICT=1

if [ ! -d "$FIXTURES" ]; then
    echo "validate_fixtures: no fixtures dir at $FIXTURES" >&2
    exit 0
fi

load_skip() {
    local skip_file="$1"
    [ -f "$skip_file" ] || return 0
    grep -vE '^\s*(#|$)' "$skip_file" | awk '{print $1}'
}

total=0
bad=0
unexpected=0
while IFS= read -r -d '' pdf; do
    total=$((total+1))
    header=$(head -c 5 "$pdf" 2>/dev/null || true)
    if [ "$header" != "%PDF-" ]; then
        rel="${pdf#$FIXTURES/}"
        dir="$(dirname "$pdf")"
        skip_list="$(load_skip "$dir/.skip")"
        if echo "$skip_list" | grep -qxF "$(basename "$pdf")"; then
            echo "  skip: $rel (known-bad, listed in .skip)"
            bad=$((bad+1))
        else
            echo "  BAD:  $rel — header=[$(head -c 40 "$pdf" | tr -cd '[:print:]' | cut -c1-40)]"
            unexpected=$((unexpected+1))
        fi
    fi
done < <(find "$FIXTURES" -type l -o -type f \) -name '*.pdf' -print0 2>/dev/null)

# NB: find's grouping for `-type l -o -type f` needs parens; the -o
# shortcut above is a bash limitation workaround.
# Rerun with the proper grouped form if the first pass found zero
# files (legacy platforms).
if [ "$total" -eq 0 ]; then
    while IFS= read -r -d '' pdf; do
        total=$((total+1))
        header=$(head -c 5 "$pdf" 2>/dev/null || true)
        if [ "$header" != "%PDF-" ]; then
            rel="${pdf#$FIXTURES/}"
            dir="$(dirname "$pdf")"
            skip_list="$(load_skip "$dir/.skip")"
            if echo "$skip_list" | grep -qxF "$(basename "$pdf")"; then
                echo "  skip: $rel (known-bad, listed in .skip)"
                bad=$((bad+1))
            else
                echo "  BAD:  $rel — header=[$(head -c 40 "$pdf" | tr -cd '[:print:]' | cut -c1-40)]"
                unexpected=$((unexpected+1))
            fi
        fi
    done < <(find -L "$FIXTURES" -type f -name '*.pdf' -print0)
fi

echo ""
echo "fixtures scanned: $total"
echo "known-bad (skip-listed): $bad"
echo "unexpected-bad: $unexpected"

if [ "$STRICT" -eq 1 ] && [ "$unexpected" -gt 0 ]; then
    echo ""
    echo "STRICT: fail because $unexpected fixture(s) are not valid PDFs and not in any .skip." >&2
    echo "Add them to the nearest .skip file with a comment explaining why," >&2
    echo "or re-fetch the upstream source." >&2
    exit 1
fi
