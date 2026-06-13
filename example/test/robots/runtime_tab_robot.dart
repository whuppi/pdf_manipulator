// RuntimeTabRobot — drives the example's Runtime tab (the lane demos).
//
// App-specific: knows the demo buttons are keyed 'run:<title>' and that
// the demo list is the tab's only vertical ListView. Inherits the
// harness scroll-to-centre-and-tap primitive, so a demo below the fold
// on a small screen is reached and tapped reliably.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../harness/pump_strategies.dart';
import '../harness/robot.dart';

class RuntimeTabRobot extends Robot {
  RuntimeTabRobot(super.tester);

  /// The Runtime body's vertical ListView scrollable — the demo list.
  /// (The TabBar and TabBarView pager are horizontal; this is the only
  /// vertical one.)
  Finder get _demoList => find.descendant(
        of: find.byType(ListView),
        matching: find.byType(Scrollable),
      );

  /// Run a demo by its visible title (the button is keyed 'run:<title>').
  Future<void> runDemo(String title) {
    return scrollToAndTap(
      find.byKey(ValueKey('run:$title')),
      list: _demoList,
    );
  }

  /// Wait until the status bar contains [text] (a demo's success line),
  /// pumping past the transient spinner without hanging.
  Future<void> expectStatusContains(String text) {
    return pumpUntil(
      tester,
      () => tester.any(find.textContaining(text)),
      describe: 'status "$text"',
    );
  }
}
