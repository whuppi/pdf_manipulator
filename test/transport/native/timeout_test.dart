// Native transport — condvar timeout and slow source handling.
// Tests the isolate + FFI path's behavior with delayed reads.

@TestOn('!browser')
library;

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_source.dart';
import 'package:pdf_manipulator/src/transport/native/native_bridge.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';

void main() {
  group('slow source', () {
    test('succeeds with 100ms delay per read', () async {
      final bridge = NativeBridge();
      try {
        final doc = await bridge.open(
            _SlowSource(minimalPdf, delay: const Duration(milliseconds: 100)));
        expect(doc.pageCount, 1);
      } finally {
        await bridge.dispose();
      }
    });

    test('succeeds with 500ms delay per read', () async {
      final bridge = NativeBridge();
      try {
        final doc = await bridge.open(
            _SlowSource(minimalPdf, delay: const Duration(milliseconds: 500)));
        expect(doc.pageCount, 1);
      } finally {
        await bridge.dispose();
      }
    });
  });

  group('dispose during operation', () {
    test('dispose completes even with hanging source', () async {
      final bridge = NativeBridge();

      // Start an op with a source that never returns
      unawaited(
        bridge.open(_HangingSource(minimalPdf)).then((_) {}, onError: (_) {}),
      );

      // Give it a moment to enter the read path
      await Future<void>.delayed(const Duration(milliseconds: 100));

      // Dispose should complete (not hang forever)
      await bridge.dispose().timeout(
        const Duration(seconds: 10),
        onTimeout: () => fail('dispose hung for >10s with hanging source'),
      );
    });
  });
}

class _SlowSource implements PdfSource {
  final Uint8List _data;
  final Duration delay;
  _SlowSource(this._data, {required this.delay});

  @override
  int get length => _data.length;

  @override
  FutureOr<Uint8List> readAt(int offset, int count) async {
    await Future<void>.delayed(delay);
    final end = (offset + count).clamp(0, _data.length);
    return Uint8List.fromList(_data.sublist(offset, end));
  }
}

class _HangingSource implements PdfSource {
  final Uint8List _data;
  _HangingSource(this._data);

  @override
  int get length => _data.length;

  @override
  FutureOr<Uint8List> readAt(int offset, int count) {
    return Completer<Uint8List>().future; // never completes
  }
}
