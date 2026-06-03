#!/usr/bin/env bash
# Run a flutter drive web integration test and exit cleanly once tests finish.
# Each mode gets its own chromedriver port and log file — no shared state.
#
# Usage: run_web_test.sh <mode> <chrome_wrapper> <flutter_cmd>
#   mode:           jspi | atomics | opfs
#   chrome_wrapper: path to chrome_with_sab.sh
#   flutter_cmd:    e.g. "fvm flutter"
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

# Ensure port is free (stale chromedriver from a killed previous run)
if lsof -ti ":$PORT" &>/dev/null; then
  echo "Port $PORT in use — killing stale process"
  lsof -ti ":$PORT" | xargs kill -9 2>/dev/null || true
  sleep 1
fi

# Start chromedriver on this mode's port
chromedriver --port="$PORT" &>/dev/null &
CD_PID=$!
sleep 2

# Run flutter drive in background, pointing at this mode's chromedriver
cd example
CHROME_EXECUTABLE="$CHROME_WRAPPER" "${FLUTTER[@]}" drive \
    --driver=test_driver/integration_test.dart \
    --target=integration_test/pdf_smoke_test.dart \
    --dart-define=PDF_IO_MODE="$MODE" \
    --driver-port="$PORT" \
    -d chrome &>"$LOG" &
DRIVE_PID=$!

# Poll the log for the terminal line (5-min timeout prevents infinite hang)
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

# Kill flutter drive + chromedriver + orphaned app Chrome
kill "$DRIVE_PID" 2>/dev/null; wait "$DRIVE_PID" 2>/dev/null
kill "$CD_PID" 2>/dev/null; wait "$CD_PID" 2>/dev/null
pkill -f 'flutter_tools_chrome_device' 2>/dev/null || true

rm -f flutter_*.log

# Report result
if grep -q 'All tests passed' "$LOG"; then
    echo "=== Example web $MODE: All tests passed ==="
    exit 0
else
    echo "=== Example web $MODE: FAILED ==="
    cat "$LOG"
    exit 1
fi
