// Runtime demo journey — the Runtime tab's demo buttons are reachable
// and tappable on every device shape.
//
// The CI failure that started all this was a Runtime demo button
// scrolled just past the bottom edge on a small screen: it built, but
// the tap missed. This journey reproduces that exact path — scroll the
// demo into view and tap it — on every device in the matrix, including
// one tighter than CI. A miss fails locally first.
//
// It taps the lightest demo (cancel-before-start: no file, no heavy
// build) so the matrix stays fast; the on-device smoke suite exercises
// the heavy demos' full execution once on a real device.

import '../harness/device_matrix.dart';
import '../robots/app_robot.dart';
import '../robots/runtime_tab_robot.dart';

void main() {
  testJourneyAcrossDevices('runtime: demos reachable + tappable',
      (tester, device) async {
    final app = AppRobot(tester);
    await app.launch();

    // Runtime is the first tab and active on launch, but assert it
    // explicitly so the demo list is the on-stage page.
    await app.openTab(appTabs.first);

    final runtime = RuntimeTabRobot(tester);
    await runtime.runDemo('Cancel before the job even starts');
    await runtime.expectStatusContains('PdfCancelled');
  });
}
