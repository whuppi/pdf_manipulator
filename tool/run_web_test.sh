#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# run_web_test.sh — Run one flutter drive web integration test.
#
# Starts chromedriver, runs flutter drive in the background, polls
# the log for completion, then kills everything cleanly. Exists
# because flutter drive on web hangs after tests finish (Chrome
# FocusManager disposal bug keeps the browser process alive).
#
# Usage:
#   run_web_test.sh <mode> <chrome_wrapper> <flutter_cmd...>
#
#   mode            jspi | atomics | opfs
#   chrome_wrapper  path to chrome_with_sab.sh (adds SharedArrayBuffer flags)
#   flutter_cmd     e.g. "fvm flutter"
#
# Called by:  Makefile targets (test-example-web-*)
# Run from:   package root
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

MODE="$1"
CHROME_WRAPPER="$2"
shift 2
FLUTTER=("$@")

LOG="/tmp/_pdf_web_test.log"
: > "$LOG"


# ═══════════════════════════════════════════════════════════════════
# 1. Start chromedriver
# ═══════════════════════════════════════════════════════════════════

chromedriver --port=4444 &>/dev/null &
CD_PID=$!
sleep 2


# ═══════════════════════════════════════════════════════════════════
# 2. Run flutter drive (background)
# ═══════════════════════════════════════════════════════════════════

cd example
CHROME_EXECUTABLE="$CHROME_WRAPPER" "${FLUTTER[@]}" drive \
    --driver=test_driver/integration_test.dart \
    --target=integration_test/pdf_smoke_test.dart \
    --dart-define=PDF_IO_MODE="$MODE" \
    -d chrome &>"$LOG" &
DRIVE_PID=$!


# ═══════════════════════════════════════════════════════════════════
# 3. Poll log for terminal line
# ═══════════════════════════════════════════════════════════════════

while kill -0 "$DRIVE_PID" 2>/dev/null; do
    if grep -q 'All tests passed\|Application finished' "$LOG" 2>/dev/null; then
        break
    fi
    sleep 0.3
done


# ═══════════════════════════════════════════════════════════════════
# 4. Cleanup — kill flutter drive + chromedriver + orphaned Chrome
# ═══════════════════════════════════════════════════════════════════

kill "$DRIVE_PID" 2>/dev/null; wait "$DRIVE_PID" 2>/dev/null
kill "$CD_PID"    2>/dev/null; wait "$CD_PID"    2>/dev/null
pkill -f 'flutter_tools_chrome_device' 2>/dev/null || true

rm -f flutter_*.log


# ═══════════════════════════════════════════════════════════════════
# 5. Report result
# ═══════════════════════════════════════════════════════════════════

if grep -q 'All tests passed' "$LOG"; then
    echo "=== Example web $MODE: All tests passed ==="
    exit 0
else
    echo "=== Example web $MODE: FAILED ==="
    cat "$LOG"
    exit 1
fi
