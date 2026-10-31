// SharedBridge — contract: every op encoded correctly.
// Mirrors lib/src/bridge/shared_bridge.dart.
//
// Verifies that SharedBridge methods produce the correct binary-encoded
// requests. These are UNIT tests against the codec layer — they don't
// need a running engine.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/pdf_transport.dart';
import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:pdf_manipulator/src/bridge/shared_bridge.dart';
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:test/test.dart';

import '../fixtures/handwritten.dart';
import '../harness/test_source_sink.dart';

void main() {
  group('shared bridge encoding', () {
    test('every EngineOp has a non-empty wire name', () {
      for (final op in EngineOp.values) {
        expect(op.wire, isNotEmpty, reason: '${op.name} wire name');
      }
    });

    test('every EngineOp wire name is unique', () {
      final wires = EngineOp.values.map((o) => o.wire).toSet();
      expect(wires.length, EngineOp.values.length);
    });

    test('encodeRequest produces valid binary', () {
      final bytes = encodeRequest('open', {'password': 'secret'});
      expect(bytes.length, greaterThan(0));
      // First byte is op name length
      expect(bytes[0], 4);
      expect(String.fromCharCodes(bytes.sublist(1, 5)), 'open');
    });

    test('decodeResponse throws PdfEngineError on empty response', () {
      expect(
        () => decodeResponse(Uint8List(0)),
        throwsA(isA<PdfEngineError>()),
      );
    });
  });

  group('shared bridge request shape', () {
    Future<List<Uint8List>> requestsOf(
      Future<void> Function(BridgeDocHandle doc) run,
    ) async {
      final transport = _RecordingTransport();
      final doc = await SharedBridge(transport).open(src(minimalPdf));
      await run(doc);
      return transport.requests;
    }

    test('extract of all pages sends a sink and no page', () async {
      final requests = await requestsOf(
        (doc) => doc.extract(pages: const PdfPages.all()),
      );
      final extract = _fieldKeys(requests.last);
      expect(extract, containsAll(['handleId', 'format', 'hasSink']));
      expect(extract, isNot(contains('page')));
    });

    test('extract of one page sends no sink', () async {
      final requests = await requestsOf(
        (doc) => doc.extract(pages: const PdfPages.single(0)),
      );
      final extract = _fieldKeys(requests.last);
      expect(extract, containsAll(['page', 'format']));
      expect(extract, isNot(contains('hasSink')));
    });
  });
}

/// The field keys of an encoded request: `op_len`, `op`, a u16 count, then
/// `key_len`, `key`, `type`, `value` per field, with the codec's value widths.
Set<String> _fieldKeys(Uint8List req) {
  final data = ByteData.sublistView(req);
  var pos = 1 + req[0];
  final count = data.getUint16(pos, Endian.little);
  pos += 2;
  final keys = <String>{};
  for (var i = 0; i < count; i++) {
    final keyLen = req[pos++];
    keys.add(String.fromCharCodes(req.sublist(pos, pos + keyLen)));
    pos += keyLen;
    final type = req[pos++];
    switch (type) {
      case 0:
        break;
      case 1:
        pos += 4;
      case 2 || 3:
        pos += 8;
      case 4:
        pos += 1;
      case 5 || 6:
        pos += 4 + data.getUint32(pos, Endian.little);
      case 7:
        pos += 4 + 4 * data.getUint32(pos, Endian.little);
      default:
        fail('unexpected wire type $type');
    }
  }
  return keys;
}

/// Records every request; answers `open` with a handle and any other
/// op with an empty ok response.
class _RecordingTransport implements PdfTransport {
  final requests = <Uint8List>[];

  @override
  Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
    Uint8List request, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
    Set<int> keepSources = const {},
    CancelHook? cancel,
  }) async {
    requests.add(request);
    final isOpen =
        String.fromCharCodes(request.sublist(1, 1 + request[0])) == 'open';
    final bytes = isOpen
        ? Uint8List.fromList([
            1, 1, 0, // ok, one field
            8, ...'handleId'.codeUnits, 1, 7, 0, 0, 0, // i32 7
          ])
        : Uint8List.fromList([1, 0, 0]);
    return (bytes: bytes, resourceIds: {for (final i in keepSources) i: 1});
  }

  @override
  Future<void> releaseSource(int resourceId) async {}

  @override
  Future<List<Uint8List>> installPrelude(
    Uint8List request, {
    required Uint8List sourceBytes,
  }) async => const [];

  @override
  PdfIoMode? get ioMode => null;

  @override
  Future<PdfIoMode> ensureInitialized() async => PdfIoMode.values.first;

  @override
  Future<void> dispose() async {}
}
