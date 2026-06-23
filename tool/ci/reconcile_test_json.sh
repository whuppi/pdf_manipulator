#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# reconcile_test_json.sh — the Android-emulator CI test verdict.
#
# CI-only, and ONLY for the Android emulator. Every other surface (SDK,
# desktop, iOS, web) trusts its own exit code or driver — nothing to reconcile.
# The emulator is the lone exception: on a SwiftShader (software-GPU) emulator,
# which is the macOS-Intel runner, a teardown watchdog kill -9s a DDS that
# dispose() hung on (flutter#187984/#187785). That dirties the exit code even
# on a clean pass AND marks those tests result:"error" — the *teardown*, not
# the test, failed. So neither the exit code nor the JSON result alone is
# trusted: this reads the report for structure plus the captured console for
# body-pass marks, and decides.
#
# THE RULE — a started test PASSES iff:
#     its JSON result is "success"   OR   its body printed a pass marker.
#   It FAILS otherwise (a non-success result with no marker); a started test
#   that never reported a result is INCOMPLETE (a crash mid-body).
#
#   Exception for the suite teardown: a failed (tearDownAll)/(tearDown) step has
#   no body, so no marker. When the watchdog fired (it prints "WATCHDOG:"), that
#   step is the kill severing the suite teardown — reported, non-fatal. With no
#   watchdog kill it stays a real failure, and a failed setUp step always does.
#
# The marker is the body-pass signal: the Android device reporter prints
# "✅ <name>" when a body finishes, so a passed body overrides a teardown-killed
# result. The console arg is optional (the rule still works on the JSON result
# alone) — defensive only; the Android caller always passes it.
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
# WATCHDOG_FIRED: emulator-run.sh prints a "WATCHDOG:" line only when it kill -9s
# a stuck DDS. If it fired, a failed *suite* teardown step (tearDownAll/tearDown,
# which has no body and so no pass marker) is that kill severing it — collateral,
# not a real failure. Without a watchdog kill, a teardown failure stays real.
WATCHDOG_FIRED=0
if [ -n "$CONSOLE" ] && [ -s "$CONSOLE" ]; then
  sed -n 's/^✅ //p' "$CONSOLE" | sed 's/[[:space:]]*$//' > "$BODYPASS"
  grep -q 'WATCHDOG:' "$CONSOLE" && WATCHDOG_FIRED=1
fi

# Two input files: the body-pass names first (the FILENAME==BPF block, which is
# empty-file safe — NR==FNR would misread the report as names when no console is
# given), then the JSON report. Each JSON field is matched then stripped to its
# value, so extraction survives whitespace or key-order changes in a future reporter.
awk -v BPF="$BODYPASS" -v WD="$WATCHDOG_FIRED" '
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
    real_fails = 0; incomplete = 0; td_severed = 0
    for (id in started) {
      passed = (result_of[id] == "success") || (name_of[id] in bodypass)
      if (passed) continue
      # A failed (tearDownAll)/(tearDown) step has no body, so no pass marker.
      # When the watchdog fired, that step is the kill severing the suite
      # teardown — collateral, not a real failure. A setUp step or a real test
      # still fails here, and a teardown failure with no watchdog kill stays real.
      if (WD == 1 && name_of[id] ~ /^\(tearDown/) {
        td_severed++; tdids = tdids " #" id (length(name_of[id]) ? " " name_of[id] : "")
        continue
      }
      if (id in done) {
        real_fails++
        failids = failids " #" id (length(name_of[id]) ? " (" name_of[id] ")" : "")
      } else {
        incomplete++; incids = incids " #" id
      }
    }
    printf "── TEST-RESULT RECONCILER ──\n"
    printf "started=%d  done=%d  body_passed=%d  failures=%d  incomplete=%d  teardown_severed=%d  done_event=%s\n", \
           count_keys(started), ndone + 0, count_keys(bodypass), real_fails, incomplete, td_severed, \
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
    if (td_severed > 0) {
      print "ℹ teardown step(s) severed by the watchdog kill — non-fatal:" tdids
    }
    if (real_fails > 0) {
      print "❌ real test failure(s):" failids
      exit 1
    }
    if (incomplete > 0) {
      print "❌ incomplete run — started tests with no pass marker and no result (crash mid-suite):" incids
      exit 1
    }
    printf "🎉 %d tests passed (success or body-pass; watchdog-severed teardowns ignored).\n", count_keys(started) - td_severed
    exit 0
  }
  function count_keys(a,   k, n) { n = 0; for (k in a) n++; return n }
' "$BODYPASS" "$REPORT"
