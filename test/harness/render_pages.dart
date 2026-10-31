// Renders pages into MemorySinks and hands back what each one received.
// A page's descriptor arrives before any of its bytes, so the helper
// records the order the factory and the sinks were used in too.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// A page rendered to memory: its pixel size and the bytes its sink got.
typedef RenderedBytes = ({int width, int height, Uint8List data});

/// Renders [pages] of [reads] (a `PdfDoc` or a one-shot) into MemorySinks and
/// returns one [RenderedBytes] per page, in page order.
Future<List<RenderedBytes>> renderToMemory(
  PdfReads reads, {
  required PdfPages pages,
  PdfRenderSize size = const PdfRenderSize.dpi(150),
  PdfRenderFormat format = PdfRenderFormat.png,
}) async {
  final sinks = <MemorySink>[];
  final described = await reads.render(
    (page) {
      final sink = MemorySink();
      sinks.add(sink);
      return sink;
    },
    pages: pages,
    size: size,
    format: format,
  );
  return [
    for (var i = 0; i < described.length; i++)
      (
        width: described[i].width,
        height: described[i].height,
        data: sinks[i].takeBytes(),
      ),
  ];
}
