// CancelHook — the one-shot binding between a PdfTask and its jobs.
//
// Created by the bridge per operation, registered by the Router with
// the job's lane once the job is submitted. A combinator's hook holds
// one action per op in flight (see PdfTask.group and the handle
// scope), so one cancel reaches them all. Cancel-before-register is
// legal: the action runs at once, and the Router checks the flag at
// entry and resolves the op as cancelled without ever submitting.
//
// Lives entirely on the main isolate's event loop — single owner,
// no locks, no check-then-act races.
//
// INTERNAL — constructed by SharedBridge, registered by the Router
// and by guard/handle scopes, driven by PdfTask.cancel().

import 'dart:collection';

import 'package:pdf_manipulator/src/types/data_sink.dart';

/// One-shot cancellation for one or many engine operations.
class CancelHook {
  bool _cancelled = false;
  final _actions = <void Function()>[];
  final _aborted = HashSet<DataSink>.identity();

  /// Aborts each of [sinks] that this hook's operation has not aborted yet,
  /// with [reason], and resolves once they are. An op and the combinator that
  /// owns it both end with an error, and both name the same output sink: the
  /// sink is aborted once however many of them say so. A sink whose abort
  /// throws is not reported: [reason] is the error that ended the operation.
  Future<void> abortSinks(Iterable<DataSink> sinks, Object reason) async {
    for (final sink in sinks) {
      if (!_aborted.add(sink)) continue;
      try {
        await sink.abort(reason);
      } catch (_) {
        // The error that ended the operation is the one reported.
      }
    }
  }

  /// Whether [cancel] has been called.
  bool get isCancelled => _cancelled;

  /// Requests cancellation. Idempotent. Runs every registered action
  /// exactly once; an op that has not been submitted yet is covered
  /// by the Router's entry check.
  void cancel() {
    if (_cancelled) return;
    _cancelled = true;
    final actions = List.of(_actions);
    _actions.clear();
    for (final action in actions) {
      action();
    }
  }

  /// Registers a live job's cancel [action] and returns the function
  /// that unregisters it (the job completed; cancelling a finished
  /// job must be a no-op, and a long-lived hook must not keep it).
  /// If [cancel] already ran, [action] runs immediately (cancel raced
  /// ahead of submit) and the returned function does nothing.
  void Function() register(void Function() action) {
    if (_cancelled) {
      action();
      return () {};
    }
    _actions.add(action);
    var registered = true;
    return () {
      if (!registered) return;
      registered = false;
      _actions.remove(action);
    };
  }
}
