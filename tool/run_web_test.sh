#!/usr/bin/env bash
# Run a flutter drive web integration test and exit cleanly once tests finish.
# Usage: run_web_test.sh <mode> <chrome_wrapper> <flutter_cmd>
#   mode:           jspi | atomics | opfs
#   chrome_wrapper: path to chrome_with_sab.sh
#   flutter_cmd:    e.g. "fvm flutter"
set -uo pipefail

MODE="$1"
CHROME_WRAPPER="$2"
shift 2
FLUTTER=("$@")

LOG="/tmp/_pdf_web_test.log"
: > "$LOG"

# Start chromedriver, record PID
chromedriver --port=4444 &>/dev/null &
CD_PID=$!
sleep 2

# Run flutter drive in background
cd example
CHROME_EXECUTABLE="$CHROME_WRAPPER" "${FLUTTER[@]}" drive \
    --driver=test_driver/integration_test.dart \
    --target=integration_test/pdf_smoke_test.dart \
    --dart-define=PDF_IO_MODE="$MODE" \
    -d chrome &>"$LOG" &
DRIVE_PID=$!

# Poll the log for the terminal line
while kill -0 "$DRIVE_PID" 2>/dev/null; do
    if grep -q 'All tests passed\|Application finished' "$LOG" 2>/dev/null; then
        break
    fi
    sleep 0.3
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
