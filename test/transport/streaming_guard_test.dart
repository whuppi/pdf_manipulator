// O(1)-memory streaming guard tests.
//
// These verify that the DataSink and DataSource interfaces enforce
// bounded memory. A StreamingGuardSink panics if a single write
// exceeds the chunk limit. A StreamingGuardSource tracks total reads.
//
// When end-to-end integration is wired (Rust compiled + running),
// these guards wrap the real DataSink/DataSource to verify the
// O(1)-memory contract under actual engine load.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:test/test.dart';

class StreamingGuardSink implements DataSink {
  final int maxChunkSize;
  int totalBytes = 0;
  int maxSingleWrite = 0;
  int writeCount = 0;

  StreamingGuardSink({this.maxChunkSize = 256 * 1024});

  @override
  void write(Uint8List chunk) {
    writeCount++;
    totalBytes += chunk.length;
    if (chunk.length > maxSingleWrite) {
      maxSingleWrite = chunk.length;
    }
    if (chunk.length > maxChunkSize) {
      throw StateError(
        'O(1)-memory violation: single write of ${chunk.length} bytes '
        'exceeds max chunk size of $maxChunkSize bytes',
      );
    }
  }
}

class StreamingGuardSource implements DataSource {
  final Uint8List _data;
  int totalBytesRead = 0;
  int readCount = 0;
  int maxSingleRead = 0;

  StreamingGuardSource(this._data);

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    readCount++;
    final actual = count.clamp(0, _data.length - offset);
    totalBytesRead += actual;
    if (actual > maxSingleRead) {
      maxSingleRead = actual;
    }
    return Uint8List.sublistView(_data, offset, offset + actual);
  }
}

void main() {
  group('StreamingGuardSink', () {
    test('accepts chunks within limit', () {
      final sink = StreamingGuardSink(maxChunkSize: 1024);
      sink.write(Uint8List(512));
      sink.write(Uint8List(1024));
      expect(sink.writeCount, 2);
      expect(sink.totalBytes, 1536);
      expect(sink.maxSingleWrite, 1024);
    });

    test('rejects chunks exceeding limit', () {
      final sink = StreamingGuardSink(maxChunkSize: 1024);
      expect(
        () => sink.write(Uint8List(2048)),
        throwsStateError,
      );
    });

    test('default limit is 256KB', () {
      final sink = StreamingGuardSink();
      sink.write(Uint8List(256 * 1024));
      expect(sink.maxSingleWrite, 256 * 1024);
    });
  });

  group('StreamingGuardSource', () {
    test('tracks read statistics', () {
      final data = Uint8List(100000);
      final source = StreamingGuardSource(data);

      source.readAt(0, 4096);
      source.readAt(4096, 4096);
      source.readAt(8192, 4096);

      expect(source.readCount, 3);
      expect(source.totalBytesRead, 12288);
      expect(source.maxSingleRead, 4096);
    });

    test('clamps reads to data length', () {
      final data = Uint8List(100);
      final source = StreamingGuardSource(data);

      final result = source.readAt(90, 50);
      expect(result.length, 10);
    });

    test('reports length correctly', () {
      final data = Uint8List(42);
      final source = StreamingGuardSource(data);
      expect(source.length, 42);
    });
  });

  group('O(1)-memory contract', () {
    test('sink never receives more than 256KB per write', () {
      // This test structure is ready for end-to-end: wrap a real
      // DataSink in StreamingGuardSink, run an editor save through
      // the full pipeline, verify maxSingleWrite <= 256KB.
      final sink = StreamingGuardSink(maxChunkSize: 256 * 1024);

      // Simulate chunked writes (what CallbackWriter produces)
      for (var i = 0; i < 100; i++) {
        sink.write(Uint8List(64 * 1024)); // 64KB chunks
      }

      expect(sink.totalBytes, 100 * 64 * 1024);
      expect(sink.maxSingleWrite, 64 * 1024);
      expect(sink.maxSingleWrite, lessThanOrEqualTo(256 * 1024));
    });

    test('source reads are bounded per call', () {
      // Simulate what CallbackReader does: 64KB max per read
      final data = Uint8List(1024 * 1024); // 1MB source
      final source = StreamingGuardSource(data);

      var offset = 0;
      while (offset < source.length) {
        final chunk = source.readAt(offset, 64 * 1024);
        offset += chunk.length;
      }

      expect(source.maxSingleRead, 64 * 1024);
      expect(source.totalBytesRead, 1024 * 1024);
    });
  });
}
