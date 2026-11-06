// CHARTER — this battery alone proves how a page gets its wasm: the manifest
// is fetched once and says which modules exist, a browser fetches the parts of
// exactly the module its mode runs and never a whole `.wasm`, a part that is
// corrupt or short fails initialization with an error that names it, on the
// host's compile and on the worker's own, and a host that answers every
// unknown path with an HTML page, or has no manifest, never gets a guess.

import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/runtime/web/lease.dart';
import 'package:pdf_manipulator/src/runtime/web/protocol.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import '../../../fixtures/handwritten.dart';
import '../../../harness/audited_server.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

const _manifestPath = 'web_assets/pdf_oxide_manifest.json';

/// The module [mode] runs, and the file names of its parts that
/// `web_assets/` holds.
String _moduleOf(PdfIoMode mode) =>
    mode == PdfIoMode.asyncify ? 'pdf_oxide_asyncify_bg' : 'pdf_oxide_bg';

String _partPath(String module, int n) => 'web_assets/$module.$n.wasm';

void registerWebWasmPartsTests({
  required String Function() workerUrl,
  required PdfIoMode mode,
}) {
  final module = _moduleOf(mode);
  final other = mode == PdfIoMode.asyncify
      ? 'pdf_oxide_bg'
      : 'pdf_oxide_asyncify_bg';

  group('web wasm parts (${mode.name})', () {
    // A module is compiled once per page by its content; these tests watch
    // its parts being fetched, so each starts with nothing compiled, and the
    // page's compiled modules are put back after it.
    late Map<String, JSObject> aside;
    setUp(() => aside = WebExecutorBooter.setCompiledModulesAside());
    tearDown(() => WebExecutorBooter.restoreCompiledModules(aside));

    // Instances here carry their own server's URL, which becomes the page's
    // inherited default; point it back at the suite's.
    Future<void> restoreSuiteConfig() async {
      final restored = Pdf(
        config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
      );
      final doc = await restored.open(PdfInput(src(minimalPdf)));
      await doc.dispose();
      await restored.dispose();
    }

    Pdf pdfOn(AuditedServer server) => Pdf(
      config: PdfConfig(webWorkerUrl: server.workerUrl, webIoMode: mode),
    );

    test('the manifest is fetched once, the parts of one module once each, and '
        'no whole wasm', () async {
      final server = await AuditedServer.start(const []);
      final pdf = pdfOn(server);
      final doc = await pdf.open(PdfInput(src(minimalPdf)));
      expect(doc.pageCount, 1);
      expect(pdf.ioMode, mode);
      await doc.dispose();
      await pdf.dispose();
      await server.settle();

      expect(server.asked('pdf_oxide_manifest.json'), ['GET $_manifestPath']);
      final wasm = server.asked('.wasm');
      final pattern = RegExp('^GET web_assets/$module\\.(\\d+)\\.wasm\$');
      final numbers = [
        for (final r in wasm)
          int.parse(
            pattern.firstMatch(r)?.group(1) ??
                fail('a request outside the parts of $module: $r'),
          ),
      ]..sort();
      expect(numbers, isNotEmpty);
      expect(numbers, [
        for (var i = 0; i < numbers.length; i++) i,
      ], reason: 'every part once, from 0, and nothing else');
      expect(server.asked('$other.0.wasm'), isEmpty);
      expect(
        server.requests.where(
          (r) => r.endsWith('/pdf_oxide_bg.wasm') || r.endsWith('_bg.wasm'),
        ),
        isEmpty,
        reason: 'a whole wasm is never fetched',
      );
      await restoreSuiteConfig();
    }, timeout: t(4));

    for (final (fault, label) in [
      ('corrupt', 'a corrupt part'),
      ('truncate', 'a short part'),
    ]) {
      test('$label fails initialization with an error naming it', () async {
        final part = _partPath(module, 0);
        final server = await AuditedServer.start(
          const [],
          corrupt: fault == 'corrupt' ? [part] : const [],
          truncate: fault == 'truncate' ? [part] : const [],
        );
        final pdf = pdfOn(server);
        await expectLater(
          pdf.open(PdfInput(src(minimalPdf))),
          throwsA(
            predicate(
              (e) => e.toString().contains('$module.0.wasm'),
              'an error naming $module.0.wasm',
            ),
          ),
        );
        await pdf.dispose();
        await restoreSuiteConfig();
      }, timeout: t(12));

      test(
        '$label fails initialization on a page without crypto.subtle',
        () async {
          // A page that is not a secure context has no crypto.subtle; the
          // part is still checked, by package:crypto's digest.
          final crypto = web.window.crypto as JSObject;
          globalContext
              .getProperty<JSObject>('Object'.toJS)
              .callMethod(
                'defineProperty'.toJS,
                crypto,
                'subtle'.toJS,
                {'value': null, 'configurable': true}.jsify(),
              );
          addTearDown(() => crypto.delete('subtle'.toJS));
          expect(crypto['subtle'], isNull);
          final part = _partPath(module, 0);
          final server = await AuditedServer.start(
            const [],
            corrupt: fault == 'corrupt' ? [part] : const [],
            truncate: fault == 'truncate' ? [part] : const [],
          );
          final pdf = pdfOn(server);
          await expectLater(
            pdf.open(PdfInput(src(minimalPdf))),
            throwsA(
              predicate(
                (e) => e.toString().contains('$module.0.wasm'),
                'an error naming $module.0.wasm',
              ),
            ),
          );
          await pdf.dispose();
          crypto.delete('subtle'.toJS);
          await restoreSuiteConfig();
        },
        timeout: t(12),
      );

      test(
        '$label fails the worker\'s own compile with an error naming it',
        () async {
          final part = _partPath(module, 0);
          final server = await AuditedServer.start(
            const [],
            corrupt: fault == 'corrupt' ? [part] : const [],
            truncate: fault == 'truncate' ? [part] : const [],
          );
          final outcome = await _bootWorker(server.workerUrl, module, mode);
          expect(outcome.type, 'error');
          expect(outcome.message, contains('$module.0.wasm'));
        },
        timeout: t(4),
      );
    }

    test(
      'the worker compiles its own module from the manifest entry',
      () async {
        final server = await AuditedServer.start(const []);
        final outcome = await _bootWorker(server.workerUrl, module, mode);
        expect(outcome.type, 'ready', reason: outcome.message);
        await server.settle();
        expect(server.asked('$other.0.wasm'), isEmpty);
        expect(
          server.requests.where((r) => r.endsWith('_bg.wasm')),
          isEmpty,
          reason: 'a whole wasm is never fetched',
        );
      },
      timeout: t(4),
    );

    test('a host with no manifest is a typed error naming the file and '
        'the web setup', () async {
      final server = await AuditedServer.start([_manifestPath]);
      final pdf = pdfOn(server);
      await expectLater(
        pdf.ensureInitialized(),
        throwsA(
          isA<PdfUnsupported>().having(
            (e) => e.message,
            'message',
            allOf(
              contains('pdf_oxide_manifest.json'),
              contains('pdf_manipulator:setup'),
            ),
          ),
        ),
      );
      await server.settle();
      expect(server.asked('.wasm'), isEmpty);
      await pdf.dispose();
    }, timeout: t(4));

    test('a single-page-app host that answers the manifest with its HTML page '
        'is the same typed error', () async {
      final server = await AuditedServer.start([_manifestPath], spa: true);
      final pdf = pdfOn(server);
      await expectLater(
        pdf.ensureInitialized(),
        throwsA(
          isA<PdfUnsupported>().having(
            (e) => e.message,
            'message',
            contains('pdf_oxide_manifest.json'),
          ),
        ),
      );
      await server.settle();
      expect(server.asked('.wasm'), isEmpty);
      await pdf.dispose();
    }, timeout: t(4));

    if (mode == PdfIoMode.asyncify) {
      test(
        'a manifest without the Asyncify entry, on a host that answers every '
        'unknown path with HTML, is a typed error and no part is requested',
        () async {
          final server = await AuditedServer.start(
            [
              _partPath('pdf_oxide_asyncify_bg', 0),
              _partPath('pdf_oxide_asyncify_bg', 1),
            ],
            spa: true,
            manifestWithout: const ['pdf_oxide_asyncify_bg'],
          );
          final pdf = pdfOn(server);
          await expectLater(
            pdf.ensureInitialized(),
            throwsA(
              isA<PdfUnsupported>().having(
                (e) => e.message,
                'message',
                allOf(
                  contains('asyncify'),
                  contains('pdf_oxide_manifest.json'),
                ),
              ),
            ),
          );
          await server.settle();
          expect(server.asked('.wasm'), isEmpty);
          await pdf.dispose();
        },
        timeout: t(4),
      );
    } else {
      test(
        'a manifest without the Asyncify entry, on a host that answers every '
        'unknown path with HTML, never requests an Asyncify part',
        () async {
          final server = await AuditedServer.start(
            const [],
            spa: true,
            manifestWithout: const ['pdf_oxide_asyncify_bg'],
          );
          final pdf = pdfOn(server);
          final doc = await pdf.open(PdfInput(src(minimalPdf)));
          expect(doc.pageCount, 1);
          expect(pdf.ioMode, mode);
          await doc.dispose();
          await pdf.dispose();
          await server.settle();
          expect(server.asked('pdf_oxide_asyncify_bg.0.wasm'), isEmpty);
          expect(server.asked('pdf_oxide_asyncify_bg.1.wasm'), isEmpty);
          await restoreSuiteConfig();
        },
        timeout: t(4),
      );
    }
  });
}

