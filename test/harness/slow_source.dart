// SlowSource — a DataSource that parks the engine on its first read.
//
// Per-op cancellation can only interrupt an operation that is blocked
// waiting on host I/O. A pure-compute op that already finished cannot
// be un-finished. To test cancellation deterministically (no timing
// luck), this source makes the engine's very first `readAt` hang on a
// Completer the test controls — so the op is provably parked when the
// test calls cancel().
//
//   final slow = SlowSource(bytes);
//   final task = pdf.open(slow);
//   await slow.firstRead;   // engine is now parked on readAt
//   task.cancel();          // cancel lands on a blocked read
//   await expectLater(task, throwsA(isA<PdfCancelled>()));
//   slow.release();         // unblock the abandoned read (op ignores it)

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_source.dart';

/// A [DataSource] whose first [readAt] never resolves until [release].
class SlowSource implements DataSource {
  SlowSource(this._data);

  final Uint8List _data;
  final Completer<void> _firstRead = Completer<void>();
  final Completer<void> _gate = Completer<void>();

  /// Completes when the engine has issued its first [readAt] — i.e.
  /// the op is genuinely parked on the source.
  Future<void> get firstRead => _firstRead.future;

  /// Unblocks the parked read. Idempotent. After a cancel the op has
  /// already unwound, so the freed read returns into nothing.
  void release() {
    if (!_gate.isCompleted) _gate.complete();
  }

  @override
  int get length => _data.length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    if (!_firstRead.isCompleted) _firstRead.complete();
    await _gate.future; // park until release()
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}
