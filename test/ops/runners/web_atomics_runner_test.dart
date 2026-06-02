// Atomics mode — all tests through the Pdf API.
// Run: dart test test/ops/runners/web_atomics_runner_test.dart -p chrome-coi

@TestOn('browser')
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

void main() {
  late Pdf pdf;
  late int serverPort;

  test('engine init', () async {
    final channel = spawnHybridUri(
      '/test/helpers/asset_server.dart',
      stayAlive: true,
    );
    serverPort = (await channel.stream.first as double).toInt();

    pdf = Pdf(config: PdfConfig(
      webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
      webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
      webIoMode: PdfIoMode.atomics,
    ));
  }, timeout: Timeout(Duration(seconds: 10)));

  test('verify I/O mode is atomics', () async {
    final doc = await pdf.open(src(minimalPdf));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.atomics);
  }, timeout: Timeout(Duration(seconds: 5)));

  tearDownAll(() => pdf.dispose());

  registerDocTests(() => pdf);
  registerEditorTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerStandaloneTests(() => pdf);
  registerSugarTests(() => pdf);
  registerLifecycleTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.atomics,
  )));
  registerInstanceTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.atomics,
  )));

  registerDocStressTests(() => pdf);
  registerEditorStressTests(() => pdf);
  registerSugarStressTests(() => pdf);
  registerStandaloneStressTests(() => pdf);
  registerBuilderStressTests(() => pdf);
  registerInstanceStressTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.atomics,
  )));
}
