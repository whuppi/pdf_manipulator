// CancelHook — the one-shot binding between a PdfTask and its job.
//
// Created by the bridge per operation, bound by the Router to the
// job's lane once the job is submitted. Cancel-before-bind is legal:
// the Router checks the flag at entry and resolves the op as
// cancelled without ever submitting.
//
// Lives entirely on the main isolate's event loop — single owner,
// no locks, no check-then-act races.
//
// INTERNAL — constructed by SharedBridge, bound by the Router,
// driven by PdfTask.cancel().

/// One-shot cancellation for a single engine operation.
class CancelHook {
  bool _cancelled = false;
  void Function()? _action;

  /// Whether [cancel] has been called.
  bool get isCancelled => _cancelled;

  /// Requests cancellation. Idempotent. Fires the bound action (the
  /// Router's lane-job cancel) exactly once if one is bound; if the
  /// job hasn't been submitted yet, the Router's entry check covers
  /// it.
  void cancel() {
    if (_cancelled) return;
    _cancelled = true;
    final action = _action;
    _action = null;
    action?.call();
  }

  /// Binds the live job's cancel action. If [cancel] already ran,
  /// the action fires immediately (cancel raced ahead of submit).
  void bind(void Function() action) {
    if (_cancelled) {
      action();
      return;
    }
    _action = action;
  }

  /// Drops the bound action — the job completed; cancelling a
  /// finished job must be a no-op.
  void unbind() {
    _action = null;
  }
}
