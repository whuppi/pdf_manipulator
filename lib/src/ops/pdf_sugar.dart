// Sugar — convenience wrappers using only these four surface files:
//   pdf_editor.dart    (edit → mutate → save)
//   pdf_builder.dart   (build → pages → save)
//   pdf_doc.dart       (open → query → dispose)
//   pdf_standalone.dart (one-shot ops)
//
// Do NOT import bridge, transport, protocol, or any internal layer.
// Sugar composes the public consumer API only.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/ops/pdf.dart';
import 'package:pdf_manipulator/src/ops/pdf_standalone.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Convenience wrappers — common multi-step PDF operations in one call.
extension PdfSugar on Pdf {
  // ── Structural ──

  /// Merges multiple PDFs into a single document.
  Future<void> merge(List<DataSource> inputs, DataSink output) async {
    if (inputs.isEmpty) throw ArgumentError('inputs must not be empty');
    final editor = await edit(inputs.first);
    for (var i = 1; i < inputs.length; i++) {
      await editor.mergeFrom(inputs[i]);
    }
    await editor.save(output);
    await editor.dispose();
  }

  /// Splits a PDF into chunks of [every] pages each.
  Future<void> split(
    DataSource source,
    DataSink Function(int index) sinkFactory, {
    required int every,
  }) async {
    if (every < 1) throw ArgumentError('every must be >= 1');
    final doc = await open(source);
    try {
      final total = doc.pageCount;
      var chunkIndex = 0;
      for (var start = 0; start < total; start += every) {
        final end = (start + every).clamp(0, total);
        await extractPages(source, sinkFactory(chunkIndex),
            pages: List.generate(end - start, (i) => start + i));
        chunkIndex++;
      }
    } finally {
      await doc.dispose();
    }
  }

  /// Splits a PDF so each chunk stays under [maxBytes].
  Future<List<int>> splitBySize(
    DataSource source,
    DataSink Function(int index) sinkFactory, {
    required int maxBytes,
  }) async {
    if (maxBytes < 1) throw ArgumentError('maxBytes must be >= 1');
    final doc = await open(source);
    try {
      final total = doc.pageCount;

      final chunkSizes = <int>[];
      var chunkStart = 0;
      var chunkIndex = 0;

      while (chunkStart < total) {
        final remaining = total - chunkStart;

        var probeSize = 1;
        var lastFit = 1;
        while (probeSize <= remaining) {
          final counter = _ByteCountSink();
          await extractPages(source, counter,
              pages: List.generate(probeSize, (j) => chunkStart + j));
          if (counter.length <= maxBytes) {
            lastFit = probeSize;
            probeSize *= 2;
          } else {
            break;
          }
        }

        var lo = lastFit;
        var hi = probeSize.clamp(1, remaining);
        var bestEnd = lastFit;

        if (lo < hi) {
          while (lo <= hi) {
            final mid = (lo + hi) ~/ 2;
            final counter = _ByteCountSink();
            await extractPages(source, counter,
                pages: List.generate(mid, (j) => chunkStart + j));
            if (counter.length <= maxBytes) {
              bestEnd = mid;
              lo = mid + 1;
            } else {
              hi = mid - 1;
            }
          }
        }

        final sink = sinkFactory(chunkIndex);
        final counter = _ByteCountSinkWrapper(sink);
        await extractPages(source, counter,
            pages: List.generate(bestEnd, (j) => chunkStart + j));
        chunkSizes.add(counter.length);

        chunkStart += bestEnd;
        chunkIndex++;
      }

      return chunkSizes;
    } finally {
      await doc.dispose();
    }
  }

  /// Splits a PDF at top-level bookmark boundaries.
  Future<void> splitByBookmarks(
    DataSource source,
    DataSink Function(int index) sinkFactory, {
    String? password,
  }) async {
    final doc = await open(source, password: password);
    final splits = await doc.planSplitByBookmarks();
    await doc.dispose();

    for (var i = 0; i < splits.length; i++) {
      final split = splits[i];
      await extractPages(source, sinkFactory(i),
          pages: List.generate(
              split.endPage - split.startPage, (j) => split.startPage + j),
          password: password);
    }
  }


  /// Removes the specified [pages] from the document.
  Future<void> deletePages(
    DataSource source,
    DataSink output, {
    required List<int> pages,
  }) async {
    final editor = await edit(source);
    for (final p in pages.toList()
      ..sort((a, b) => b.compareTo(a))) {
      await editor.deletePage(p);
    }
    await editor.save(output);
    await editor.dispose();
  }

  /// Reorders pages according to [order] (list of page indices).
  Future<void> reorderPages(
    DataSource source,
    DataSink output, {
    required List<int> order,
  }) async {
    final editor = await edit(source);
    await editor.selectPages(order);
    await editor.save(output);
    await editor.dispose();
  }

  /// Moves a single page from index [from] to index [to].
  Future<void> movePage(
    DataSource source,
    DataSink output, {
    required int from,
    required int to,
  }) async {
    final editor = await edit(source);
    await editor.movePage(from: from, to: to);
    await editor.save(output);
    await editor.dispose();
  }

  /// Rotates specific pages by their mapped degree values.
  Future<void> rotatePages(
    DataSource source,
    DataSink output, {
    required Map<int, int> pages,
  }) async {
    final editor = await edit(source);
    for (final entry in pages.entries) {
      await editor.rotatePage(entry.key, degrees: entry.value);
    }
    await editor.save(output);
    await editor.dispose();
  }

