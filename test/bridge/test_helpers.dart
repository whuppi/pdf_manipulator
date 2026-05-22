// Test helpers for the new bridge layer.
// Uses the new api/ PdfSource and PdfSink types.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_source.dart';
import 'package:pdf_manipulator/src/api/pdf_sink.dart';

class TestSource implements PdfSource {
  TestSource(this._data);
  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.fromList(_data.sublist(offset, end));
  }
}

class TestSink implements PdfSink {
  final _buf = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _buf.add(chunk);

  Uint8List takeBytes() => _buf.takeBytes();
  int get length => _buf.length;
}

PdfSource src(Uint8List bytes) => TestSource(bytes);
