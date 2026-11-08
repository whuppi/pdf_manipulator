#!/usr/bin/env bash
# ios_vm_rescue.sh <flutter-verbose-log> — relaunches the app once when
# `flutter test` on an iOS simulator waits for a VM Service URL that never comes.
#
# flutter_tools learns the app's VM Service URL from one line the app prints
# two seconds after launch, read through `simctl spawn … log stream`. On a cold
# CI Mac that spawn can attach after the line went by; the line is lost and
# the tool waits forever. The app is fine: launched again while the stream
# listens, its line is read and the run goes on. So when the log shows the
# wait for longer than RESCUE_AFTER seconds with no URL, this terminates the
# app and launches it again with the exact arguments flutter_tools used (from
# its verbose `simctl launch` line). It runs in the background beside the test
# and exits when the URL appears, after one rescue, or when the log's writer ends.
set -uo pipefail

log="${1:?usage: ios_vm_rescue.sh <flutter-verbose-log>}"
after="${RESCUE_AFTER:-90}"

waited=0
while :; do
  sleep 5
  [ -f "$log" ] || continue
  grep -q 'VM Service URL on device' "$log" && exit 0
  if grep -q 'Waiting for VM Service port to be available' "$log"; then
    waited=$((waited + 5))
  fi
  [ "$waited" -ge "$after" ] && break
done

launch=$(grep -o 'xcrun simctl launch .*' "$log" | tail -1)
[ -n "$launch" ] || { echo "ios_vm_rescue: no simctl launch line in $log" >&2; exit 0; }
read -r -a args <<< "${launch#xcrun simctl launch }"
device="${args[0]}"
bundle="${args[1]}"
echo "ios_vm_rescue: no VM Service URL after ${after}s; relaunching $bundle (the log stream missed its line)"
xcrun simctl terminate "$device" "$bundle" || true
sleep 2
xcrun simctl launch "${args[@]}"
