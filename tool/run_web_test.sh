#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# run_web_test.sh — Run one flutter drive web integration test.
#
# Uses -d web-server: Flutter serves the app, chromedriver manages
# one Chrome instance. No DWDS, no dual-Chrome, no AppConnection
# race. SharedArrayBuffer enabled via --web-browser-flag.
#
# Usage:
#   run_web_test.sh <mode> <flutter_cmd...>
#
#   mode         jspi | atomics | opfs
#   flutter_cmd  e.g. "fvm flutter"
#
# Called by:  Makefile targets (test-example-web-*)
# Run from:   package root
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

MODE="$1"
shift
FLUTTER=("$@")

LOG="/tmp/_pdf_web_test.log"
: > "$LOG"


# ═══════════════════════════════════════════════════════════════════
# 1. Start chromedriver
# ═══════════════════════════════════════════════════════════════════

chromedriver --port=4444 &>/dev/null &
CD_PID=$!
# Wait for chromedriver to bind :4444 by polling its status endpoint, not a
# fixed sleep that races on a loaded runner.
for _ in $(seq 1 50); do
  curl -fsS --max-time 1 http://127.0.0.1:4444/status >/dev/null 2>&1 && break
  sleep 0.2
done


# ═══════════════════════════════════════════════════════════════════
# 2. Run flutter drive (-d web-server, single Chrome via chromedriver)
# ═══════════════════════════════════════════════════════════════════

cd example || exit 1
"${FLUTTER[@]}" drive \
    --driver=test_driver/integration_test.dart \
    --target=integration_test/pdf_smoke_test.dart \
    --dart-define=PDF_IO_MODE="$MODE" \
    -d web-server \
    --browser-name=chrome \
    --driver-port=4444 \
    --web-browser-flag=--enable-features=SharedArrayBuffer \
    --web-browser-flag=--no-sandbox 2>&1 | tee "$LOG" &
DRIVE_PID=$!


# ═══════════════════════════════════════════════════════════════════
# 3. Poll log for terminal line
# ═══════════════════════════════════════════════════════════════════

while kill -0 "$DRIVE_PID" 2>/dev/null; do
    if grep -qE 'All tests passed|Application finished' "$LOG" 2>/dev/null; then
        break
    fi
    sleep 0.3
done


# ═══════════════════════════════════════════════════════════════════
# 4. Cleanup
# ═══════════════════════════════════════════════════════════════════

kill "$DRIVE_PID" 2>/dev/null; wait "$DRIVE_PID" 2>/dev/null
kill "$CD_PID"    2>/dev/null; wait "$CD_PID"    2>/dev/null

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
