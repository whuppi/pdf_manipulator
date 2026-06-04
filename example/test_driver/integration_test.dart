// Custom driver that streams Chrome console output (live test progress)
// while waiting for integration test results.
//
// The default integrationDriver() blocks silently until all tests finish.
// This driver polls Chrome's browser logs via chromedriver and prints
// each console.log line — the "00:00 +N: test name" progress that the
// Dart test runner produces inside Chrome via print().

import 'dart:async';
import 'dart:io';

import 'package:flutter_driver/flutter_driver.dart';
import 'package:integration_test/common.dart';
import 'package:webdriver/async_io.dart' as async_io;

Future<void> main() async {
  final FlutterDriver driver = await FlutterDriver.connect();
  final rawDriver = (driver as WebFlutterDriver).webDriver;

  // Poll browser console logs in background.
  var done = false;
  unawaited(Future(() async {
    while (!done) {
      try {
        final entries =
            await rawDriver.logs.get(async_io.LogType.browser).toList();
        for (final entry in entries) {
          final raw = entry.message;
          if (raw == null) continue;
          // Chrome log format: 'http://...dart_sdk.js N:N "actual message"'
          // Extract the quoted message.
          final qStart = raw.indexOf('"');
          final qEnd = raw.lastIndexOf('"');
          if (qStart != -1 && qEnd > qStart) {
            print(raw.substring(qStart + 1, qEnd));
          }
        }
      } catch (_) {}
      await Future.delayed(const Duration(milliseconds: 10));
    }
  }));

  final jsonResult = await driver.requestData(
    null,
    timeout: const Duration(minutes: 20),
  );
  done = true;
  final response = Response.fromJson(jsonResult);
  await driver.close();

  if (response.allTestsPassed) {
    print('All tests passed.');
    exit(0);
  } else {
    print('Failure Details:\n${response.formattedFailureDetails}');
    exit(1);
  }
}
