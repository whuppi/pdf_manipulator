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

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';

/// Max bytes per readAt call. Matches the 64KB read buffer in Rust.
const maxReadChunk = 64 * 1024;

/// Max bytes per write call. Matches the 256KB write buffer in Rust.
const maxWriteChunk = 256 * 1024;

class TestSource implements DataSource {
  TestSource(this._data, {this.onRead});
  final Uint8List _data;

  /// Called on every [readAt], before the bytes are returned. Tests
  /// count reads and mark moments in an op through it.
  final void Function(int offset, int count)? onRead;

  @override
  int get length => _data.length;

  @override
  Uint8List readAt(int offset, int count) {
    onRead?.call(offset, count);
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

  @override
  void close() {}
}

class TestSink implements DataSink {
  final _buf = BytesBuilder(copy: false);

  /// How many times the library aborted this sink.
  int aborts = 0;

  @override
  FutureOr<void> close() {}

  @override
  FutureOr<void> abort(Object reason) {
    aborts++;
    _buf.clear();
  }

  @override
  FutureOr<void> write(Uint8List chunk) {
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

/// A [TestSink] that keeps the order of what happens to it, for the abort
/// contract: one abort, one close, no write after the abort. A write that
/// fails is the injected failure of an op.
class CountingSink extends TestSink {
  /// [failOnWrite] is the 1-based write that throws; none when null.
  /// [onFirstWrite] runs as the first write arrives, for a test that cancels
  /// its task from inside the op.
  CountingSink({this.failOnWrite, this.onFirstWrite});

  final int? failOnWrite;
  final void Function()? onFirstWrite;

  /// `write`, `abort` and `close`, in the order they arrived.
  final events = <String>[];
  int writes = 0;
  int closes = 0;
  int writesAfterAbort = 0;

  @override
  FutureOr<void> write(Uint8List chunk) {
    if (aborts > 0) writesAfterAbort++;
    events.add('write');
    if (++writes == 1) onFirstWrite?.call();
    if (writes == failOnWrite) throw StateError('the sink failed');
    return super.write(chunk);
  }

  @override
  FutureOr<void> abort(Object reason) {
    events.add('abort');
    return super.abort(reason);
  }

  @override
  FutureOr<void> close() {
    events.add('close');
    closes++;
  }
}

DataSource src(Uint8List bytes) => TestSource(bytes);
