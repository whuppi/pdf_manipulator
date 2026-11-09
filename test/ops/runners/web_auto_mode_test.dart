// Auto-detection of the web I/O mode, on a page that never forces one (a
// forced mode is inherited by every later instance of the page, so this has a
// file of its own). Run: dart test test/ops/runners/web_auto_mode_test.dart -p chrome
//
// The order is Atomics, JSPI, OPFS, Asyncify: Atomics measured faster than JSPI
// on every source (t-562), and Asyncify about 2.2 times slower than OPFS, so it is chosen only where OPFS cannot run and the app
// shipped the Asyncify wasm (its entry in pdf_oxide_manifest.json). OPFS can
// run when `navigator.storage.getDirectory()` resolves. Chrome has no switch
// for JSPI, so the tests take `WebAssembly.Suspending` away from the page, as
// it is away in a browser older than Chrome 137, and make getDirectory reject,
// as it does in some private-browsing modes; a plain `-p chrome` page has no
// SharedArrayBuffer, a `-p chrome-coi` page has it (the Makefile runs both).

@TestOn('browser')
library;

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import '../../fixtures/handwritten.dart';
import '../../harness/audited_server.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

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

/// Runs [body] on a page whose origin private file system rejects, as it does
/// in some private-browsing modes: `navigator.storage.getDirectory` is
/// replaced on this realm's storage manager. Workers keep theirs.
Future<void> _withoutOpfs(Future<void> Function() body) async {
  final storage = web.window.navigator.storage as JSObject;
  storage['getDirectory'] = (() => Future<JSAny?>.error(
    StateError('no origin private file system'),
  ).toJS).toJS;
  try {
    await body();
  } finally {
    storage.delete('getDirectory'.toJS);
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

const _asyncifyModule = 'pdf_oxide_asyncify_bg';

void main() {
  test(
    'Atomics where the page has SharedArrayBuffer, even with JSPI',
    () async {
      final server = await AuditedServer.start(const []);
      expect(await _modeOf(server.workerUrl), PdfIoMode.atomics);
      await server.settle();
      expect(server.asked('$_asyncifyModule.0.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? false : 'the page has no SharedArrayBuffer',
  );

  test(
    'JSPI where the browser has it and the page has no SharedArrayBuffer, '
    'without asking for an Asyncify part',
    () async {
      final server = await AuditedServer.start(const []);
      expect(await _modeOf(server.workerUrl), PdfIoMode.jspi);
      await server.settle();
      expect(server.asked('$_asyncifyModule.0.wasm'), isEmpty);
      expect(server.asked('$_asyncifyModule.1.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: !_hasJspi
        ? 'this browser has no JSPI'
        : _hasSharedArrayBuffer
        ? 'the page has SharedArrayBuffer'
        : false,
  );

  test(
    'without JSPI and without SharedArrayBuffer, OPFS even when the app '
    'shipped Asyncify, never asking for an Asyncify part',
    () async {
      final server = await AuditedServer.start(const []);
      late PdfIoMode mode;
      await _withoutJspi(() async => mode = await _modeOf(server.workerUrl));
      expect(mode, PdfIoMode.opfs);
      await server.settle();
      expect(server.asked('$_asyncifyModule.0.wasm'), isEmpty);
      expect(server.asked('$_asyncifyModule.1.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );

  test(
    'where the origin private file system rejects, Asyncify when the app '
    'shipped it',
    () async {
      final server = await AuditedServer.start(const []);
      late PdfIoMode mode;
      await _withoutJspi(
        () => _withoutOpfs(() async => mode = await _modeOf(server.workerUrl)),
      );
      expect(mode, PdfIoMode.asyncify);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );

  test(
    'where the origin private file system rejects and the manifest has no '
    'Asyncify entry, a typed error naming both modes',
    () async {
      final server = await AuditedServer.start(
        const [],
        manifestWithout: const [_asyncifyModule],
      );
      final pdf = Pdf(config: PdfConfig(webWorkerUrl: server.workerUrl));
      await _withoutJspi(
        () => _withoutOpfs(
          () => expectLater(
            pdf.ensureInitialized(),
            throwsA(
              isA<PdfUnsupported>().having(
                (e) => e.message,
                'message',
                allOf(contains('opfs'), contains('asyncify')),
              ),
            ),
          ),
        ),
      );
      await pdf.dispose();
      await server.settle();
      expect(server.asked('.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );

  test(
    'a host that answers every unknown path with HTML and a manifest without '
    'the Asyncify entry never selects Asyncify or asks for one of its parts',
    () async {
      final server = await AuditedServer.start(
        const [],
        spa: true,
        manifestWithout: const [_asyncifyModule],
      );
      late PdfIoMode mode;
      await _withoutJspi(() async => mode = await _modeOf(server.workerUrl));
      expect(mode, PdfIoMode.opfs);
      await server.settle();
      expect(server.asked('$_asyncifyModule.0.wasm'), isEmpty);
      expect(server.asked('$_asyncifyModule.1.wasm'), isEmpty);
    },
    timeout: t(4),
    skip: _hasSharedArrayBuffer ? 'the page has SharedArrayBuffer' : false,
  );
}
