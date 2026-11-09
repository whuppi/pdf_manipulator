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

    test('takeString decodes the chunks as UTF-8 and drains', () {
      final sink = MemorySink()
        ..write(Uint8List.fromList([0xC3]))
        ..write(Uint8List.fromList([0xA9, 0x21]));
      expect(sink.takeString(), 'é!');
      expect(sink.takeString(), isEmpty);
    });

    test('close holds nothing: the bytes are still there', () {
      final sink = MemorySink()..write(Uint8List.fromList([1, 2]));
      sink.close();
      expect(sink.takeBytes(), equals([1, 2]));
    });
  });
}
