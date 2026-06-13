// AppRobot — drives the example's top-level shell (boot + tab nav).
//
// This is an APP-specific robot: it knows the example's tabs and shell.
// It inherits every hardened primitive from the harness Robot, so tab
// switching is reliable at any device size without re-solving scroll /
// offstage quirks here.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pdf_manipulator_example/main.dart' as app;

import '../harness/robot.dart';

/// Every tab: its label (on the TabBar) paired with a marker — a string
/// unique to that tab's body, used to confirm the page actually switched.
const appTabs = <({String label, String marker})>[
  (label: 'Runtime', marker: 'The lane architecture'),
  (label: 'Doc', marker: 'Open a PDF to query it'),
  (label: 'Sugar', marker: 'Pick a PDF for one-shot ops'),
  (label: 'Standalone', marker: 'Source in → sink out, no handle'),
  (label: 'Editor', marker: 'Parse once, mutate N times, save once'),
  (label: 'Builder', marker: 'Create PDFs from scratch'),
  (label: 'Merge', marker: 'Pick 2+ PDFs to merge — drag to reorder'),
];

class AppRobot extends Robot {
  AppRobot(super.tester);

  /// Boot the example and wait for the first stable frame.
  Future<void> launch() async {
    app.main();
    await settle();
  }

  /// The TabBar's own (horizontal) scrollable — the first Scrollable in
  /// the tree, above the body.
  Finder get _tabBar => find.byType(Scrollable).first;

  /// Switch to [tab] and confirm its body is on-stage via the tab's
  /// marker text.
  Future<void> openTab(({String label, String marker}) tab) {
    return tapTab(
      find.text(tab.label),
      tabBar: _tabBar,
      pageContent: find.textContaining(tab.marker),
    );
  }
}
