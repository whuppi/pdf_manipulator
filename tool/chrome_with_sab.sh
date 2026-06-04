#!/usr/bin/env bash
# ────────────────────────────────────────────────────────────────────
# chrome_with_sab.sh — Launch Chrome with SharedArrayBuffer enabled.
#
# flutter drive launches TWO Chromes: one for the app (via ChromeDevice)
# and one for the test driver (via chromedriver). --web-browser-flag only
# reaches the chromedriver Chrome. The app Chrome never gets the flag.
#
# Setting CHROME_EXECUTABLE to this script makes ChromeDevice launch
# Chrome with SAB enabled, so Atomics mode works in integration tests.
#
# Called by:  Makefile via CHROME_SAB variable
# ────────────────────────────────────────────────────────────────────


# ═══════════════════════════════════════════════════════════════════
# Find Chrome binary
# ═══════════════════════════════════════════════════════════════════
# CHROME_PATH (set by setup-web-testing CI action) takes priority.
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


# ═══════════════════════════════════════════════════════════════════
# CI flags
# ═══════════════════════════════════════════════════════════════════

CI_FLAGS=""
if [[ "$(uname)" == "Linux" ]] && [[ "${CI:-}" == "true" || "$(id -u)" == "0" ]]; then
  CI_FLAGS="--no-sandbox --disable-gpu"
fi

exec "$CHROME" --enable-features=SharedArrayBuffer $CI_FLAGS "$@"
