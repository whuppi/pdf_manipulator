// Navigation journey — every tab reachable + showing its surface, on
// every device shape.
//
// Runs host-VM via the device matrix. Because the matrix includes a
// viewport smaller than any CI emulator, a tab that clips off-screen (or
// a tap that misses on a narrow bar) fails HERE, locally, on every
// `flutter test` — never first in CI.

import 'package:flutter_test/flutter_test.dart';

import 'package:pdf_manipulator_example_test_support/pdf_manipulator_example_test_support.dart';

void main() {
  testJourneyAcrossDevices('navigation: every tab opens',
      (tester, device) async {
    final app = AppRobot(tester);
    await app.launch();

    // Visit every tab in order and confirm its body switched in.
    for (final tab in appTabs) {
      await app.openTab(tab);
      app.expectVisible(
        find.textContaining(tab.marker),
        reason: '${tab.label} body not shown on $device',
      );
    }
  });
}
