// CancelHook — the one-shot cancel binding.
//
// The contract under test: cancel runs every registered action exactly
// once; cancel-before-register runs the action at register time;
// unregistering makes a later cancel skip that action; registering never
// replaces another action.

@TestOn('vm')
library;

import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:test/test.dart';

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
  });
}
