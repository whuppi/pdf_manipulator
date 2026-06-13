// Robot — the base class every app-specific robot extends.
//
// A Robot wraps a WidgetTester and exposes hardened interaction
// primitives that solve Flutter's well-known UI-test foot-guns ONCE, so
// individual robots and journeys never re-solve them:
//
//   • settle()         — advance frames to a stable point WITHOUT
//                        hanging on a transient/indeterminate animation
//                        (the classic `pumpAndSettle timed out`).
//   • scrollToAndTap() — build a lazy off-screen child, CENTRE it in the
//                        viewport, then tap. scrollUntilVisible alone
//                        only brings the START edge in, so on a small
//                        screen the tap point can still miss.
//   • tapTab()         — switch a TabBar tab and wait until the new
//                        page's content is actually on-stage, not the
//                        kept-alive offstage page underneath.
//   • expectVisible()  — assert a finder resolves to exactly one
//                        on-stage, hit-testable widget.
//
// App-agnostic. Copy harness/ into any Flutter app; subclass Robot per
// screen.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'pump_strategies.dart';

class Robot {
  Robot(this.tester);

  final WidgetTester tester;

  /// Advance frames to a stable point. Unlike pumpAndSettle, this never
  /// hangs when a transient/indeterminate animation (a progress spinner)
  /// is still ticking — see pump_strategies.dart.
  Future<void> settle() => settleWithoutHanging(tester);

  /// Build [target] if it's a lazy off-screen child of [list], centre it
  /// in the viewport, and tap it.
  ///
  /// Why three steps (each defeats a real foot-gun):
  ///   1. scrollUntilVisible BUILDS a child that a lazy ListView hasn't
  ///      created yet (ensureVisible can't find an unbuilt element).
  ///   2. Scrollable.ensureVisible(alignment: 0.5) CENTRES it — Flutter's
  ///      scroll-to only guarantees the start edge is visible, so a tall
  ///      child's centre (the tap point) can sit past the far edge.
  ///   3. duration: zero jumps instead of animating, so no animation is
  ///      left in flight for the tap.
  Future<void> scrollToAndTap(
    Finder target, {
    required Finder list,
    double scrollDelta = 120,
    int maxScrolls = 60,
  }) async {
    await tester.scrollUntilVisible(
      target,
      scrollDelta,
      scrollable: list,
      maxScrolls: maxScrolls,
    );
    await Scrollable.ensureVisible(
      tester.element(target),
      alignment: 0.5,
      duration: Duration.zero,
    );
    await tester.pump();
    await tester.tap(target);
    await settle();
  }

  /// Tap a TabBar tab found by [tabFinder], scrolling the (scrollable)
  /// bar until the tab is reachable, then wait until [pageContent] — a
  /// finder for something unique to the destination page — is actually
  /// on-stage.
  ///
  /// A TabBarView keeps inactive pages alive but OFFSTAGE. Right after
  /// the tap the destination page is still mid-swap, so a finder can
  /// resolve to the offstage copy (a tap then lands on dead space). We
  /// pump until [pageContent] is on-stage and hit-testable.
  Future<void> tapTab(
    Finder tabFinder, {
    required Finder tabBar,
    required Finder pageContent,
  }) async {
    await tester.scrollUntilVisible(tabFinder, 120, scrollable: tabBar);
    await Scrollable.ensureVisible(
      tester.element(tabFinder),
      alignment: 0.5,
      duration: Duration.zero,
    );
    await tester.pump();
    await tester.tap(tabFinder);
    await settle();
    await pumpUntil(
      tester,
      () => _isOnStage(pageContent),
      describe: 'tab page content on-stage',
    );
  }

  /// Assert [finder] resolves to exactly one on-stage widget.
  void expectVisible(Finder finder, {String? reason}) {
    expect(finder, findsOneWidget, reason: reason);
  }

  /// True when [finder] matches at least one on-stage element. `find`
  /// defaults to skipOffstage: true, so a match here means on-stage.
  bool _isOnStage(Finder finder) => tester.any(finder);
}
