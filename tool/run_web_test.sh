#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# run_web_test.sh — Run one flutter drive web integration test.
#
# Starts chromedriver, runs flutter drive in the background, polls
# the log for completion, then kills everything cleanly. Exists
# because flutter drive on web hangs after tests finish (Chrome
# FocusManager disposal bug keeps the browser process alive).
#
# Each mode gets its own chromedriver port and log file — no shared
# state between sequential runs.
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

# Per-mode port — eliminates port reuse race between sequential modes
case "$MODE" in
  jspi)    PORT=4444 ;;
  atomics) PORT=4445 ;;
  opfs)    PORT=4446 ;;
  *)       PORT=4447 ;;
esac

LOG="/tmp/_pdf_web_test_${MODE}.log"
: > "$LOG"


# ═══════════════════════════════════════════════════════════════════
# 1. Start chromedriver
# ═══════════════════════════════════════════════════════════════════

# Ensure port is free (stale chromedriver from a killed previous run)
if lsof -ti ":$PORT" &>/dev/null; then
  echo "Port $PORT in use — killing stale process"
  lsof -ti ":$PORT" | xargs kill -9 2>/dev/null || true
  sleep 1
fi

chromedriver --port="$PORT" &>/dev/null &
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
    --driver-port="$PORT" \
    --profile \
    -d chrome &>"$LOG" &
DRIVE_PID=$!


# ═══════════════════════════════════════════════════════════════════
# 3. Poll log for terminal line (5-min timeout prevents infinite hang)
# ═══════════════════════════════════════════════════════════════════

TIMEOUT=300
ELAPSED=0
while kill -0 "$DRIVE_PID" 2>/dev/null; do
    if grep -qE 'All tests passed|Application finished' "$LOG" 2>/dev/null; then
        break
    fi
    if [ "$ELAPSED" -ge "$TIMEOUT" ]; then
        echo "=== TIMEOUT: $MODE produced no result after ${TIMEOUT}s ==="
        echo "Last 20 lines of log:"
        tail -20 "$LOG"
        break
    fi
    sleep 1
    ELAPSED=$((ELAPSED + 1))
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
