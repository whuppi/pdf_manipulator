// Device matrix runner — runs one journey against every device profile.
//
// Call testJourneyAcrossDevices(...) with a name and a body. It emits one
// testWidgets per profile in kDeviceMatrix, setting that profile's
// viewport size + pixel ratio before the body runs and restoring the
// defaults after. The body receives the WidgetTester and the active
// profile, so a failure names the exact device it broke on.
//
// This is what makes local a strict superset of CI: every journey is
// proven on six device shapes — including one tighter than any CI
// emulator — on the host VM, every run.
//
// App-agnostic — part of harness/.

import 'package:flutter_test/flutter_test.dart';

import 'device_profiles.dart';

/// Signature of a journey body: drive the app, assert outcomes.
typedef JourneyBody = Future<void> Function(
  WidgetTester tester,
  DeviceProfile device,
);

/// Register [body] as a test for every profile in [matrix].
///
/// The viewport is set BEFORE the body (so the first frame already lays
/// out at the device size) and reset in a tearDown (so profiles never
/// leak into each other).
void testJourneyAcrossDevices(
  String description,
  JourneyBody body, {
  List<DeviceProfile> matrix = kDeviceMatrix,
}) {
  for (final device in matrix) {
    testWidgets('$description — $device', (tester) async {
      tester.view.physicalSize = device.size * device.devicePixelRatio;
      tester.view.devicePixelRatio = device.devicePixelRatio;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await body(tester, device);
    });
  }
}
