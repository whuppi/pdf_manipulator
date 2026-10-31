// The backstop for a handle its developer never disposed.
//
// A handle owns an engine document and a lane pin; `dispose()` releases
// both. When the garbage collector frees a handle that was never disposed,
// the finalizer registered here releases them. Its timing is not
// guaranteed, so it never replaces `dispose()`.
//
// The release closure captures the bridge handle only, never the owning
// object: a closure over the owner would keep it alive and the finalizer
// would never run.

import 'dart:async';
import 'dart:developer' as developer;

import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/errors.dart';

/// Releases the engine side of a handle that was garbage collected without
/// `dispose()`, and in debug mode logs where it was opened.
final class HandleBackstop {
  const HandleBackstop._();

  static final _finalizer = Finalizer<_Leak>(_release);
  static final _leaks = Expando<_Leak>('HandleBackstop');

  /// Receives the debug warning; replaced by tests to read it.
  @visibleForTesting
  static void Function(String message) warn = _log;

  /// Registers [owner], a handle of [kind] (`PdfEditor`), so that its
  /// [release] runs when it is collected undisposed. [release] must not
  /// refer to [owner].
  static void attach(
    Object owner,
    String kind,
    Future<void> Function() release,
  ) {
    StackTrace? openedAt;
    assert(() {
      openedAt = StackTrace.current;
      return true;
    }());
    final leak = _Leak(kind, release, openedAt);
    _leaks[owner] = leak;
    _finalizer.attach(owner, leak, detach: owner);
  }

  /// Unregisters [owner]; called by its `dispose()`.
  static void detach(Object owner) {
    _leaks[owner] = null;
    _finalizer.detach(owner);
  }

  /// Runs what the finalizer runs for [owner] now, as if it had just been
  /// collected. Tests use it because a collection cannot be forced.
  @visibleForTesting
  static Future<void> finalizeNow(Object owner) async {
    final leak = _leaks[owner];
    if (leak == null) {
      throw PdfInvalidArgument('$owner is not registered with the backstop');
    }
    detach(owner);
    await _release(leak);
  }

  static Future<void> _release(_Leak leak) async {
    var debug = false;
    assert(() {
      debug = true;
      return true;
    }());
    if (debug) {
      warn(
        'a ${leak.kind} was never disposed; call dispose(), or open it '
        'through read, rewrite or buildTo.\nOpened at:\n${leak.openedAt}',
      );
    }
    try {
      await leak.release();
    } catch (_) {
      // The lane may already be gone with its Pdf instance; nothing is left
      // to release then.
    }
  }

  static void _log(String message) =>
      developer.log(message, name: 'pdf_manipulator', level: 900);
}

final class _Leak {
  const _Leak(this.kind, this.release, this.openedAt);

  final String kind;
  final Future<void> Function() release;
  final StackTrace? openedAt;
}
