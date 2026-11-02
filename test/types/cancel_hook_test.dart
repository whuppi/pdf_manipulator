// CancelHook — the one-shot cancel binding.
//
// The contract under test: cancel runs every registered action exactly
// once; cancel-before-register runs the action at register time;
// unregistering makes a later cancel skip that action; registering never
// replaces another action.

@TestOn('vm')
library;

import 'dart:async';

import 'package:pdf_manipulator/pdf_manipulator.dart' show PdfCancelled;
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:test/test.dart';

import '../harness/test_source_sink.dart';

void main() {
  group('CancelHook', () {
    test('cancel fires the registered action exactly once', () {
      final hook = CancelHook();
      var fired = 0;
      hook.register(() => fired++);

      hook.cancel();
      hook.cancel(); // idempotent
      expect(fired, 1);
      expect(hook.isCancelled, isTrue);
    });

    test('cancel before register fires the action at register time', () {
      final hook = CancelHook();
      hook.cancel();
      expect(hook.isCancelled, isTrue);

      var fired = 0;
      final unregister = hook.register(() => fired++);
      expect(fired, 1, reason: 'register on a cancelled hook fires at once');
      unregister(); // nothing to drop; must not throw
      hook.cancel();
      expect(fired, 1);
    });

    test('unregister makes a later cancel fire nothing', () {
      final hook = CancelHook();
      var fired = 0;
      final unregister = hook.register(() => fired++);
      unregister();

      hook.cancel();
      expect(fired, 0);
      expect(
        hook.isCancelled,
        isTrue,
        reason: 'the flag still flips — only the action is detached',
      );
    });

    test('many actions all fire, once each', () {
      final hook = CancelHook();
      final fired = List.filled(3, 0);
      for (var i = 0; i < fired.length; i++) {
        hook.register(() => fired[i]++);
      }

      hook.cancel();
      hook.cancel();
      expect(fired, [1, 1, 1]);
    });

    test('unregistering one action leaves the others', () {
      final hook = CancelHook();
      var first = 0;
      var second = 0;
      var third = 0;
      hook.register(() => first++);
      final dropSecond = hook.register(() => second++);
      hook.register(() => third++);
      dropSecond();
      dropSecond(); // twice is harmless

      hook.cancel();
      expect([first, second, third], [1, 0, 1]);
    });

    test('a second registration of the same action survives the first '
        'one unregistering twice', () {
      final hook = CancelHook();
      var fired = 0;
      void action() => fired++;
      final dropFirst = hook.register(action);
      hook.register(action);
      dropFirst();
      dropFirst();

      hook.cancel();
      expect(fired, 1);
    });

    test(
      'an action registered after cancel runs at once, others untouched',
      () {
        final hook = CancelHook();
        var early = 0;
        hook.register(() => early++);
        hook.cancel();

        var late = 0;
        hook.register(() => late++);
        expect(early, 1);
        expect(late, 1);
      },
    );

    test('cancel after a completed cancel-and-unregister cycle is silent', () {
      final hook = CancelHook();
      var fired = 0;
      final unregister = hook.register(() => fired++);
      hook.cancel();
      unregister(); // job completed; Router unregisters in its finally

      hook.cancel();
      expect(fired, 1);
    });

    test(
      'abortSinks aborts each sink once however often it is named',
      () async {
        final hook = CancelHook();
        final a = CountingSink();
        final b = CountingSink();
        await hook.abortSinks([a, b], const PdfCancelled());
        await hook.abortSinks([a], const PdfCancelled());
        expect(a.aborts, 1);
        expect(b.aborts, 1);
      },
    );

    test('a sink is aborted once per hook, so once per operation', () async {
      final sink = CountingSink();
      await CancelHook().abortSinks([sink], const PdfCancelled());
      await CancelHook().abortSinks([sink], const PdfCancelled());
      expect(sink.aborts, 2);
    });

    test(
      'an abort that throws is not reported and does not stop the rest',
      () async {
        final hook = CancelHook();
        final after = CountingSink();
        await hook.abortSinks([_ThrowingAbort(), after], StateError('why'));
        expect(after.aborts, 1);
      },
    );
  });
}

class _ThrowingAbort extends CountingSink {
  @override
  FutureOr<void> abort(Object reason) => throw StateError('abort failed');
}
