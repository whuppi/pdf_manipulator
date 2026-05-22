import 'package:test/test.dart';
import 'package:pdf_manipulator/src/bridge/native/native_bridge.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';

import '../../helpers/pdf_fixtures.dart';
import '../test_helpers.dart';

void main() {
  late NativeBridge bridge;

  setUp(() { bridge = NativeBridge(); });
  tearDown(() => bridge.dispose());

  group('NativeBridge.render (streaming)', () {
    test('renders single page of minimal PDF', () async {
      final source = src(minimalPdf);
      final pages = <RenderedPage>[];

      await for (final page in bridge.render(source,
          pages: const PdfPages.single(0),
          size: const PdfRenderSize(maxWidth: 100, maxHeight: 150))) {
        pages.add(page);
      }

      expect(pages.length, 1);
      expect(pages[0].width, greaterThan(0));
      expect(pages[0].height, greaterThan(0));
      expect(pages[0].data.length, greaterThan(0));
    });

    test('renders all pages of multi-page PDF', () async {
      final threePages = await buildThreePagePdf();
      final source = src(threePages);
      final pages = <RenderedPage>[];

      await for (final page in bridge.render(source,
          pages: const PdfPages.all(),
          size: const PdfRenderSize(maxWidth: 100, maxHeight: 150))) {
        pages.add(page);
      }

      expect(pages.length, 3);
      for (final p in pages) {
        expect(p.width, greaterThan(0));
        expect(p.height, greaterThan(0));
      }
    });
  });

  group('NativeBridge.extractImages (streaming)', () {
    test('returns empty stream for PDF with no images', () async {
      final source = src(minimalPdf);
      final images = <PdfImage>[];

      await for (final img in bridge.extractImages(source,
          pages: const PdfPages.single(0))) {
        images.add(img);
      }

      expect(images, isEmpty);
    });
  });
}
