// Extracts images into MemorySinks and hands back what each one received. An
// image's descriptor arrives before any of its bytes, so the helper records
// the sinks in the order the factory was asked for them.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// An image extracted to memory: its descriptor and the bytes its sink got.
typedef ExtractedBytes = ({PdfImage image, Uint8List data});

/// Extracts the images of [pages] of [reads] (a `PdfDoc` or a one-shot) into
/// MemorySinks and returns one [ExtractedBytes] per image, in the order found.
Future<List<ExtractedBytes>> extractToMemory(
  PdfReads reads, {
  required PdfPages pages,
}) async {
  final sinks = <MemorySink>[];
  final described = await reads.extractImages((image) {
    final sink = MemorySink();
    sinks.add(sink);
    return sink;
  }, pages: pages);
  return [
    for (var i = 0; i < described.length; i++)
      (image: described[i], data: sinks[i].takeBytes()),
  ];
}

/// The descriptors of the images of [pages] of [reads], their bytes dropped.
Future<List<PdfImage>> extractedImages(
  PdfReads reads, {
  required PdfPages pages,
}) => reads.extractImages((image) => MemorySink(), pages: pages);
