// Channel buffers — condvar layout, flag bits, capacity constants.
// Mirrors vendor/pdf_oxide/src/host/native/shared_buffer.rs.
//
// Verifies the Dart-side constants match the Rust-side layout.
// If these fail, Dart and Rust disagree on byte offsets → silent corruption.

@TestOn('!browser')
library;

// io-exempt: reads shared_buffer.rs from disk to pin the layout both sides hardcode.
import 'dart:io';

import 'package:pdf_manipulator/src/runtime/native/channel_buffers.dart';
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

  group('single-writer words', () {
    // The offsets are parsed from the Rust source, so the two sides cannot
    // drift: Dart writes the response word, Rust writes the cancel word.
    final rust = File(
      'vendor/pdf_oxide/src/host/native/shared_buffer.rs',
    ).readAsStringSync();

    int rustOffset(String module, String name) {
      final body = RegExp(
        'pub mod $module \\{(.*?)\\n\\}',
        dotAll: true,
      ).firstMatch(rust)!.group(1)!;
      final match = RegExp('pub const $name: usize = (\\d+);').firstMatch(body);
      if (match == null) fail('$module::$name not found in shared_buffer.rs');
      return int.parse(match.group(1)!);
    }

    test('read channel: Dart response word and Rust cancel word', () {
      expect(readResponseOffset, rustOffset('read_channel', 'OFFSET_RESPONSE'));
      expect(readCancelOffset, rustOffset('read_channel', 'OFFSET_CANCEL'));
      expect(
        readRequestSeqOffset,
        rustOffset('read_channel', 'OFFSET_REQUEST_SEQ'),
      );
      expect(
        {readResponseOffset, readCancelOffset, readRequestSeqOffset},
        hasLength(3),
        reason: 'one word per writer',
      );
      expect(readResponseOffset, isNot(readCancelOffset));
      expect(
        (readResponseOffset - readCancelOffset).abs(),
        greaterThanOrEqualTo(4),
      );
    });

    test('write channel: Dart response word and Rust cancel word', () {
      expect(
        writeResponseOffset,
        rustOffset('write_channel', 'OFFSET_RESPONSE'),
      );
      expect(writeCancelOffset, rustOffset('write_channel', 'OFFSET_CANCEL'));
      expect(
        writeRequestSeqOffset,
        rustOffset('write_channel', 'OFFSET_REQUEST_SEQ'),
      );
      expect(
        {writeResponseOffset, writeCancelOffset, writeRequestSeqOffset},
        hasLength(3),
        reason: 'one word per writer',
      );
      expect(writeResponseOffset, isNot(writeCancelOffset));
      expect(
        (writeResponseOffset - writeCancelOffset).abs(),
        greaterThanOrEqualTo(4),
      );
    });

    test('a response echoes its request and touches no other word', () {
      final read = ReadChannelBuffer();
      final write = WriteChannelBuffer();
      addTearDown(() {
        read.free();
        write.free();
      });
      expect(read.cancelled, isFalse);
      expect(read.requestSeq, 0);
      read.respond(7, flagReady);
      read.respond(8, flagReady);
      expect(read.cancelled, isFalse);
      expect(read.requestSeq, 0, reason: 'only Rust writes the request word');
      write.respond(3, flagAck);
      expect(write.cancelled, isFalse);
      expect(write.requestSeq, 0);
    });
  });
}
