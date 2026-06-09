// OPFS mode — all tests through the Pdf API.
// Run: dart test test/ops/runners/web_opfs_runner_test.dart -p chrome

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
      webIoMode: PdfIoMode.opfs,
    ));
  }, timeout: Timeout(Duration(seconds: 3)));

  test('verify I/O mode is opfs', () async {
    final doc = await pdf.open(src(minimalPdf));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.opfs);
  }, timeout: Timeout(Duration(seconds: 3)));

  tearDownAll(() => pdf.dispose());

  registerDocTests(() => pdf);
  registerEditorTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerStandaloneTests(() => pdf);
  registerSugarTests(() => pdf);
  registerLifecycleTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.opfs,
  )));
  registerInstanceTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.opfs,
  )));

  registerDocStressTests(() => pdf);
  registerEditorStressTests(() => pdf);
  registerSugarStressTests(() => pdf);
  registerStandaloneStressTests(() => pdf);
  registerBuilderStressTests(() => pdf);
  registerInstanceStressTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: 'http://localhost:$serverPort/web_assets/coordinator.js',
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    webIoMode: PdfIoMode.opfs,
  )));
}
