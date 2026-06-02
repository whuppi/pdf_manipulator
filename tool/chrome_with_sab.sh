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

# Find Chrome binary — macOS, Linux, Windows (Git Bash)
if [[ -x "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" ]]; then
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

# CI Linux runners need --no-sandbox (Chrome refuses to start as root/in containers without it)
CI_FLAGS=""
if [[ "$(uname)" == "Linux" ]] && [[ "${CI:-}" == "true" || "$(id -u)" == "0" ]]; then
  CI_FLAGS="--no-sandbox --disable-gpu"
fi

exec "$CHROME" --enable-features=SharedArrayBuffer $CI_FLAGS "$@"
