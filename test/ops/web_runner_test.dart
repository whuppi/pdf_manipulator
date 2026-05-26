// Runs all shared ops through the public Pdf API (web/WASM path).
// The SAME tests run through native in native_runner_test.dart.
//
// Web needs an asset server because dart test -p chrome runs tests on
// a different origin than the package root. spawnHybridUri starts a
// shelf server on the VM side; fetchAsBlobUrl turns cross-origin JS
// into same-origin blob URLs the browser can load as Workers.

@TestOn('browser')
library;

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../transport/web/web_test_helper.dart';
import 'open.dart';
import 'merge.dart';
import 'structural.dart';
import 'content.dart';
import 'stream.dart';
import 'security.dart';
import 'editor.dart';
import 'builder.dart';
import 'error.dart';
import 'lifecycle.dart';
import 'coverage_check.dart';
import 'stress.dart';
import 'timing_test.dart';

void main() {
  late Pdf pdf;
  late String coordinatorBlobUrl;
  late String wasmWorkerUrl;

  setUpAll(() async {
    final channel = spawnHybridUri('/test/helpers/asset_server.dart');
    final port = ((await channel.stream.first) as num).toInt();
    coordinatorBlobUrl = await fetchAsBlobUrl(
        'http://localhost:$port/web_assets/coordinator.js');
    wasmWorkerUrl = 'http://localhost:$port/web_assets/worker.js';
    pdf = Pdf(config: PdfConfig(
      webCoordinatorUrl: coordinatorBlobUrl,
      webWorkerUrl: wasmWorkerUrl,
    ));
  });

  tearDownAll(() => pdf.dispose());

  registerOpenTests(() => pdf);
  registerMergeTests(() => pdf);
  registerStructuralTests(() => pdf);
  registerContentTests(() => pdf);
  registerStreamTests(() => pdf);
  registerSecurityTests(() => pdf);
  registerEditorTests(() => pdf);
  registerBuilderTests(() => pdf);
  registerErrorTests(() => pdf);
  registerLifecycleTests(() => Pdf(config: PdfConfig(
    webCoordinatorUrl: coordinatorBlobUrl,
    webWorkerUrl: wasmWorkerUrl,
  )));
  registerTimingTests(() => pdf);
  registerStressTests(() => pdf);
  registerCoverageCheck();
}
