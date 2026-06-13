// Navigation journey — every tab reachable + showing its surface, on
// every device shape.
//
// Runs host-VM via the device matrix. Because the matrix includes a
// viewport smaller than any CI emulator, a tab that clips off-screen (or
// a tap that misses on a narrow bar) fails HERE, locally, on every
// `flutter test` — never first in CI.

import 'package:flutter_test/flutter_test.dart';

import '../../integration_test/support/harness/device_matrix.dart';
import '../../integration_test/support/robots/app_robot.dart';

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
