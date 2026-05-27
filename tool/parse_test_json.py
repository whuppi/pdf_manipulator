#!/usr/bin/env python3
"""Stream dart test --reporter json. Live progress, full diagnostics at end."""
import sys, json

tests = {}
errors = {}
prints = {}
suite_end = 0

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        ev = json.loads(line)
    except json.JSONDecodeError:
        # Build hook output, compilation messages — pass through
        sys.stderr.write(line + "\n")
        sys.stderr.flush()
        continue

    etype = ev.get("type")
    ts = ev.get("time", 0)

    if etype == "testStart":
        t = ev.get("test", {})
        tid = t["id"]
        tests[tid] = {
            "name": t.get("name", "?"),
            "file": t.get("url", ""),
            "line": t.get("line"),
            "start_ms": ts,
            "end_ms": None,
            "result": None,
            "duration_ms": None,
        }

    elif etype == "testDone":
        tid = ev.get("testID")
        if tid in tests:
            tests[tid]["end_ms"] = ts
            tests[tid]["result"] = ev.get("result", "?")
            tests[tid]["duration_ms"] = ts - tests[tid]["start_ms"]

            done = sum(1 for t in tests.values() if t["result"] is not None)
            fail = sum(1 for t in tests.values() if t["result"] == "failure")
            total = len(tests)
            ms = tests[tid]["duration_ms"]
            name = tests[tid]["name"]

            if ev.get("result") == "failure":
                sys.stdout.write(f"\033[31m  ✗ {name} ({ms}ms)\033[0m\n")
            else:
                sys.stdout.write(f"\r  \033[2m{done}/{total}\033[0m  {name} ({ms}ms)          \n")
            sys.stdout.flush()

    elif etype == "error":
        tid = ev.get("testID")
        errors.setdefault(tid, []).append({
            "message": ev.get("error", ""),
            "stack": ev.get("stackTrace", ""),
        })

    elif etype == "print":
        tid = ev.get("testID")
        prints.setdefault(tid, []).append(ev.get("message", ""))

    elif etype == "done":
        suite_end = ts
        break

# Results
passed = [t for t in tests.values() if t["result"] == "success"]
failed = [t for t in tests.values() if t["result"] == "failure"]
errored = [t for t in tests.values() if t["result"] == "error"]
total_s = suite_end / 1000.0

print()
print(f"{'='*70}")
print(f"  PASSED: {len(passed)}   FAILED: {len(failed)}   ERRORS: {len(errored)}   TIME: {total_s:.1f}s")
print(f"{'='*70}")

# Failures
if failed:
    print("\n\033[31mFAILURES:\033[0m")
    for t in failed:
        tid = [k for k, v in tests.items() if v is t][0]
        print(f"\n  ✗ {t['name']}")
        print(f"    {t['duration_ms']}ms  {t.get('file', '')}:{t.get('line', '?')}")
        for err in errors.get(tid, []):
            for el in err["message"].split("\n")[:5]:
                print(f"    {el}")

# Slowest 10
print(f"\n{'─'*70}")
print("SLOWEST:")
by_time = sorted(tests.values(), key=lambda t: t.get("duration_ms") or 0, reverse=True)
for t in by_time[:10]:
    ms = t.get("duration_ms") or 0
    bar = "█" * min(40, ms // 200)
    print(f"  {ms:6d}ms {bar} {t['name']}")

# Group timing
print(f"\n{'─'*70}")
print("GROUPS:")
gtimes = {}
for t in tests.values():
    g = t["name"].split(" ")[0] if " " in t["name"] else "other"
    gtimes.setdefault(g, {"ms": 0, "n": 0})
    gtimes[g]["ms"] += t.get("duration_ms") or 0
    gtimes[g]["n"] += 1
for g in sorted(gtimes, key=lambda g: gtimes[g]["ms"], reverse=True):
    s = gtimes[g]["ms"] / 1000
    print(f"  {s:6.1f}s  ({gtimes[g]['n']:3d} tests)  {g}")

print()
sys.exit(1 if failed else 0)