/// How a raw worker's boot ended: `ready`, or `error` with its message.
typedef _BootOutcome = ({String type, String message});

/// Boots the executor script at [workerUrl] by itself, without a module the
/// host compiled, so the worker compiles from the manifest entry of [module]
/// the way it does when the host's compile failed.
Future<_BootOutcome> _bootWorker(
  String workerUrl,
  String module,
  PdfIoMode mode,
) async {
  final base = workerUrl.substring(0, workerUrl.lastIndexOf('/') + 1);
  final response = await web.window
      .fetch('${base}pdf_oxide_manifest.json'.toJS)
      .toDart;
  final manifest =
      jsonDecode((await response.text().toDart).toDart) as Map<String, dynamic>;
  final blob = web.Blob(
    [
      "import('$workerUrl').catch((e) => postMessage({type: 'bootfailed', error: String(e)}));"
          .toJS,
    ].toJS,
    web.BlobPropertyBag(type: 'application/javascript'),
  );
  final worker = web.Worker(
    web.URL.createObjectURL(blob).toJS,
    web.WorkerOptions(type: 'module'),
  );
  final outcome = Completer<_BootOutcome>();
  worker.onmessage = ((web.MessageEvent event) {
    final data = event.data as JSObject?;
    final type = (data?['type'] as JSString?)?.toDart;
    if (type == 'booted') {
      final init = JSObject()
        ..['type'] = 'init'.toJS
        ..['ioMode'] = mode.name.toJS
        ..['baseUrl'] = base.toJS
        ..['wasmEntry'] = (manifest[module] as Object?).jsify()
        ..['protocol'] = () {
          final p = JSObject();
          laneProtocolCodes().forEach((k, v) => p[k] = v.toJS);
          return p;
        }();
      worker.postMessage(init);
    } else if (type == 'ready') {
      outcome.complete((type: 'ready', message: ''));
    } else if (type == 'error' || type == 'bootfailed') {
      outcome.complete((
        type: 'error',
        message: (data!['error'] as JSString?)?.toDart ?? '',
      ));
    }
  }).toJS;
  try {
    return await outcome.future;
  } finally {
    worker.terminate();
  }
}
