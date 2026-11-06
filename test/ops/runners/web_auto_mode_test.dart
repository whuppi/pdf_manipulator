// Auto-detection of the web I/O mode, on a page that never forces one (a
// forced mode is inherited by every later instance of the page, so this has a
// file of its own). Run: dart test test/ops/runners/web_auto_mode_test.dart -p chrome
//
// The order is JSPI, Atomics, Asyncify when the app shipped its wasm, OPFS.
// Chrome has no switch for JSPI, so the tests take `WebAssembly.Suspending`
// away from the page, as it is away in a browser older than Chrome 137; a
// plain `-p chrome` page has no SharedArrayBuffer.

@TestOn('browser')
library;

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/audited_server.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

const _asyncifyWasm = 'web_assets/pdf_oxide_asyncify_bg.wasm';

final bool _hasSharedArrayBuffer = globalContext
    .hasProperty('SharedArrayBuffer'.toJS)
    .toDart;

final bool _hasJspi = (globalContext['WebAssembly']! as JSObject)
    .hasProperty('Suspending'.toJS)
    .toDart;

/// Runs [body] on a page without JSPI: `WebAssembly.Suspending` is removed
/// from this realm. Workers are other realms and keep theirs; auto-detection
/// never reads those.
Future<void> _withoutJspi(Future<void> Function() body) async {
  final wasm = globalContext['WebAssembly']! as JSObject;
  final suspending = wasm['Suspending'];
  wasm.delete('Suspending'.toJS);
  try {
    await body();
  } finally {
    if (suspending != null) wasm['Suspending'] = suspending;
  }
}

Future<PdfIoMode> _modeOf(String workerUrl) async {
  final pdf = Pdf(config: PdfConfig(webWorkerUrl: workerUrl));
  final doc = await pdf.open(PdfInput(src(minimalPdf)));
  expect(doc.pageCount, 1);
  await doc.dispose();
  final mode = pdf.ioMode!;
  await pdf.dispose();
  return mode;
}

void main() {
  test(
    'JSPI where the browser has it, without asking for the Asyncify wasm',
    () async {
      final server = await AuditedServer.start([_asyncifyWasm]);
      expect(await _modeOf(server.workerUrl), PdfIoMode.jspi);
      await server.settle();
      expect(server.asked('pdf_oxide_asyncify_bg.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: _hasJspi ? false : 'this browser has no JSPI',
  );

  test(
    'without JSPI and without SharedArrayBuffer, Asyncify when the app shipped it',
    () async {
      final server = await AuditedServer.start(const []);
      late PdfIoMode mode;
      await _withoutJspi(() async => mode = await _modeOf(server.workerUrl));
      expect(mode, PdfIoMode.asyncify);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );

  test(
    'without JSPI and without SharedArrayBuffer, OPFS when the app did not '
    'ship Asyncify, having only asked whether the file exists',
    () async {
      final server = await AuditedServer.start([_asyncifyWasm]);
      late PdfIoMode mode;
      await _withoutJspi(() async => mode = await _modeOf(server.workerUrl));
      expect(mode, PdfIoMode.opfs);
      await server.settle();
      expect(server.asked('pdf_oxide_asyncify_bg.wasm'), [
        'HEAD $_asyncifyWasm',
      ]);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );
}
