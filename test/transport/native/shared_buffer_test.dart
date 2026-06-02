// SharedBuffer — condvar layout, flag bits, capacity constants.
// Mirrors lib/src/transport/native/shared_buffer.dart.
//
// Verifies the Dart-side constants match the Rust-side layout.
// If these fail, Dart and Rust disagree on byte offsets → silent corruption.

@TestOn('!browser')
library;

import 'package:pdf_manipulator/src/transport/native/shared_buffer.dart';
import 'package:test/test.dart';

void main() {
  group('shared buffer layout', () {
    test('flag bits are distinct powers of two', () {
      expect(flagReady, 1);
      expect(flagError, 2);
      expect(flagCancelled, 4);
      expect(flagAck, 8);
      // No overlaps
      expect(flagReady & flagError, 0);
      expect(flagReady & flagCancelled, 0);
      expect(flagReady & flagAck, 0);
      expect(flagError & flagCancelled, 0);
    });

    test('read buffer capacity is 64KB', () {
      expect(readBufferSize, greaterThan(64 * 1024));
    });

    test('write buffer capacity is 256KB', () {
      expect(writeBufferSize, greaterThan(256 * 1024));
    });

    test('read buffer size includes header + data', () {
      // Header is at least 160 bytes (offsets up to _readData = 160)
      // Data is 64KB
      expect(readBufferSize, 160 + 64 * 1024);
    });

    test('write buffer size includes header + data', () {
      // Header is at least 144 bytes (offsets up to _writeData = 144)
      // Data is 256KB
      expect(writeBufferSize, 144 + 256 * 1024);
    });
  });
}
