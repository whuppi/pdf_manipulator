// CHARTER — this battery alone proves the lane protocol's reset verb on every
// platform: a reset gives the executor a fresh engine state (no old handle,
// ids counting from the start, a new open works), a reset is refused while a
// job is outstanding and leaves that job unharmed, and an idle executor may
// reset again and again. The lease id and the single-exec wall are
// platform-shaped, so they are proved in test/ops/platform/. It drives the
// platform's real lane directly, with no Router.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/slow_source.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

Uint8List _open(DataSource source) => bin.encodeRequest('open', {
  'sourceLength': source.length,
  'password': null,
});

Uint8List _info(int handle) =>
    bin.encodeRequest('pageInfo', {'handleId': handle, 'pageIndices': <int>[]});

bool _ok(Uint8List bytes) => bytes.isNotEmpty && bytes[0] == 1;

String _failure(Uint8List bytes) {
  try {
    bin.decodeResponse(bytes);
    return '';
  } catch (e) {
    return e.toString();
  }
}

/// Runs one job on [lane] and returns its response bytes. [group] is the id
/// of the job that created the handle the request names, as the Router
/// passes it.
Future<Uint8List> _run(
  Lease lane,
  int jobId,
  Uint8List request, {
  List<DataSource> sources = const [],
  Set<int> keep = const {},
  int? group,
}) async {
  final result = await lane.submit(
    Job(
      jobId: jobId,
      request: request,
      sources: sources,
      sinks: const [],
      keepSources: keep,
      holdGroupId: group,
    ),
  );
  return result.bytes;
}

void registerLaneProtocolTests({PdfConfig? Function()? config}) {
  group('lane protocol: reset', () {
    late Executor executor;
    late Lease lane;

    setUp(() async {
      executor = await createBooter(config: config?.call()).boot();
      lane = executor.beginLane();
    });
    tearDown(() => executor.terminate());

    test('a reset gives a fresh engine state in the same executor', () async {
      final first = src(minimalPdf);
      final opened = await _run(
        lane,
        1,
        _open(first),
        sources: [first],
        keep: {0},
      );
      final old = peekResponseHandleId(opened)!;
      expect(_ok(await _run(lane, 2, _info(old), group: 1)), isTrue);

      await lane.reset();

      final gone = await _run(lane, 3, _info(old), group: 1);
      expect(_ok(gone), isFalse, reason: 'the reset left the old document');
      expect(_failure(gone), contains('document not found'));

      // A fresh state counts handles from the start again: the same id names
      // the new document, which is how a replaced state shows itself.
      final second = src(minimalPdf);
      final reopened = await _run(
        lane,
        4,
        _open(second),
        sources: [second],
        keep: {0},
      );
      expect(_ok(reopened), isTrue, reason: 'a new open works after a reset');
      expect(peekResponseHandleId(reopened), old);
      expect(_ok(await _run(lane, 5, _info(old), group: 4)), isTrue);
    }, timeout: t(10));

    test('an idle executor resets again and again', () async {
      await lane.reset(); // a fresh executor resets too
      final source = src(minimalPdf);
      await _run(lane, 1, _open(source), sources: [source], keep: {0});
      await lane.reset();
      await lane.reset();
      await lane.reset();
      final again = src(minimalPdf);
      final opened = await _run(lane, 2, _open(again), sources: [again]);
      expect(_ok(opened), isTrue);
    }, timeout: t(10));

    test(
      'a reset while a job runs is refused and the job is unharmed',
      () async {
        final slow = SlowSource(minimalPdf);
        final running = _run(lane, 1, _open(slow), sources: [slow], keep: {0});
        await slow.firstRead;

        await expectLater(
          lane.reset(),
          throwsA(
            isA<LeaseProtocolError>().having(
              (e) => e.message,
              'message',
              executorBusyMessage,
            ),
          ),
        );

        // A job queued behind the running one counts as outstanding too.
        final queued = _run(lane, 2, bin.encodeRequest('builderCreate', {}));
        await expectLater(lane.reset(), throwsA(isA<LeaseProtocolError>()));

        slow.release();
        final opened = await running;
        expect(
          _ok(opened),
          isTrue,
          reason: 'the refused reset disturbed the job',
        );
        expect(_ok(await queued), isTrue);

        // Drained, the lane resets, and the refused reset left no trace.
        await lane.reset();
        final after = src(minimalPdf);
        expect(
          _ok(await _run(lane, 3, _open(after), sources: [after])),
          isTrue,
        );
      },
      timeout: t(10),
    );
  });
}
