// CHARTER — this battery alone proves: on a page without SharedArrayBuffer a
// cancel reaches a running CPU-bound op through the job's blob URL within
// 500 ms, and that URL is revoked once, when the job is over. On a page with
// SharedArrayBuffer the cancel word is used and no URL exists, so these skip.

import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import '../../../fixtures/generated/fixtures.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

const _cancelAfter = Duration(milliseconds: 200);
const _latencyBound = Duration(milliseconds: 500);
const _minUncancelled = Duration(seconds: 1);

final String? _hasCancelWord =
    globalContext.hasProperty('SharedArrayBuffer'.toJS).toDart
    ? 'the page has SharedArrayBuffer: the cancel word is used, not a URL'
    : null;

/// Records the empty-blob URLs the page creates (the cancel URLs; the worker
/// script's blob is not empty) and every URL it revokes, while installed.
class _UrlTap {
  final created = <String>[];
  final revoked = <String>[];

  final JSObject _url = globalContext['URL']! as JSObject;
  late final JSFunction _create = _url['createObjectURL']! as JSFunction;
  late final JSFunction _revoke = _url['revokeObjectURL']! as JSFunction;

  void install() {
    final create = _create;
    final revoke = _revoke;
    _url['createObjectURL'] = ((JSObject blob) {
      final url = create.callAsFunction(_url, blob)! as JSString;
      if ((blob['size']! as JSNumber).toDartInt == 0) created.add(url.toDart);
      return url;
    }).toJS;
    _url['revokeObjectURL'] = ((JSString url) {
      revoked.add(url.toDart);
      return revoke.callAsFunction(_url, url);
    }).toJS;
  }

  void uninstall() {
    _url['createObjectURL'] = _create;
    _url['revokeObjectURL'] = _revoke;
  }
}

/// True when a fetch of [url] fails, as it does for a revoked blob URL.
Future<bool> _fetchFails(String url) async {
  try {
    await web.window.fetch(url.toJS).toDart;
    return false;
  } catch (_) {
    return true;
  }
}

void registerWebCancelChannelTests(Pdf Function() createPdf) {
  group('web cancel channel', () {
    // One engine job over a 5000-page document: it makes no source read
    // after the open (OPFS pre-copies) and the cancel is the only way out.
    Uint8List? big;

    Future<PdfDoc> openBig(Pdf pdf) async {
      if (big == null) {
        final merged = MemorySink();
        await pdf.merge([
          for (var i = 0; i < 5; i++) src(fThousandPage),
        ], merged);
        big = merged.takeBytes();
      }
      return pdf.open(src(big!));
    }

    PdfTask<void> startExtract(PdfDoc doc) =>
        doc.extractText(MemorySink(), pages: const PdfPages.all());

    Future<void> smallOp(PdfDoc doc) =>
        doc.extractText(MemorySink(), pages: const PdfPages.single(0));

    test(
      'a cancel stops a running CPU-bound op within 500 ms, and the Pdf '
      'serves the next op',
      () async {
        final pdf = createPdf();
        final doc = await openBig(pdf);

        await smallOp(doc); // the first call is slow: warm it up
        final whole = Stopwatch()..start();
        await startExtract(doc);
        whole.stop();
        expect(
          whole.elapsed,
          greaterThanOrEqualTo(_minUncancelled),
          reason:
              'the op must outlast the cancel point to prove anything: '
              'it ran ${whole.elapsedMilliseconds} ms uncancelled',
        );

        final task = startExtract(doc);
        await Future<void>.delayed(_cancelAfter);
        final sinceCancel = Stopwatch()..start();
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        // The caller is answered before the worker stops. The lane runs one
        // job at a time, so a warm small op (a few ms) ends once the worker
        // is free: the time to here is the engine's stop.
        await smallOp(doc);
        sinceCancel.stop();
        printOnFailure(
          'cancel latency: ${sinceCancel.elapsedMilliseconds} ms '
          '(uncancelled op: ${whole.elapsedMilliseconds} ms)',
        );
        expect(
          sinceCancel.elapsed,
          lessThan(_latencyBound),
          reason:
              'the worker kept running ${sinceCancel.elapsedMilliseconds} ms',
        );
        await doc.dispose();
      },
      timeout: t(60),
      skip: _hasCancelWord,
    );

    test(
      "every job's cancel URL is revoked once, and no longer fetches",
      () async {
        final tap = _UrlTap()..install();
        addTearDown(tap.uninstall);
        final pdf = createPdf();
        final doc = await openBig(pdf);

        // One job that ends by itself, one that is cancelled.
        await smallOp(doc);
        final task = startExtract(doc);
        await Future<void>.delayed(_cancelAfter);
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        await smallOp(doc);
        await doc.dispose();

        expect(tap.created, isNotEmpty);
        for (final url in tap.created) {
          expect(
            tap.revoked.where((r) => r == url),
            hasLength(1),
            reason: 'one revoke per created URL: $url',
          );
          expect(await _fetchFails(url), isTrue, reason: 'revoked: $url');
        }
        expect(tap.created.toSet(), hasLength(tap.created.length));
      },
      timeout: t(60),
      skip: _hasCancelWord,
    );
  });
}