  /// Rotates all pages by [degrees] (must be a multiple of 90).
  Future<void> rotateAllPages(
    DataSource source,
    DataSink output, {
    required int degrees,
  }) async {
    final editor = await edit(source);
    await editor.rotateAllPages(degrees: degrees);
    await editor.save(output);
    await editor.dispose();
  }

  // ── Content ──

  /// Flattens all form fields in the document into static content.
  Future<void> flattenForms(DataSource source, DataSink output) async {
    final editor = await edit(source);
    await editor.flattenForms();
    await editor.save(output);
    await editor.dispose();
  }

  /// Applies all pending redaction marks, permanently removing content.
  Future<void> applyRedactions(DataSource source, DataSink output) async {
    final editor = await edit(source);
    await editor.applyRedactions();
    await editor.save(output);
    await editor.dispose();
  }

  /// Embeds a file attachment into the PDF.
  Future<void> embedFile(
    DataSource source,
    DataSink output, {
    required String name,
    required DataSource fileData,
  }) async {
    final editor = await edit(source);
    await editor.embedFile(name, fileData);
    await editor.save(output);
    await editor.dispose();
  }

  /// Erases content within the specified [regions] on [page].
  Future<void> eraseRegions(
    DataSource source,
    DataSink output, {
    required int page,
    required List<PdfRect> regions,
  }) async {
    final editor = await edit(source);
    await editor.eraseRegions(page, regions);
    await editor.save(output);
    await editor.dispose();
  }

  /// Compresses the PDF by optimizing images and optionally garbage-collecting.
  Future<void> compress(
    DataSource source,
    DataSink output, {
    int imageQuality = 75,
    bool garbageCollect = true,
  }) async {
    final editor = await edit(source);
    await editor.optimizeImages(quality: imageQuality);
    await editor.save(output,
        options: PdfSaveOptions.fullRewrite(
          compress: true,
          garbageCollect: garbageCollect,
        ));
    await editor.dispose();
  }

  // ── Security ──

  /// Adds a text watermark to the specified [pages] (default: all).
  Future<void> watermark(
    DataSource source,
    DataSink output, {
    required String text,
    PdfPages pages = const PdfPages.all(),
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) async {
    final editor = await edit(source);
    final indices = switch (pages) {
      PdfAllPages() => const <int>[-1],
      PdfSinglePage(:final index) => [index],
      PdfPageList(:final indices) => indices,
      PdfPageRange(:final start, :final end) =>
        List.generate(end - start, (i) => start + i),
    };
    for (final i in indices) {
      await editor.addWatermark(i, text, style: style, position: position, layer: layer);
    }
    await editor.save(output);
    await editor.dispose();
  }

  /// Encrypts a PDF with the given [encryption] configuration.
  Future<void> encrypt(
    DataSource source,
    DataSink output, {
    required PdfEncryptionConfig encryption,
  }) async {
    final editor = await edit(source);
    await editor.save(output, options: PdfSaveOptions.fullRewrite(encryption: encryption));
    await editor.dispose();
  }

  /// Decrypts a password-protected PDF and saves without encryption.
  Future<void> decrypt(
    DataSource source,
    DataSink output, {
    required String password,
  }) async {
    final editor = await edit(source, password: password);
    await editor.save(output,
        options: const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.remove()));
    await editor.dispose();
  }

  // ── Stamps ──

  /// Adds a predefined stamp to a single [page].
  Future<void> addStamp(
    DataSource source,
    DataSink output, {
    required int page,
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) async {
    final editor = await edit(source);
    await editor.addStamp(page, type: type, rect: rect, opacity: opacity);
    await editor.save(output);
    await editor.dispose();
  }

  /// Adds a custom image stamp to a single [page].
  Future<void> addImageStamp(
    DataSource source,
    DataSink output, {
    required int page,
    required DataSource imageData,
    required PdfRect rect,
    double opacity = 1.0,
  }) async {
    final editor = await edit(source);
    await editor.addImageStamp(page, imageData, rect: rect, opacity: opacity);
    await editor.save(output);
    await editor.dispose();
  }

  // ── Conversion ──

  /// Converts a PDF to PDF/A at the given conformance [level].
  Future<void> convertToPdfA(
    DataSource source,
    DataSink output, {
    int level = 1,
    String? password,
  }) async {
    final editor = await edit(source, password: password);
    await editor.convertToPdfA(level: level);
    await editor.save(output);
    await editor.dispose();
  }

  // ── Builder sugar ──

  /// Creates a PDF from a list of images, one per A4 page.
  Future<void> imagesToPdf(List<DataSource> images, DataSink output) async {
    final builder = await build();
    for (final img in images) {
      final page = await builder.addA4Page();
      await page.image(img, const PdfRect(x: 0, y: 0, width: 595, height: 842));
      await page.done();
    }
    await builder.save(output);
    await builder.dispose();
  }
}

class _ByteCountSink implements DataSink {
  int _length = 0;
  int get length => _length;
  @override
  void write(Uint8List chunk) => _length += chunk.length;
}

class _ByteCountSinkWrapper implements DataSink {
  _ByteCountSinkWrapper(this._inner);

  final DataSink _inner;
  int _length = 0;
  int get length => _length;
  @override
  void write(Uint8List chunk) {
    _length += chunk.length;
    final result = _inner.write(chunk);
    if (result is Future<void>) unawaited(result);
  }
}
