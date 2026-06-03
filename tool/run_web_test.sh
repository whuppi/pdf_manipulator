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
# Retries once on AppConnectionException (Flutter DWDS bug #181357).
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
MAX_ATTEMPTS=2


# ═══════════════════════════════════════════════════════════════════
# Run one attempt — returns 0 on pass, 1 on fail
# ═══════════════════════════════════════════════════════════════════

run_one_attempt() {
  : > "$LOG"

  # Ensure port is free (stale chromedriver from a killed previous run)
  if lsof -ti ":$PORT" &>/dev/null; then
    echo "Port $PORT in use — killing stale process"
    lsof -ti ":$PORT" | xargs kill -9 2>/dev/null || true
    sleep 1
  fi

  chromedriver --port="$PORT" &>/dev/null &
  local cd_pid=$!
  sleep 2

  cd example
  CHROME_EXECUTABLE="$CHROME_WRAPPER" "${FLUTTER[@]}" drive \
      --driver=test_driver/integration_test.dart \
      --target=integration_test/pdf_smoke_test.dart \
      --dart-define=PDF_IO_MODE="$MODE" \
      --driver-port="$PORT" \
      -d chrome &>"$LOG" &
  local drive_pid=$!

  # Poll log for terminal line (5-min timeout prevents infinite hang)
  local timeout=300 elapsed=0
  while kill -0 "$drive_pid" 2>/dev/null; do
      if grep -qE 'All tests passed|Application finished' "$LOG" 2>/dev/null; then
          break
      fi
      if [ "$elapsed" -ge "$timeout" ]; then
          echo "=== TIMEOUT: $MODE produced no result after ${timeout}s ==="
          echo "Last 20 lines of log:"
          tail -20 "$LOG"
          break
      fi
      sleep 1
      elapsed=$((elapsed + 1))
  done

  # Cleanup
  kill "$drive_pid" 2>/dev/null; wait "$drive_pid" 2>/dev/null
  kill "$cd_pid"    2>/dev/null; wait "$cd_pid"    2>/dev/null
  pkill -f 'flutter_tools_chrome_device' 2>/dev/null || true
  rm -f flutter_*.log
  cd ..

  if grep -q 'All tests passed' "$LOG"; then
    return 0
  else
    return 1
  fi
}


# ═══════════════════════════════════════════════════════════════════
# Main — run with retry on AppConnectionException
# ═══════════════════════════════════════════════════════════════════
# Flutter DWDS bug #181357: AppConnectionException is non-deterministic.
# Chrome's debug port occasionally fails to connect on CI. One retry
# is enough — if it fails twice, it's a real problem.

for attempt in $(seq 1 $MAX_ATTEMPTS); do
  if run_one_attempt; then
    echo "=== Example web $MODE: All tests passed ==="
    exit 0
  fi

  if [ "$attempt" -lt "$MAX_ATTEMPTS" ] && grep -q 'AppConnectionException' "$LOG"; then
    echo "=== $MODE attempt $attempt failed (AppConnectionException) — retrying ==="
    sleep 3
  fi
done

echo "=== Example web $MODE: FAILED ==="
cat "$LOG"
exit 1
