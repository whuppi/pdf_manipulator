// PdfTask — the unhandled-error physics, unit-tested directly.
//
// These are the load-bearing guarantees the integration suites can
// only observe as side effects:
//   - a fire-and-forgotten cancelled task NEVER reaches the zone
//   - a fire-and-forgotten REAL failure ALWAYS reaches the zone
//   - an awaited task behaves exactly like a Future (awaiter owns
//     every error; the zone sees none)
//   - map() preserves the cancel hook and hands error reporting to
//     the outermost task in the chain
//   - group()/guard() route a group cancel onto whichever step is
//     currently in flight

@TestOn('vm')
library;

import 'dart:async';

import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:test/test.dart';

/// Runs [body] in a guarded zone and returns the errors the zone
/// caught after the microtask queue drained.
Future<List<Object>> _zoneErrors(Future<void> Function() body) async {
  final errors = <Object>[];
  await runZonedGuarded(() async {
    await body();
    // The stay-loud re-raise is microtask-deferred; give it room.
    await Future<void>.delayed(Duration.zero);
    await Future<void>.delayed(Duration.zero);
  }, (e, _) => errors.add(e));
  return errors;
}

void main() {
  group('PdfTask zone physics', () {
    test('awaited success delivers the value, zone stays clean', () async {
      final errors = await _zoneErrors(() async {
        final completer = Completer<int>();
        final task = PdfTask.internal(completer.future, CancelHook());
        completer.complete(42);
        expect(await task, 42);
      });
      expect(errors, isEmpty);
    });

    test('awaited failure throws to the awaiter, zone stays clean', () async {
      final errors = await _zoneErrors(() async {
        final completer = Completer<int>();
        final task = PdfTask.internal(completer.future, CancelHook());
        completer.completeError(StateError('engine error'));
        await expectLater(task, throwsA(isA<StateError>()));
      });
      expect(
        errors,
        isEmpty,
        reason: 'the awaiter owns the error; the zone must not',
      );
    });

    test('fire-and-forget PdfCancelled is zone-silent', () async {
      final errors = await _zoneErrors(() async {
        final completer = Completer<int>();
        unawaited(PdfTask.internal(completer.future, CancelHook()));
        completer.completeError(const PdfCancelled());
      });
      expect(
        errors,
        isEmpty,
        reason: 'cancellation is a normal lifecycle event, never noise',
      );
    });

    test(
      'fire-and-forget REAL failure reaches the zone exactly once',
      () async {
        final errors = await _zoneErrors(() async {
          final completer = Completer<int>();
          unawaited(PdfTask.internal(completer.future, CancelHook()));
          completer.completeError(StateError('engine error'));
        });
        expect(
          errors,
          hasLength(1),
          reason: 'real failures must never be silently swallowed',
        );
        expect(errors.single, isA<StateError>());
      },
    );

    test('a same-turn listener suppresses the zone re-raise', () async {
      final errors = await _zoneErrors(() async {
        final completer = Completer<int>();
        final task = PdfTask.internal(completer.future, CancelHook());
        // Listener attached BEFORE completion — it owns the error.
        final caught = task.catchError((Object _) => -1);
        completer.completeError(StateError('engine error'));
        expect(await caught, -1);
      });
      expect(errors, isEmpty);
    });
  });

  group('PdfTask cancel surface', () {
    test('cancel drives the hook and is idempotent', () {
      final hook = CancelHook();
      var fired = 0;
      hook.bind(() => fired++);
      final task = PdfTask.internal(Completer<int>().future, hook);

      expect(task.isCancelled, isFalse);
      task.cancel();
      task.cancel();
      expect(task.isCancelled, isTrue);
      expect(fired, 1);
    });

    test('map preserves the cancel hook — child cancel reaches the job', () {
      final hook = CancelHook();
      var fired = 0;
      hook.bind(() => fired++);
      final completer = Completer<int>();
      final parent = PdfTask.internal(completer.future, hook);

      final child = parent.map((v) => 'value: $v');
      child.cancel();
      expect(fired, 1);
      expect(
        parent.isCancelled,
        isTrue,
        reason: 'one hook, one job — every layer sees the cancel',
      );
    });

    test('map hands error reporting to the outermost task', () async {
      final errors = await _zoneErrors(() async {
        final completer = Completer<int>();
        final parent = PdfTask.internal(completer.future, CancelHook());
        unawaited(parent.map((v) => v)); // child fire-and-forgotten too
        completer.completeError(StateError('engine error'));
      });
      expect(
        errors,
        hasLength(1),
        reason:
            'parent is marked listened by map(); only the '
            'outermost task reports — never both',
      );
    });

    test('map converts the value for awaiting callers', () async {
      final completer = Completer<int>();
      final task = PdfTask.internal(
        completer.future,
        CancelHook(),
      ).map((v) => 'got $v');
      completer.complete(7);
      expect(await task, 'got 7');
    });
  });

  group('PdfTask.group + guard', () {
    test('group cancel lands on the step currently in flight', () async {
      final stepHook = CancelHook();
      var stepCancelled = 0;
      stepHook.bind(() => stepCancelled++);
      final stepCompleter = Completer<int>();
      final step = PdfTask.internal(stepCompleter.future, stepHook);

      late final CancelHook groupHook;
      final task = PdfTask.group<int>((hook) async {
        groupHook = hook;
        return hook.guard(step);
      });

      await Future<void>.delayed(Duration.zero); // body reaches guard
      task.cancel();
      expect(
        stepCancelled,
        1,
        reason: 'the group hook is bound to the live step',
      );
      expect(groupHook.isCancelled, isTrue);

      stepCompleter.completeError(const PdfCancelled());
      await expectLater(task, throwsA(isA<PdfCancelled>()));
    });

    test('cancel before guard cancels the next step on sight', () async {
      final stepHook = CancelHook();
      var stepCancelled = 0;
      stepHook.bind(() => stepCancelled++);
      final stepCompleter = Completer<int>();
      final step = PdfTask.internal(stepCompleter.future, stepHook);

      final gate = Completer<void>();
      final task = PdfTask.group<int>((hook) async {
        await gate.future; // cancel arrives while the op is parked here
        return hook.guard(step);
      });

      task.cancel();
      gate.complete();
      await Future<void>.delayed(Duration.zero);
      expect(
        stepCancelled,
        1,
        reason: 'guard on a cancelled hook fires immediately',
      );

      stepCompleter.completeError(const PdfCancelled());
      await expectLater(task, throwsA(isA<PdfCancelled>()));
    });

    test('a later guard replaces the earlier step binding', () async {
      final firstHook = CancelHook();
      var firstCancelled = 0;
      firstHook.bind(() => firstCancelled++);
      final secondHook = CancelHook();
      var secondCancelled = 0;
      secondHook.bind(() => secondCancelled++);

      final first = Completer<int>();
      final second = Completer<int>();

      final task = PdfTask.group<int>((hook) async {
        await hook.guard(PdfTask.internal(first.future, firstHook));
        return hook.guard(PdfTask.internal(second.future, secondHook));
      });

      first.complete(1); // step one done — binding moves to step two
      await Future<void>.delayed(Duration.zero);
      task.cancel();
      expect(firstCancelled, 0, reason: 'finished steps are never poked');
      expect(secondCancelled, 1);

      second.completeError(const PdfCancelled());
      await expectLater(task, throwsA(isA<PdfCancelled>()));
    });
  });
}
