// Runs all shared ops through WebBridge (browser path).
// The SAME tests run through NativeBridge in native_runner_test.dart.

@TestOn('browser')
library;

import 'package:pdf_manipulator/src/transport/web/bridge.dart';
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

void main() {
  late WebBridge bridge;
  late String coordinatorBlobUrl;
  late String wasmWorkerUrl;

  setUpAll(() async {
    final channel = spawnHybridUri('/test/helpers/asset_server.dart');
    final port = ((await channel.stream.first) as num).toInt();
    coordinatorBlobUrl = await fetchAsBlobUrl(
        'http://localhost:$port/web_assets/coordinator.js');
    wasmWorkerUrl = 'http://localhost:$port/web_assets/wasm_worker.js';
    bridge = WebBridge(
      coordinatorUrl: coordinatorBlobUrl,
      wasmWorkerUrl: wasmWorkerUrl,
    );
  });

  tearDownAll(() => bridge.dispose());

  registerOpenTests(() => bridge);
  registerMergeTests(() => bridge);
  registerStructuralTests(() => bridge);
  registerContentTests(() => bridge);
  registerStreamTests(() => bridge);
  registerSecurityTests(() => bridge);
  registerEditorTests(() => bridge);
  registerBuilderTests(() => bridge);
  registerErrorTests(() => bridge);
  registerLifecycleTests(() => WebBridge(
    coordinatorUrl: coordinatorBlobUrl,
    wasmWorkerUrl: wasmWorkerUrl,
  ));
  registerStressTests(() => bridge);
  registerCoverageCheck();
}
