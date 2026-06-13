// Responsive widget test — the example laid out at every device class.
//
// A real app keeps two test suites: integration_test/ drives the app on
// a real device (visible, real screen), and test/ runs host-VM widget
// tests where the viewport can be resized to any device. Size emulation
// belongs here, not in the on-device suite — setting physicalSize there
// hides the UI (flutter/flutter#149209). Here there's no device UI to
// hide, so resizing is free and correct.
//
// This proves every tab and every Runtime demo button is reachable at
// phone, tablet, and desktop sizes. Runs under `flutter test` with no
// device, so a small-screen layout regression fails locally instead of
// only on CI's Android emulator.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pdf_manipulator_example/main.dart' as app;

/// Device classes that bracket the real ones: a small phone (the Android
/// CI emulator's size), a tablet, and a desktop window. Logical pixels.
const _viewports = <({String name, Size size})>[
  (name: 'phone', size: Size(320, 640)),
  (name: 'tablet', size: Size(768, 1024)),
  (name: 'desktop', size: Size(1280, 800)),
];

/// Every tab label, in order.
const _tabs = [
  'Runtime',
  'Doc',
  'Sugar',
  'Standalone',
  'Editor',
  'Builder',
  'Merge',
];

/// The Runtime demos the on-device smoke tests tap — must stay reachable
/// at every size. If one sits unreachably below the fold, the scroll
/// here fails exactly as it would on a real small screen.
const _runtimeDemos = [
  'Cancel before the job even starts',
  'Open 4 documents in parallel',
  'Dispose mid-flight — measure it',
];

void main() {
  for (final vp in _viewports) {
    testWidgets(
        'responsive @ ${vp.name} (${vp.size.width.toInt()}×'
        '${vp.size.height.toInt()}): every tab + demo reachable', (t) async {
      t.view.physicalSize = vp.size;
      t.view.devicePixelRatio = 1.0;
      addTearDown(t.view.resetPhysicalSize);
      addTearDown(t.view.resetDevicePixelRatio);

      app.main();
      await t.pumpAndSettle();

      // Every tab reachable at this width — scroll it on-screen (the bar
      // is scrollable, so narrow widths push the last tabs off the right
      // edge), then tap it and prove its surface shows.
      for (final tab in _tabs) {
        await _tapTab(t, tab);
      }

      // Each Runtime demo button scrolls into view — the exact
      // reachability the on-device smoke tests depend on.
      await _tapTab(t, 'Runtime');
      final list = find.descendant(
        of: find.byType(ListView),
        matching: find.byType(Scrollable),
      );
      for (final demo in _runtimeDemos) {
        await t.scrollUntilVisible(
          find.byKey(ValueKey('run:$demo')),
          200,
          scrollable: list,
        );
      }
    });
  }
}

/// Scrolls a tab on-screen then taps it.
Future<void> _tapTab(WidgetTester t, String label) async {
  final tab = find.text(label);
  await t.scrollUntilVisible(tab, 120,
      scrollable: find.byType(Scrollable).first);
  await t.tap(tab);
  await t.pumpAndSettle();
}
