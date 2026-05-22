// Test helpers for the new API PdfSource/PdfSink — in-memory implementations.
// Uses the api/ types (not the old core/ types).
// NOT exported from the package. Used only in new bridge tests.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/pdf_sink.dart';

class TestPdfSource implements PdfSource {
  TestPdfSource(this._data);

  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.sublistView(_data, offset, end);
  }
}

class TestPdfSink implements PdfSink {
  final _builder = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _builder.add(chunk);

  Uint8List takeBytes() => _builder.takeBytes();
  int get length => _builder.length;
}

PdfSource sourceOf(Uint8List bytes) => TestPdfSource(bytes);
