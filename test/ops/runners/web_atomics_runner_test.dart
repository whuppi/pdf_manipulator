// Atomics mode — all tests through the Pdf API.
// Run: dart test test/ops/runners/web_atomics_runner_test.dart -p chrome-coi

@TestOn('browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../core/pdf_doc_battery.dart';
import '../core/pdf_editor_battery.dart';
import '../core/pdf_form_encoding_battery.dart';
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
import '../platform/web/web_runtime_battery.dart';
import '../../harness/timeouts.dart';

void main() {
  late Pdf pdf;
  var booted = false;
  late int serverPort;

  test('engine init', () async {
    final channel = spawnHybridUri(
      '/test/harness/asset_server.dart',
      stayAlive: true,
    );
    serverPort = (await channel.stream.first as double).toInt();
    await purgeOpfsLaneFiles(); // hermetic: judge THIS run only

    pdf = Pdf(
      config: PdfConfig(
        webLaneWorkerUrl:
            'http://localhost:$serverPort/web_assets/lane_worker.js',
        webIoMode: PdfIoMode.atomics,
      ),
    );
    booted = true;
  }, timeout: t(2));

  test('verify I/O mode is atomics', () async {
    final doc = await pdf.open(src(minimalPdf));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.atomics);
  }, timeout: t(1));

  // A --name filter can skip 'engine init' — only dispose what booted.
  tearDownAll(() => booted ? pdf.dispose() : null);

  registerDocTests(() => pdf);
  registerEditorTests(() => pdf);
  registerFormEncodingTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerStandaloneTests(() => pdf);
  registerSugarTests(() => pdf);
  registerLifecycleTests(
    () => Pdf(
      config: PdfConfig(
        webLaneWorkerUrl:
            'http://localhost:$serverPort/web_assets/lane_worker.js',
        webIoMode: PdfIoMode.atomics,
      ),
    ),
  );
  registerInstanceTests(
    () => Pdf(
      config: PdfConfig(
        webLaneWorkerUrl:
            'http://localhost:$serverPort/web_assets/lane_worker.js',
        webIoMode: PdfIoMode.atomics,
      ),
    ),
  );

  registerDocStressTests(() => pdf);
  registerEditorStressTests(() => pdf);
  registerSugarStressTests(() => pdf);
  registerStandaloneStressTests(() => pdf);
  registerBuilderStressTests(() => pdf);
  registerInstanceStressTests(
    () => Pdf(
      config: PdfConfig(
        webLaneWorkerUrl:
            'http://localhost:$serverPort/web_assets/lane_worker.js',
        webIoMode: PdfIoMode.atomics,
      ),
    ),
  );

  // LAST on purpose: its residue check sweeps the whole suite.
  registerWebPlatformTests(
    workerUrl: () => 'http://localhost:$serverPort/web_assets/lane_worker.js',
    mode: PdfIoMode.atomics,
    disposeSharedInstance: () => pdf.dispose(),
  );
}
