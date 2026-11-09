// CHARTER — this battery alone proves what a page asks the server for and
// when the Asyncify mode is refused: a browser fetches only the wasm its mode
// runs, a page whose app did not ship the Asyncify wasm (its manifest has no
// entry for it) never requests it, and forcing Asyncify without it is a typed
// error when the instance is initialized.

import 'dart:async';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../../fixtures/handwritten.dart';
import '../../../harness/audited_server.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

const _asyncifyParts = [
  'web_assets/pdf_oxide_asyncify_bg.0.wasm',
  'web_assets/pdf_oxide_asyncify_bg.1.wasm',
];

void registerWebAsyncifyAssetTests({
  required String Function() workerUrl,
  required PdfIoMode mode,
}) {
  group('web asyncify assets (${mode.name})', () {
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

    if (mode != PdfIoMode.asyncify) {
      test('a page without the Asyncify wasm never asks for it', () async {
        final server = await AuditedServer.start(
          _asyncifyParts,
          manifestWithout: const ['pdf_oxide_asyncify_bg'],
        );
        final pdf = Pdf(
          config: PdfConfig(webWorkerUrl: server.workerUrl, webIoMode: mode),
        );
        final doc = await pdf.open(PdfInput(src(minimalPdf)));
        expect(doc.pageCount, 1);
        expect(pdf.ioMode, mode);
        await doc.dispose();
        await pdf.dispose();
        await server.settle();

        expect(server.asked('pdf_oxide_asyncify_bg.0.wasm'), isEmpty);
        expect(server.asked('pdf_oxide_asyncify_bg.1.wasm'), isEmpty);
        await restoreSuiteConfig();
      }, timeout: t(4));
    }

    test(
      'forcing Asyncify without its wasm is a typed error at initialization',
      () async {
        final server = await AuditedServer.start(
          _asyncifyParts,
          manifestWithout: const ['pdf_oxide_asyncify_bg'],
        );
        final pdf = Pdf(
          config: PdfConfig(
            webWorkerUrl: server.workerUrl,
            webIoMode: PdfIoMode.asyncify,
          ),
        );
        await expectLater(
          pdf.ensureInitialized(),
          throwsA(
            isA<PdfUnsupported>().having(
              (e) => e.message,
              'message',
              allOf(contains('asyncify'), contains('pdf_oxide_manifest.json')),
            ),
          ),
        );
        await server.settle();
        expect(
          server.asked('.wasm'),
          isEmpty,
          reason: 'the manifest is the only thing the page asks',
        );
        await pdf.dispose();
        if (mode != PdfIoMode.asyncify) await restoreSuiteConfig();
      },
      timeout: t(4),
    );

    if (mode == PdfIoMode.asyncify) {
      test(
        'Asyncify mode fetches its own wasm and not the normal one',
        () async {
          final server = await AuditedServer.start(const []);
          final pdf = Pdf(
            config: PdfConfig(
              webWorkerUrl: server.workerUrl,
              webIoMode: PdfIoMode.asyncify,
            ),
          );
          final doc = await pdf.open(PdfInput(src(minimalPdf)));
          expect(doc.pageCount, 1);
          await doc.dispose();
          await pdf.dispose();
          await server.settle();

          // The page compiles each wasm once, so the suite's earlier use of
          // this file may have served it; what this instance must never do
          // is ask for the normal one. The engine only starts on the
          // Asyncify build: the worker refuses a module without its exports.
          expect(server.asked('pdf_oxide_bg.0.wasm'), isEmpty);
          expect(server.asked('pdf_oxide_bg.1.wasm'), isEmpty);

          await restoreSuiteConfig();
        },
        timeout: t(4),
      );
    }
  });
}
