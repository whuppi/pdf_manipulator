// CHARTER — this battery alone proves: an LZWDecode content stream decodes
// to the bytes it was encoded from, whichever code-width switch its
// /EarlyChange entry names (1 when the entry is absent), across every code
// width and clear code, and that the other switch does not decode it.
//
// Diet: one handwritten page whose content is long enough to fill the
// 4096-entry LZW table several times, compressed here by `lzwEncode`.
// Presence proofs are SEMANTIC: the content shows one token per line, so
// the decoded stream is proven by extracting the page's text.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/extract_text.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerLzwTests(Pdf Function() createPdf) {
  group('lzw', () {
    final early = lzwEncode(lzwPageContent, earlyChange: true);
    final late = lzwEncode(lzwPageContent, earlyChange: false);

    /// The tokens the page of [pdfBytes] shows, in text order.
    Future<List<String>> tokensOf(Uint8List pdfBytes) async {
      final doc = await createPdf().open(PdfInput(src(pdfBytes)));
      try {
        final text = await extractTextToString(doc);
        return [for (final m in RegExp('[A-Z]{8}').allMatches(text)) m[0]!];
      } finally {
        await doc.dispose();
      }
    }

    /// Like [tokensOf], with a refused page read as no tokens.
    Future<List<String>> tokensOrNone(Uint8List pdfBytes) async {
      try {
        return await tokensOf(pdfBytes);
      } on PdfError {
        return const [];
      }
    }

    test('the fixture crosses every code width and holds clear codes', () {
      for (final encoded in [early, late]) {
        expect(encoded.widths, {9, 10, 11, 12});
        expect(encoded.clears, greaterThan(2));
      }
    });

    test(
      '/EarlyChange 1 decodes across every code width and clear code',
      () async {
        final pdf = lzwPagePdf(
          early.bytes,
          decodeParms: '<< /EarlyChange 1 >>',
        );
        expect(await tokensOf(pdf), lzwPageTokens);
      },
      timeout: t(2),
    );

    test(
      '/EarlyChange 0 decodes across every code width and clear code',
      () async {
        final pdf = lzwPagePdf(late.bytes, decodeParms: '<< /EarlyChange 0 >>');
        expect(await tokensOf(pdf), lzwPageTokens);
      },
      timeout: t(2),
    );

    test('an absent /EarlyChange is 1', () async {
      expect(await tokensOf(lzwPagePdf(early.bytes)), lzwPageTokens);
    }, timeout: t(2));

    test('a stream read with the wrong switch is not decoded as the right '
        'one', () async {
      // Either an error or other text, never the original.
      final lateAsEarly = lzwPagePdf(late.bytes);
      expect(await tokensOrNone(lateAsEarly), isNot(lzwPageTokens));
      final earlyAsLate = lzwPagePdf(
        early.bytes,
        decodeParms: '<< /EarlyChange 0 >>',
      );
      expect(await tokensOrNone(earlyAsLate), isNot(lzwPageTokens));
    }, timeout: t(2));
  });
}
