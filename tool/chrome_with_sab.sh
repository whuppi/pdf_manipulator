#!/usr/bin/env bash
# Launches Chrome with --enable-features=SharedArrayBuffer.
#
# flutter drive launches TWO Chromes: one for the app (via ChromeDevice)
# and one for the test driver (via chromedriver). --web-browser-flag only
# reaches the chromedriver Chrome. WebDriverService.start() creates fresh
# DebuggingOptions that drop webBrowserFlags and webCrossOriginIsolation
# (flutter/flutter packages/flutter_tools/lib/src/drive/web_driver_service.dart
# lines 86-98 on stable 3.44). The app Chrome never gets the flag.
#
# Setting CHROME_EXECUTABLE to this script makes ChromeDevice launch Chrome
# with SAB enabled, so Atomics mode works in flutter drive integration tests.

# Find Chrome binary.
# CHROME_PATH env var (set by setup-chrome GitHub Action) takes priority.
# Falls back to auto-detection for local dev.
if [[ -n "${CHROME_PATH:-}" ]] && [[ -x "$CHROME_PATH" ]]; then
  CHROME="$CHROME_PATH"
elif [[ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]]; then
  CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
elif command -v google-chrome-stable &>/dev/null; then
  CHROME="google-chrome-stable"
elif command -v google-chrome &>/dev/null; then
  CHROME="google-chrome"
elif command -v chromium-browser &>/dev/null; then
  CHROME="chromium-browser"
elif command -v chromium &>/dev/null; then
  CHROME="chromium"
elif [[ -x "/c/Program Files/Google/Chrome/Application/chrome.exe" ]]; then
  CHROME="/c/Program Files/Google/Chrome/Application/chrome.exe"
elif [[ -x "$LOCALAPPDATA/Google/Chrome/Application/chrome.exe" ]]; then
  CHROME="$LOCALAPPDATA/Google/Chrome/Application/chrome.exe"
else
  echo "chrome_with_sab.sh: Chrome not found" >&2
  exit 1
fi

# CI Linux: match the flags chromedriver uses in getDesiredCapabilities()
# (flutter_tools/lib/src/drive/web_driver_service.dart) so the app Chrome
# starts as fast and cleanly as the test Chrome.
#
# Without these, the app Chrome is slow to load the Dart app. DWDS scans
# tabs for window["$dartAppInstanceId"] with a 50ms gap timeout — if the
# app hasn't set that global yet, DWDS throws AppConnectionException.
# These flags eliminate startup overhead (first-run dialogs, extensions,
# background networking, GPU init) so Chrome loads the page faster.
CI_FLAGS=""
if [[ "$(uname)" == "Linux" ]] && [[ "${CI:-}" == "true" || "$(id -u)" == "0" ]]; then
  CI_FLAGS="--no-sandbox --disable-gpu --disable-dev-shm-usage"
  CI_FLAGS+=" --headless=new"
  CI_FLAGS+=" --disable-background-timer-throttling"
  CI_FLAGS+=" --disable-extensions"
  CI_FLAGS+=" --disable-popup-blocking"
  CI_FLAGS+=" --disable-translate"
  CI_FLAGS+=" --no-default-browser-check"
  CI_FLAGS+=" --no-first-run"
  CI_FLAGS+=" --disable-default-apps"
  CI_FLAGS+=" --disable-sync"
  CI_FLAGS+=" --disable-background-networking"
  CI_FLAGS+=" --disable-hang-monitor"
  CI_FLAGS+=" --disable-component-extensions-with-background-pages"
  CI_FLAGS+=" --disable-prompt-on-repost"
fi

exec "$CHROME" --enable-features=SharedArrayBuffer $CI_FLAGS "$@"
