// MemorySource — random-access reads over an in-memory buffer.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  final bytes = Uint8List.fromList(List.generate(100, (i) => i));

  group('MemorySource', () {
    test('length reflects the buffer', () {
      expect(MemorySource(bytes).length, 100);
      expect(MemorySource(Uint8List(0)).length, 0);
    });

    test('readAt returns the requested window', () {
      expect(MemorySource(bytes).readAt(10, 5), equals([10, 11, 12, 13, 14]));
    });

    test('readAt clamps an over-read to the available bytes', () {
      // Ask for more than remains — get only what's there, no throw.
      expect(MemorySource(bytes).readAt(96, 50), equals([96, 97, 98, 99]));
    });

    test('readAt past EOF returns empty, never throws', () {
      // Regression: offset > length must not RangeError.
      expect(MemorySource(bytes).readAt(100, 10), isEmpty);
      expect(MemorySource(bytes).readAt(200, 10), isEmpty);
    });

    test('readAt returns a view, not a copy', () {
      // The class promises zero-copy reads; the transport relies on it.
      // Mutating the backing buffer must show through the returned window.
      final buf = Uint8List.fromList([0, 1, 2, 3, 4]);
      final view = MemorySource(buf).readAt(1, 3); // [1, 2, 3]
      buf[2] = 99;
      expect(view[1], 99, reason: 'a copy would still read 2 here');
    });
  });
}
