// CancelHook — the one-shot cancel binding.
//
// The contract under test: cancel fires the bound action exactly
// once; cancel-before-bind fires at bind time; unbind makes a later
// cancel a no-op; rebinding replaces (never stacks) the action.

@TestOn('vm')
library;

import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:test/test.dart';

void main() {
  group('CancelHook', () {
    test('cancel fires the bound action exactly once', () {
      final hook = CancelHook();
      var fired = 0;
      hook.bind(() => fired++);

      hook.cancel();
      hook.cancel(); // idempotent
      expect(fired, 1);
      expect(hook.isCancelled, isTrue);
    });

    test('cancel before bind fires the action at bind time', () {
      final hook = CancelHook();
      hook.cancel();
      expect(hook.isCancelled, isTrue);

      var fired = 0;
      hook.bind(() => fired++);
      expect(fired, 1, reason: 'bind on a cancelled hook fires immediately');
    });

    test('unbind makes a later cancel fire nothing', () {
      final hook = CancelHook();
      var fired = 0;
      hook.bind(() => fired++);
      hook.unbind();

      hook.cancel();
      expect(fired, 0);
      expect(
        hook.isCancelled,
        isTrue,
        reason: 'the flag still flips — only the action is detached',
      );
    });

    test('rebind replaces the previous action', () {
      final hook = CancelHook();
      var first = 0;
      var second = 0;
      hook.bind(() => first++);
      hook.bind(() => second++); // a new step takes over

      hook.cancel();
      expect(first, 0, reason: 'replaced action must not fire');
      expect(second, 1);
    });

    test('cancel after a completed cancel-and-unbind cycle is silent', () {
      final hook = CancelHook();
      var fired = 0;
      hook.bind(() => fired++);
      hook.cancel();
      hook.unbind(); // job completed; Router unbinds in its finally

      hook.cancel();
      expect(fired, 1);
    });
  });
}
