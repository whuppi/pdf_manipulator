#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# reconcile_test_json.sh — the single source of test-run truth.
#
# Decides pass/fail from a `dart test --file-reporter json:` report,
# never from console pass/fail marks or an exit code. The JSON reporter
# flushes one `testDone` event per test AS IT FINISHES, so the per-test
# verdict is on disk BEFORE any teardown hang or watchdog kill -9 can
# sever the console tally or the exit code (flutter#187984, #187785).
#
# Three things a console parse gets wrong, that this gets right:
#   - a real suite-load failure is a JSON `error` result, structurally
#     distinct from the spurious golden-comparator "loading (failed)"
#     line — no blanket grep -v that masks real load failures.
#   - a crash mid-suite leaves a `testStart` with no matching
#     `testDone` → caught as INCOMPLETE (never reported green).
#   - it does NOT require the final `done` event, which the teardown
#     kill can eat even on a clean pass.
#
# Pure awk/grep (no jq/python/dart). Events are newline-delimited, one
# JSON object per line.
#
# Usage:   reconcile_test_json.sh <report.json>
# Exit:    0 = every started test finished success
#          1 = real failure(s) or an incomplete (crashed) run
#          2 = no usable report (missing/empty/no test events)
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

REPORT="${1:?usage: reconcile_test_json.sh <report.json>}"

if [ ! -s "$REPORT" ]; then
  echo "❌ no test report at '$REPORT' (missing or empty) — cannot confirm the run."
  exit 2
fi

# One awk pass. Offsets are computed from the fixed key prefixes:
#   "test":{"id":   = 13 chars   "testID":  = 9 chars   "result":" = 10 chars
# IDs are integers and result is a lowercase word, so fixed-width
# substr extraction is exact and portable (no gawk capture groups).
awk '
  /"type":"testStart"/ {
    if (match($0, /"test":[{]"id":[0-9]+/)) {
      id = substr($0, RSTART + 13, RLENGTH - 13)
      started[id] = 1
    }
    next
  }
  /"type":"testDone"/ {
    id = ""; res = ""
    if (match($0, /"testID":[0-9]+/))     id  = substr($0, RSTART + 9,  RLENGTH - 9)
    if (match($0, /"result":"[a-z]+"/))   res = substr($0, RSTART + 10, RLENGTH - 11)
    if (id != "") done[id] = 1
    ndone++
    if (res == "failure" || res == "error") {
      fails++
      # the test name was on its testStart; surface the id either way
      failids = failids " #" id
    }
    next
  }
  /"type":"done"/ {
    if ($0 ~ /"success":false/) donefail = 1
    sawdone = 1
  }
  END {
    incomplete = 0
    for (s in started) if (!(s in done)) { incomplete++; incids = incids " #" s }
    printf "── TEST-RESULT RECONCILER ──\n"
    printf "started=%d  done=%d  failures=%d  incomplete=%d  done_event=%s\n", \
           nstart_count(started), ndone + 0, fails + 0, incomplete, (sawdone ? (donefail ? "fail" : "ok") : "absent")
    if (ndone + 0 == 0) {
      print "❌ report contained no testDone events — no tests ran."
      exit 2
    }
    if (fails + 0 > 0) {
      print "❌ real test failure(s):" failids
      exit 1
    }
    if (incomplete > 0) {
      print "❌ incomplete run — started tests with no result (crash mid-suite):" incids
      exit 1
    }
    if (donefail) {
      print "❌ run reported done:success=false."
      exit 1
    }
    printf "🎉 %d tests passed (every started test finished success).\n", ndone + 0
    exit 0
  }
  function nstart_count(a,   k, n) { n = 0; for (k in a) n++; return n }
' "$REPORT"
