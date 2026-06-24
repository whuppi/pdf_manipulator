// Subprocess payload for native_finalizer_battery.dart.
//
// Opens a doc (which pins a held source channel on a lane), disposes,
// and lets the process exit while the detached lane thread winds down
// on its own. If kill's no-notify-after-kill guarantee ever breaks, a
// lane thread invokes a dead NativeCallable here and the process
// FATAL-aborts ("Callback invoked after it has been deleted") instead
// of exiting 0.

// io-exempt: subprocess payload (sets exitCode) for the finalizer test.
import 'dart:io';

import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../../../fixtures/handwritten.dart';
import '../../../harness/test_source_sink.dart';

void main() async {
  final pdf = Pdf();
  final doc = await pdf.open(src(minimalPdf));
  if (doc.pageCount != 1) {
    // Explicit check, not assert — asserts may be disabled in this
    // launch mode, and a payload that can't fail proves nothing.
    exitCode = 2;
    return;
  }
  await doc.dispose();
  await pdf.dispose();
}
