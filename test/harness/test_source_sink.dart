// O(1) memory I/O guards for test infrastructure.
//
// These guards enforce bounded chunk sizes at the Dart ↔ Rust transport
// boundary. Any readAt > 64KB or write > 256KB throws immediately.
//
// WHAT THIS CATCHES:
//   - Dart-side full-file reads (readAt(0, source.length))
//   - Rust-side full-output dumps (write_all of entire PDF)
//   - Any future regression where an op bypasses the streaming path
//
// WHAT THIS DOES NOT CATCH:
//   - Rust-side internal buffering (reading 64KB chunks but
//     accumulating them into a Vec<u8> inside the engine). The
//     transport looks O(1) but Rust memory is O(n). A tracking-
//     allocator check inside the Rust harness would close this gap —
//     tracked in CAPABILITY_ROADMAP.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';

/// Max bytes per readAt call. Matches the 64KB read buffer in Rust.
const maxReadChunk = 64 * 1024;

/// Max bytes per write call. Matches the 256KB write buffer in Rust.
const maxWriteChunk = 256 * 1024;

class TestSource implements DataSource {
  TestSource(this._data);
  final Uint8List _data;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    if (count > maxReadChunk) {
      throw StateError(
        'O(1) SOURCE violation: readAt requested $count bytes '
        '(max $maxReadChunk). Source length: $length, offset: $offset.',
      );
    }
    if (offset >= _data.length) return Uint8List(0);
    final end = (offset + count).clamp(0, _data.length);
    // Return a VIEW, not a copy. A real DataSource (MemorySource, BlobSource)
    // returns views. The transport must handle views correctly — copying
    // before postMessage transfer to avoid detaching the backing buffer.
    return Uint8List.sublistView(_data, offset, end);
  }
}

class TestSink implements DataSink {
  final _buf = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) {
    if (chunk.length > maxWriteChunk) {
      throw StateError(
        'O(1) SINK violation: write received ${chunk.length} bytes '
        '(max $maxWriteChunk).',
      );
    }
    _buf.add(chunk);
  }

  Uint8List takeBytes() => _buf.takeBytes();
  int get length => _buf.length;
}

DataSource src(Uint8List bytes) => TestSource(bytes);
