// The O(1)-memory guards guard the whole suite — so they get their
// own proof. Every battery feeds the engine through TestSource and
// TestSink; if their chunk limits silently stopped throwing, every
// "O(1) memory" claim in the suite would go vacuous without a single
// failure. These tests pin that the guards can actually fire.

@TestOn('vm || browser')
library;

import 'dart:typed_data';

import 'package:test/test.dart';

import 'test_source_sink.dart';

void main() {
  group('TestSource guard', () {
    test('serves reads up to the 64KB chunk limit', () {
      final source = TestSource(Uint8List(maxReadChunk * 2));
      expect(source.readAt(0, maxReadChunk).length, maxReadChunk);
    });

    test('throws on a read past the chunk limit', () {
      final source = TestSource(Uint8List(maxReadChunk * 2));
      expect(
        () => source.readAt(0, maxReadChunk + 1),
        throwsStateError,
        reason:
            'a full-file read is exactly the regression this '
            'guard exists to catch',
      );
    });

    test('clamps reads at end of data and returns empty past it', () {
      final source = TestSource(Uint8List.fromList([1, 2, 3]));
      expect(source.readAt(2, 10), [3]);
      expect(source.readAt(3, 10), isEmpty);
    });

    test('returns views over the backing buffer, not copies', () {
      // Real sources (memory, blob) return views; the transport must
      // copy before transfer. The harness must match that shape or
      // it tests a friendlier contract than production gives.
      final data = Uint8List.fromList([1, 2, 3, 4]);
      final view = TestSource(data).readAt(1, 2);
      data[1] = 99;
      expect(
        view[0],
        99,
        reason:
            'a copy here would hide transfer-detach bugs the '
            'real sources would surface',
      );
    });
  });

  group('TestSink guard', () {
    test('accepts writes up to the 256KB chunk limit', () {
      final sink = TestSink();
      sink.write(Uint8List(maxWriteChunk));
      expect(sink.length, maxWriteChunk);
    });

    test('throws on a write past the chunk limit', () {
      final sink = TestSink();
      expect(
        () => sink.write(Uint8List(maxWriteChunk + 1)),
        throwsStateError,
        reason:
            'a full-output dump is exactly the regression this '
            'guard exists to catch',
      );
    });

    test('takeBytes concatenates chunks in write order', () {
      final sink = TestSink();
      sink.write(Uint8List.fromList([1, 2]));
      sink.write(Uint8List.fromList([3]));
      expect(sink.takeBytes(), [1, 2, 3]);
    });
  });
}
