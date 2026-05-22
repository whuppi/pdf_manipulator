// Test helpers for PdfSource/PdfSink — in-memory implementations.
// NOT exported from the package. Used only in tests.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// In-memory PdfSource for testing. Wraps a Uint8List.
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

/// In-memory PdfSink for testing. Collects output bytes.
class TestPdfSink implements PdfSink {
  final _builder = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _builder.add(chunk);

  Uint8List takeBytes() => _builder.takeBytes();
  int get length => _builder.length;
}

/// Convenience — wrap Uint8List as PdfSource for tests.
PdfSource sourceOf(Uint8List bytes) => TestPdfSource(bytes);
