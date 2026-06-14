// Runs all tests through the native FFI path.
// Same tests run on web via web_opfs/jspi/atomics_runner_test.dart.

@TestOn('!browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';

import '../core/pdf_doc_battery.dart';
import '../core/pdf_editor_battery.dart';
import '../core/pdf_builder_battery.dart';
import '../core/pdf_standalone_battery.dart';
import '../core/pdf_sugar_battery.dart';
import '../core/pdf_lifecycle_battery.dart';
import '../core/pdf_instance_battery.dart';
import '../stress/pdf_doc_stress_battery.dart';
import '../stress/pdf_editor_stress_battery.dart';
import '../stress/pdf_sugar_stress_battery.dart';
import '../stress/pdf_standalone_stress_battery.dart';
import '../stress/pdf_builder_stress_battery.dart';
import '../stress/pdf_instance_stress_battery.dart';
import '../platform/native/native_finalizer_battery.dart';
import '../../harness/timeouts.dart';

void main() {
  late Pdf pdf;
  var booted = false;

  test('engine init', () {
    pdf = Pdf();
    booted = true;
  }, timeout: t(2));

  test('verify I/O mode is native', () async {
    final doc = await pdf.open(src(minimalPdf));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.native);
  }, timeout: t(1));

  // A --name filter can skip 'engine init' — only dispose what booted.
  tearDownAll(() => booted ? pdf.dispose() : null);

  // Core tests
  registerDocTests(() => pdf);
  registerEditorTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerStandaloneTests(() => pdf);
  registerSugarTests(() => pdf);
  registerLifecycleTests(() => Pdf());
  registerInstanceTests(() => Pdf());

  // Stress tests
  registerDocStressTests(() => pdf);
  registerEditorStressTests(() => pdf);
  registerSugarStressTests(() => pdf);
  registerStandaloneStressTests(() => pdf);
  registerBuilderStressTests(() => pdf);
  registerInstanceStressTests(() => Pdf());

  // Native-only platform tests
  registerNativeFinalizerTests();
}
