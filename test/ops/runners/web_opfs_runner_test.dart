// OPFS mode — all tests through the Pdf API.
// Run: dart test test/ops/runners/web_opfs_runner_test.dart -p chrome

@TestOn('browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/runtime/web/lease.dart' show WebExecutor;
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../core/pdf_doc_battery.dart';
import '../core/pdf_editor_battery.dart';
import '../core/pdf_form_battery.dart';
import '../core/pdf_form_encoding_battery.dart';
import '../core/pdf_kept_encryption_battery.dart';
import '../core/pdf_lzw_battery.dart';
import '../core/pdf_merge_battery.dart';
import '../core/pdf_builder_battery.dart';
import '../core/pdf_standalone_battery.dart';
import '../core/pdf_sugar_battery.dart';
import '../core/pdf_one_shot_battery.dart';
import '../core/pdf_render_battery.dart';
import '../core/pdf_determinism_battery.dart';
import '../core/pdf_abort_battery.dart';
import '../core/pdf_cancel_battery.dart';
import '../core/pdf_combinator_cancel_battery.dart';
import '../core/pdf_lifecycle_battery.dart';
import '../core/pdf_instance_battery.dart';
import '../core/pdf_lane_protocol_battery.dart';
import '../core/pdf_runtime_safety_battery.dart';
import '../core/pdf_executor_battery.dart';
import '../stress/pdf_doc_stress_battery.dart';
import '../stress/pdf_editor_stress_battery.dart';
import '../stress/pdf_sugar_stress_battery.dart';
import '../stress/pdf_standalone_stress_battery.dart';
import '../stress/pdf_builder_stress_battery.dart';
import '../stress/pdf_instance_stress_battery.dart';
import '../platform/web/web_asyncify_assets_battery.dart';
import '../platform/web/web_wasm_parts_battery.dart';
import '../platform/web/web_cancel_channel_battery.dart';
import '../platform/web/web_backing_battery.dart';
import '../platform/web/web_large_offset_battery.dart';
import '../platform/web/web_lane_protocol_battery.dart';
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
        webWorkerUrl:
            'http://localhost:$serverPort/web_assets/executor_worker.js',
        webIoMode: PdfIoMode.opfs,
      ),
    );
    booted = true;
    // Harness setup, not the engine: spawning the hybrid asset server
    // compiles an isolate, which takes seconds on a loaded machine.
  }, timeout: t(10));

  test('verify I/O mode is opfs', () async {
    final doc = await pdf.open(PdfInput(src(minimalPdf)));
    await doc.dispose();
    expect(pdf.ioMode, PdfIoMode.opfs);
  }, timeout: t(1));

  // A --name filter can skip 'engine init' — only dispose what booted.
  tearDownAll(() => booted ? pdf.dispose() : null);

  registerDocTests(() => pdf);
  registerEditorTests(() => pdf);
  registerFormEncodingTests(() => pdf);
  registerKeptEncryptionTests(() => pdf);
  registerLzwTests(() => pdf);
  registerMergeTests(() => pdf);
  registerFormTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerStandaloneTests(() => pdf);
  registerSugarTests(() => pdf);
  registerOneShotTests(() => pdf);
  registerRenderTests(() => pdf);
  registerDeterminismTests(() => pdf);
  registerCancelTests(
    () => pdf,
    skipExtract: 'OPFS pre-copies its sources: no source read marks the start',
  );
  registerCombinatorCancelTests(
    () => pdf,
    skipExtract: 'OPFS pre-copies its sources: no source read marks the start',
  );
  registerWebCancelChannelTests(() => pdf);
  registerAbortTests(() => pdf);
  registerLifecycleTests(
    () => Pdf(
      config: PdfConfig(
        webWorkerUrl:
            'http://localhost:$serverPort/web_assets/executor_worker.js',
        webIoMode: PdfIoMode.opfs,
      ),
    ),
  );
  registerInstanceTests(
    () => Pdf(
      config: PdfConfig(
        webWorkerUrl:
            'http://localhost:$serverPort/web_assets/executor_worker.js',
        webIoMode: PdfIoMode.opfs,
      ),
    ),
  );

  registerRuntimeSafetyTests(
    config: () => PdfConfig(
      webWorkerUrl:
          'http://localhost:$serverPort/web_assets/executor_worker.js',
      webIoMode: PdfIoMode.opfs,
    ),
  );

  PdfConfig executorConfig() => PdfConfig(
    webWorkerUrl: 'http://localhost:$serverPort/web_assets/executor_worker.js',
    webIoMode: PdfIoMode.opfs,
  );
  registerExecutorTests(
    () => Pdf(config: executorConfig()),
    config: executorConfig,
    memBytes: (executor) => (executor as WebExecutor).memBytes,
  );

  registerLaneProtocolTests(
    config: () => PdfConfig(
      webWorkerUrl:
          'http://localhost:$serverPort/web_assets/executor_worker.js',
      webIoMode: PdfIoMode.opfs,
    ),
  );

  registerWebLaneProtocolTests(
    workerUrl: () =>
        'http://localhost:$serverPort/web_assets/executor_worker.js',
    mode: PdfIoMode.opfs,
  );

  registerWebBackingTests(
    workerUrl: () =>
        'http://localhost:$serverPort/web_assets/executor_worker.js',
    mode: PdfIoMode.opfs,
  );

  registerWebAsyncifyAssetTests(
    workerUrl: () =>
        'http://localhost:$serverPort/web_assets/executor_worker.js',
    mode: PdfIoMode.opfs,
  );

  registerWebWasmPartsTests(
    workerUrl: () =>
        'http://localhost:$serverPort/web_assets/executor_worker.js',
    mode: PdfIoMode.opfs,
  );

  registerWebLargeOffsetTests(
    () => pdf,
    skip: 'OPFS pre-copies its sources: a 4 GiB source would be copied to disk',
  );

  registerDocStressTests(() => pdf);
  registerEditorStressTests(() => pdf);
  registerSugarStressTests(() => pdf);
  registerStandaloneStressTests(() => pdf);
  registerBuilderStressTests(() => pdf);
  registerInstanceStressTests(
    () => Pdf(
      config: PdfConfig(
        webWorkerUrl:
            'http://localhost:$serverPort/web_assets/executor_worker.js',
        webIoMode: PdfIoMode.opfs,
      ),
    ),
  );

  // LAST on purpose: its residue check sweeps the whole suite.
  registerWebPlatformTests(
    workerUrl: () =>
        'http://localhost:$serverPort/web_assets/executor_worker.js',
    mode: PdfIoMode.opfs,
    disposeSharedInstance: () => pdf.dispose(),
  );
}
