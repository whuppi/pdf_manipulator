#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# reconcile_test_json.sh — the single, platform-agnostic test-run verdict.
#
# Every surface (SDK, web, and all integration platforms) decides pass/fail
# the SAME way: from the `*test --file-reporter json:` report, not from a
# flaky exit code or a console tally. One rule, every platform.
#
# THE RULE — a started test PASSES iff:
#     its JSON result is "success"   OR   its body printed a pass marker.
#   It FAILS otherwise (a non-success result with no marker), and a started
#   test that never reported a result is INCOMPLETE (a crash mid-body).
#
# Why the marker override exists: on the Android emulator a teardown watchdog
# kill -9s a DDS that dispose() hung on (flutter#187984/#187785). That dirties
# the exit code even on a clean pass AND marks those tests result:"error" —
# because the *teardown*, not the test, failed. The body still passed, so a
# dispose-on-DDS teardown flake is infra, not a product failure. The marker is
# the body-pass signal: the Android device reporter prints "✅ <name>" when a
# test body finishes. Desktop/SDK use the "+N:" compact reporter (no marker)
# and have no watchdog, so there the result is trustworthy and the override is
# simply never engaged — pass no console log and the rule falls through to it.
#
# Truncation-robust: a crash mid-suite leaves a testStart with no testDone →
# caught as INCOMPLETE. Does NOT require the final "done" event (the watchdog
# kill can eat it), and never trusts its success flag — the per-test verdict
# above is the truth. Pure awk/grep (no jq/python). No apostrophes in awk
# comments: the program is single-quoted, so one would end the string.
#
# Usage:  reconcile_test_json.sh <report.json> [console.log]
# Exit:   0 = every started test passed (success or body-pass marker)
#         1 = real failure(s) or an incomplete (crashed) run
#         2 = no usable report (missing/empty/no test events)
#         3 = reporter format drift (testDone events seen but no ids parsed)
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

REPORT="${1:?usage: reconcile_test_json.sh <report.json> [console.log]}"
CONSOLE="${2:-}"

if [ ! -s "$REPORT" ]; then
  echo "❌ no test report at '$REPORT' (missing or empty) — cannot confirm the run."
  exit 2
fi

# Body-pass names: lines that printed "✅ <name>", marker + trailing space
# stripped, one per line. Empty when no console log is supplied (every surface
# that lacks the marker), which leaves the rule reading the JSON result alone.
BODYPASS="$(mktemp)"
trap 'rm -f "$BODYPASS"' EXIT
if [ -n "$CONSOLE" ] && [ -s "$CONSOLE" ]; then
  sed -n 's/^✅ //p' "$CONSOLE" | sed 's/[[:space:]]*$//' > "$BODYPASS"
fi

# Two input files: the body-pass names first (loaded while NR==FNR), then the
# JSON report. Each JSON field is matched then stripped to its value, so
# extraction survives whitespace or key-order changes in a future reporter.
awk -v BPF="$BODYPASS" '
  FILENAME == BPF { if (length($0)) bodypass[$0] = 1; next }

  /"type":"testStart"/ {
    if (match($0, /"test":[[:space:]]*[{][[:space:]]*"id":[[:space:]]*[0-9]+/)) {
      id = substr($0, RSTART, RLENGTH); gsub(/[^0-9]/, "", id)
      started[id] = 1
      if (match($0, /"name":"[^"]*"/)) {
        nm = substr($0, RSTART, RLENGTH); sub(/^"name":"/, "", nm); sub(/"$/, "", nm)
        name_of[id] = nm
      }
    }
    next
  }
  /"type":"testDone"/ {
    id = ""; res = ""
    if (match($0, /"testID":[[:space:]]*[0-9]+/)) {
      id = substr($0, RSTART, RLENGTH); gsub(/[^0-9]/, "", id)
    }
    if (match($0, /"result":[[:space:]]*"[a-z]+"/)) {
      res = substr($0, RSTART, RLENGTH); sub(/^"result":[[:space:]]*"/, "", res); sub(/"$/, "", res)
    }
    if (id != "") { done[id] = 1; result_of[id] = res }
    ndone++
    next
  }
  /"type":"done"/ {
    if ($0 ~ /"success":false/) donefail = 1
    sawdone = 1
  }
  END {
    real_fails = 0; incomplete = 0
    for (id in started) {
      passed = (result_of[id] == "success") || (name_of[id] in bodypass)
      if (passed) continue
      if (id in done) {
        real_fails++
        failids = failids " #" id (length(name_of[id]) ? " (" name_of[id] ")" : "")
      } else {
        incomplete++; incids = incids " #" id
      }
    }
    printf "── TEST-RESULT RECONCILER ──\n"
    printf "started=%d  done=%d  body_passed=%d  failures=%d  incomplete=%d  done_event=%s\n", \
           count_keys(started), ndone + 0, count_keys(bodypass), real_fails, incomplete, \
           (sawdone ? (donefail ? "fail" : "ok") : "absent")
    if (ndone + 0 == 0) {
      print "❌ report contained no testDone events — no tests ran."
      exit 2
    }
    # Wholesale format-drift guard. Truncation corrupts at most the final line,
    # so testDone events with not one parsed id means the reporter JSON shape
    # changed and the extractor went blind — fail loud rather than green a run
    # we can no longer read. (One bad line stays an incomplete, not this.)
    if (count_keys(started) == 0 && count_keys(done) == 0) {
      print "❌ reporter format drift — testDone events but no test ids parsed; update the extractor."
      exit 3
    }
    if (real_fails > 0) {
      print "❌ real test failure(s):" failids
      exit 1
    }
    if (incomplete > 0) {
      print "❌ incomplete run — started tests with no pass marker and no result (crash mid-suite):" incids
      exit 1
    }
    printf "🎉 %d tests passed (success or body-pass; teardown-only flakes ignored).\n", count_keys(started)
    exit 0
  }
  function count_keys(a,   k, n) { n = 0; for (k in a) n++; return n }
' "$BODYPASS" "$REPORT"
