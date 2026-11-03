#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# clean_chrome_clones.sh — removes the Chrome code-sign clones that killed
# headless browser launches leave behind on macOS.
#
#   bash tool/clean_chrome_clones.sh
#
# On a launch macOS clones the Chrome app into
# `$(getconf DARWIN_USER_TEMP_DIR)/../X/com.google.Chrome.code_sign_clone/`
# (over a gigabyte each) and Chrome removes the clone when it exits cleanly.
# A launch that is killed leaves it, and the folder grows without bound.
#
# It runs only when no Chrome process runs at all: a clone belongs to a running
# Chrome or to none, so with none running every clone is a leftover and goes.
# Prints how many it removed. Skips on any system but macOS. Always exits 0:
# a cleanup that fails must not fail the test that called it.
# ────────────────────────────────────────────────────────────────────
set -uo pipefail

if [ "$(uname -s)" != Darwin ]; then
  exit 0
fi

# Process names of every Chrome flavour a test can launch: the browser, its
# helpers, Chrome for Testing, Chromium, the headless shell and chromedriver.
# Matches the process name, not the command line, so an Electron app's
# chrome_crashpad_handler does not count and this script's own path does not.
if pgrep -ix 'Google Chrome.*|Chromium.*|chromedriver|chrome|chrome-headless-shell' >/dev/null; then
  echo "chrome clones: a Chrome is running, none removed"
  exit 0
fi

CLONES="$(getconf DARWIN_USER_TEMP_DIR)../X/com.google.Chrome.code_sign_clone"
if [ ! -d "$CLONES" ]; then
  echo "chrome clones: removed 0 (no clone folder)"
  exit 0
fi

removed=0
for clone in "$CLONES"/code_sign_clone.*; do
  [ -d "$clone" ] || continue
  rm -rf "${clone:?}" && removed=$((removed + 1))
done
echo "chrome clones: removed $removed"
