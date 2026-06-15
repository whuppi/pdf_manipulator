// MemorySink — collects written chunks into one buffer.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  group('MemorySink', () {
    test('takeBytes concatenates chunks in write order', () {
      final sink = MemorySink()
        ..write(Uint8List.fromList([1, 2]))
        ..write(Uint8List.fromList([3]))
        ..write(Uint8List.fromList([4, 5]));
      expect(sink.takeBytes(), equals([1, 2, 3, 4, 5]));
    });

    test('takeBytes drains — a second call returns empty', () {
      final sink = MemorySink()..write(Uint8List.fromList([1, 2, 3]));
      expect(sink.takeBytes(), equals([1, 2, 3]));
      expect(sink.takeBytes(), isEmpty);
    });

    test('an empty sink yields empty bytes', () {
      expect(MemorySink().takeBytes(), isEmpty);
    });
  });
}
