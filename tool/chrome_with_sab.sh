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
exec /Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome \
  --enable-features=SharedArrayBuffer "$@"
