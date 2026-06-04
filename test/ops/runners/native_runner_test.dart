// Runs all tests through the native FFI path.
// Same tests run on web via web_opfs/jspi/atomics_runner_test.dart.

@TestOn('!browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/test_source_sink.dart';

import '../core/pdf_doc_test.dart';
import '../core/pdf_editor_test.dart';
import '../core/pdf_builder_test.dart';
import '../core/pdf_standalone_test.dart';
import '../core/pdf_sugar_test.dart';
import '../core/pdf_lifecycle_test.dart';
import '../core/pdf_instance_test.dart';
import '../stress/pdf_doc_stress_test.dart';
import '../stress/pdf_editor_stress_test.dart';
import '../stress/pdf_sugar_stress_test.dart';
import '../stress/pdf_standalone_stress_test.dart';
import '../stress/pdf_builder_stress_test.dart';
import '../stress/pdf_instance_stress_test.dart';
import '../native/native_finalizer_test.dart';

void main() {
  late Pdf pdf;

  test('engine init', () {
    pdf = Pdf();
  }, timeout: Timeout(Duration(seconds: 5)));

  test('verify I/O mode is native', () async {
    final doc = await pdf.open(src(minimalPdf));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.native);
  }, timeout: Timeout(Duration(seconds: 2)));

  tearDownAll(() => pdf.dispose());

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
