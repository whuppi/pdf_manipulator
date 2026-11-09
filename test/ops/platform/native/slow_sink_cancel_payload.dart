// Subprocess payload for native_finalizer_battery.dart.
//
// Cancels a merge while its sink is still inside a write, then waits for
// that write to finish. The cancelled job posts and frees its write
// buffer while the sink is busy; when the sink returns, the listener must
// leave the freed buffer alone. Touching it is undefined behaviour in the
// native heap: it may crash this process or hang it, so the parent test
// asks for exit code 0 over many runs.

// io-exempt: subprocess payload (sets exitCode) for the finalizer test.
import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/runtime/native/channel_buffers.dart';

import '../../../fixtures/handwritten.dart';
import '../../../harness/test_source_sink.dart';

/// A sink whose first write takes [delay] to finish.
class _SlowSink implements DataSink {
  _SlowSink(this.delay);

  final Duration delay;
  final started = Completer<void>();
  final finished = Completer<void>();

  @override
  Future<void> write(Uint8List chunk) async {
    if (started.isCompleted) return;
    started.complete();
    await Future<void>.delayed(delay);
    finished.complete();
  }

  @override
  void abort(Object reason) {}

  @override
  void close() {}
}

void main() async {
  final pdf = Pdf();
  final sink = _SlowSink(const Duration(milliseconds: 400));
  final task = pdf.merge([
    PdfInput(src(minimalPdf)),
    PdfInput(src(minimalPdf)),
  ], sink);
  await sink.started.future;
  task.cancel();
  try {
    await task;
    exitCode = 2; // a cancelled merge must not succeed
    return;
  } on PdfCancelled {
    // expected
  }
  await sink.finished.future;
  // Let the listener's continuation run after the sink returned.
  await Future<void>.delayed(const Duration(milliseconds: 100));
  if (WriteChannelBuffer.writesAfterFree != 1) {
    exitCode = 3; // the guard did not see the late write
  }
  await pdf.dispose();
}
