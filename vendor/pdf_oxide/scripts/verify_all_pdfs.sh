#!/bin/bash
# Verify pdf_oxide against all PDFs in a directory tree.
# Usage: ./scripts/verify_all_pdfs.sh <pdf_directory>
#
# Runs extract_text_simple on every PDF and reports pass/fail/panic statistics.

set -euo pipefail

PDF_DIR="${1:?Usage: $0 <pdf_directory>}"
BINARY="./target/release/examples/extract_text_simple"

if [ ! -f "$BINARY" ]; then
    echo "Building release examples..."
    cargo build --release --examples 2>&1 | tail -3
fi

TOTAL=0
PASS=0
FAIL=0
PANIC=0
TIMEOUT=0

FAIL_LIST=$(mktemp)
PANIC_LIST=$(mktemp)
TIMEOUT_LIST=$(mktemp)

trap "rm -f $FAIL_LIST $PANIC_LIST $TIMEOUT_LIST" EXIT

echo "Scanning for PDFs in: $PDF_DIR"
PDF_FILES=$(fd -e pdf -e PDF --type f . "$PDF_DIR")
PDF_COUNT=$(echo "$PDF_FILES" | wc -l)
echo "Found $PDF_COUNT PDF files"
echo "Starting verification..."
echo ""

while IFS= read -r pdf; do
    TOTAL=$((TOTAL + 1))
    BASENAME=$(basename "$pdf")

    # Run with 30 second timeout, capture stderr
    OUTPUT=$(timeout 30 "$BINARY" "$pdf" 2>&1 >/dev/null) && RC=$? || RC=$?

    if [ $RC -eq 0 ]; then
        PASS=$((PASS + 1))
    elif [ $RC -eq 124 ]; then
        TIMEOUT=$((TIMEOUT + 1))
        echo "$pdf" >> "$TIMEOUT_LIST"
        echo "  TIMEOUT: $BASENAME"
    elif echo "$OUTPUT" | grep -q "panic"; then
        PANIC=$((PANIC + 1))
        echo "$pdf" >> "$PANIC_LIST"
        echo "  PANIC: $BASENAME"
    else
        FAIL=$((FAIL + 1))
        echo "$pdf" >> "$FAIL_LIST"
    fi

    # Progress every 100 files
    if [ $((TOTAL % 100)) -eq 0 ]; then
        echo "  [$TOTAL/$PDF_COUNT] pass=$PASS fail=$FAIL panic=$PANIC timeout=$TIMEOUT"
    fi
done <<< "$PDF_FILES"

echo ""
echo "========================================="
echo "  VERIFICATION RESULTS"
echo "========================================="
echo "  Total:    $TOTAL"
echo "  Pass:     $PASS"
echo "  Fail:     $FAIL (parse/extraction errors)"
echo "  Panic:    $PANIC (crashes)"
echo "  Timeout:  $TIMEOUT (>30s)"
echo "========================================="

PASS_RATE=$(awk "BEGIN {printf \"%.1f\", ($PASS/$TOTAL)*100}")
echo "  Pass rate: $PASS_RATE%"
echo ""

if [ $PANIC -gt 0 ]; then
    echo "PANICS (crashes — these are bugs):"
    cat "$PANIC_LIST" | while read -r f; do echo "  - $f"; done
    echo ""
fi

if [ $TIMEOUT -gt 0 ]; then
    echo "TIMEOUTS (>30s — potential infinite loops):"
    cat "$TIMEOUT_LIST" | while read -r f; do echo "  - $f"; done
    echo ""
fi

if [ $FAIL -gt 0 ] && [ $FAIL -le 50 ]; then
    echo "FAILURES (first 50):"
    head -50 "$FAIL_LIST" | while read -r f; do echo "  - $(basename "$f")"; done
    echo ""
fi

# Exit with error if any panics or timeouts
if [ $PANIC -gt 0 ] || [ $TIMEOUT -gt 0 ]; then
    exit 1
fi
