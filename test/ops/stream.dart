// Stream — render pages to images, extract embedded images.

import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/test_source_sink.dart';

void registerStreamTests(PdfBridge Function() b) {
  group('stream', () {
    test('render single page yields one result', () async {
      final pages = <RenderedPage>[];
      await for (final page in b().render(
        src(minimalPdf),
        pages: const PdfPages.single(0),
      )) {
        pages.add(page);
      }
      expect(pages, hasLength(1));
      expect(pages[0].data.length, greaterThan(0));
    });

    test('render all pages of multi-page PDF', () async {
      final mergeSink = TestSink();
      await b().merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final pages = <RenderedPage>[];
      await for (final page in b().render(
        src(twoPage),
        pages: const PdfPages.all(),
      )) {
        pages.add(page);
      }
      expect(pages, hasLength(2));
    });

    test('extractImages returns list (may be empty for minimal PDF)', () async {
      final images = <PdfImage>[];
      await for (final img in b().extractImages(
        src(minimalPdf),
        pages: const PdfPages.all(),
      )) {
        images.add(img);
      }
      expect(images, isA<List<PdfImage>>());
    });
  });
}
