// Repro script — verifies dispose() exits cleanly.
// Called by native_finalizer_test.dart as a subprocess.
//
// Creates Pdf, opens a doc (pins source callbacks), disposes, exits.
// Before the fix: dispose() killed the isolate while Rust threads
// still held NativeCallable pointers → FATAL on Windows.
// After the fix: dispose() cancels buffers, joins threads, then
// closes callables → clean exit.

import 'package:pdf_manipulator/pdf_manipulator.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/test_source_sink.dart';

void main() async {
  final pdf = Pdf();
  final doc = await pdf.open(src(minimalPdf));
  assert(doc.pageCount == 1);
  await doc.dispose();
  await pdf.dispose();
}
