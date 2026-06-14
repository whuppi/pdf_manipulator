// PdfTask — a cancellable engine operation.
//
// Every engine method returns a PdfTask instead of a bare Future.
// It IS a Future (awaitable, then-able, zero migration pain) plus
// one verb: cancel().
//
// Unhandled-error physics (load-bearing):
//   - A cancelled outcome (PdfCancelled) is a normal lifecycle event
//     — task.cancel(), pdf.dispose() mid-flight. It must NEVER
//     surface as an unhandled zone error when the caller
//     fired-and-forgot the task.
//   - A real failure on a fire-and-forgotten task must STAY loud:
//     it is re-raised into the zone so it is never silently lost.
//   - An awaited task behaves exactly like a Future: the awaiter
//     receives every error, the zone receives none.

import 'dart:async';

import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/errors.dart';

// Multi-step composers (sugar ops) reach CancelHook through this
// file so they never import the runtime layer directly.
export 'package:pdf_manipulator/src/types/cancel_hook.dart' show CancelHook;

/// A running engine operation: awaitable like a [Future], cancellable
/// with [cancel].
///
/// ```dart
/// final task = doc.extract(pages: PdfPages.all);
/// task.cancel();          // soft-cancel — idempotent
/// await task;             // → throws PdfCancelled
/// ```
///
/// Cancelling after completion is a no-op. A cancelled task resolves
/// with [PdfCancelled] — never with a partial result.
class PdfTask<T> implements Future<T> {
  /// INTERNAL — built by the bridge around a tracked operation
  /// future and its cancel hook. Not for consumer construction.
  @internal
  PdfTask.internal(this._inner, this._hook) {
    unawaited(
      _inner.then<void>(
        (_) {},
        onError: (Object e, StackTrace st) {
          if (e is PdfCancelled) return; // normal lifecycle — silent
          // Real failure: stay loud for fire-and-forget callers. The
          // microtask gives a same-turn awaiter time to attach first;
          // once anything listens, the listener owns the error.
          scheduleMicrotask(() {
            if (!_listened) Error.throwWithStackTrace(e, st);
          });
        },
      ),
    );
  }

  /// INTERNAL — wraps a multi-step flow in one cancellable task.
  ///
  /// [body] receives the task's hook; it runs each engine step with
  /// `hook.guard(step)` so cancel() always lands on the step that is
  /// currently in flight (re-binding replaces the previous step's
  /// action; a cancelled hook cancels the next step on sight).
  @internal
  static PdfTask<T> group<T>(Future<T> Function(CancelHook hook) body) {
    final hook = CancelHook();
    return PdfTask.internal(body(hook), hook);
  }

  final Future<T> _inner;
  final CancelHook _hook;
  bool _listened = false;

  /// Requests cancellation of this operation. Idempotent; a no-op
  /// once the task has completed. The task resolves with
  /// [PdfCancelled].
  void cancel() => _hook.cancel();

  /// Whether [cancel] has been requested (the task may still be
  /// settling).
  bool get isCancelled => _hook.isCancelled;

  /// INTERNAL — maps the result while preserving the cancel hook,
  /// so decoded/typed layers stay cancellable.
  @internal
  PdfTask<R> map<R>(FutureOr<R> Function(T value) convert) {
    _listened = true; // the child task takes over error reporting
    return PdfTask.internal(_inner.then(convert), _hook);
  }

  @override
  Future<R> then<R>(
    FutureOr<R> Function(T value) onValue, {
    Function? onError,
  }) {
    _listened = true;
    return _inner.then(onValue, onError: onError);
  }

  @override
  Future<T> catchError(Function onError, {bool Function(Object error)? test}) {
    _listened = true;
    return _inner.catchError(onError, test: test);
  }

  @override
  Future<T> whenComplete(FutureOr<void> Function() action) {
    _listened = true;
    return _inner.whenComplete(action);
  }

  @override
  Stream<T> asStream() {
    _listened = true;
    return _inner.asStream();
  }

  @override
  Future<T> timeout(Duration timeLimit, {FutureOr<T> Function()? onTimeout}) {
    _listened = true;
    return _inner.timeout(timeLimit, onTimeout: onTimeout);
  }
}

/// INTERNAL — step composition for multi-step flows (see
/// [PdfTask.group]).
extension CancelHookGuard on CancelHook {
  /// Runs one step under this hook: cancel() on the group cancels
  /// whichever step is currently guarded.
  @internal
  Future<T> guard<T>(PdfTask<T> task) {
    bind(task.cancel);
    return task;
  }
}
