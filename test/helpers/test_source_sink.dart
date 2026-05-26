// Test helpers for the new bridge layer.
// Uses the new api/ DataSource and DataSink types.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';

class TestSource implements DataSource {
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

class TestSink implements DataSink {
  final _buf = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _buf.add(chunk);

  Uint8List takeBytes() => _buf.takeBytes();
  int get length => _buf.length;
}

DataSource src(Uint8List bytes) => TestSource(bytes);
