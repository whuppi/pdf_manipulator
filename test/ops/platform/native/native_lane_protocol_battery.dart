// Native-only: the lease id and the single-job rule of the lane protocol.
//
// A lease is one lane's use of one executor, numbered by the executor. Native job ids come from a
// Router, so two leases on one executor count them separately and a cancel
// keyed by job id alone could hit the wrong job; the executor therefore
// drops every submit, cancel, release and reset that carries a lease which
// is not its current one. Read and write answers need no lease: each is
// numbered by its channel's request word and goes to a buffer one job owns
// (native_stale_answer_battery proves the late-answer case).
//
// The mailbox may buffer a second job while one runs; the thread starts
// nothing until the job in hand has posted, which is the native form of the
// worker's single-exec wall.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/native/bindings.dart' as bindings;
import 'package:pdf_manipulator/src/runtime/native/lease.dart'
    show NativeExecutor, NativeLease;
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:test/test.dart';

import '../../../fixtures/handwritten.dart';
import '../../../harness/slow_source.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

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

Job _job(
  int jobId,
  Uint8List request, {
  List<DataSource> sources = const [],
  Set<int> keep = const {},
}) => Job(
  jobId: jobId,
  request: request,
  sources: sources,
  sinks: const [],
  keepSources: keep,
);

void registerNativeLaneProtocolTests() {
  group('native lane protocol', () {
    late NativeExecutor executor;
    late NativeLease lane;

    setUp(() async {
      executor = await createBooter().boot() as NativeExecutor;
      lane = executor.beginLane() as NativeLease;
    });
    tearDown(() => executor.terminate());

    test('a drain answers a parked job at once, settles when the thread has '
        'posted, and the executor serves the next lease', () async {
      final slow = SlowSource(minimalPdf);
      final running = lane.submit(_job(1, _open(slow), sources: [slow]));
      await slow.firstRead;

      final drained = lane.drain();
      expect((await running).bytes, buildCancelledResponse());
      expect(await drained, isTrue, reason: 'the engine is intact');
      slow.release();

      await lane.reset();
      final next = executor.beginLane();
      final source = src(minimalPdf);
      final opened = await next.submit(
        _job(1, _open(source), sources: [source]),
      );
      expect(_ok(opened.bytes), isTrue, reason: 'the next lease serves');
    }, timeout: t(10));

    test('a stale cancel never reaches the running job', () async {
      final slow = SlowSource(minimalPdf);
      final running = lane.submit(_job(1, _open(slow), sources: [slow]));
      await slow.firstRead;

      // The stale lease names the running job's id, as a second lease's
      // counter would. The job is parked on its read, where a cancel that
      // reached it would end it.
      expect(
        bindings.executorJobCancel(lane.executorKey, lane.lease + 7, 1),
        0,
      );
      expect(
        bindings.executorLeaseBegin(lane.executorKey),
        0,
        reason: 'a lease does not change hands under a running job',
      );

      slow.release();
      final result = await running;
      expect(
        _ok(result.bytes),
        isTrue,
        reason: 'the stale cancel ended the current lease\'s job',
      );
    }, timeout: t(10));

    test(
      'a replaced lease is refused on every verb and touches nothing',
      () async {
        final first = src(minimalPdf);
        final opened = await lane.submit(
          _job(1, _open(first), sources: [first], keep: {0}),
        );
        expect(_ok(opened.bytes), isTrue);
        await lane.reset();
        final next = bindings.executorLeaseBegin(lane.executorKey);
        expect(
          next,
          greaterThan(lane.lease),
          reason: 'the reset lane takes the next lease',
        );

        // Submit: refused with the stale-lease error before any source is read.
        var reads = 0;
        final probe = TestSource(minimalPdf, onRead: (_, _) => reads++);
        final stale = await lane.submit(
          _job(2, _open(probe), sources: [probe], keep: {0}),
        );
        expect(_ok(stale.bytes), isFalse);
        expect(_failure(stale.bytes), contains(staleLeaseMessage));
        expect(reads, 0, reason: 'a stale submit reached its source');
        for (final token in stale.holds.values) {
          lane.releaseHold(token); // the Router's cleanup of a failed job
        }

        // Reset: refused, typed.
        await expectLater(
          lane.reset(),
          throwsA(
            isA<LeaseProtocolError>().having(
              (e) => e.message,
              'message',
              staleLeaseMessage,
            ),
          ),
        );

        // Cancel: ignored.
        expect(bindings.executorJobCancel(lane.executorKey, lane.lease, 2), 0);
      },
      timeout: t(10),
    );

    test('a reset drops the held reader and every document', () async {
      final first = src(minimalPdf);
      final opened = await lane.submit(
        _job(1, _open(first), sources: [first], keep: {0}),
      );
      final old = peekResponseHandleId(opened.bytes)!;
      expect(opened.holds, hasLength(1));
      expect(
        bindings.executorLeaseBegin(lane.executorKey),
        0,
        reason: 'the held reader is registered with the executor',
      );

      await lane.reset();

      final gone = await lane.submit(_job(2, _info(old)));
      expect(_failure(gone.bytes), contains('document not found'));
      lane.releaseHold(opened.holds.values.single); // already freed: a no-op

      final second = src(minimalPdf);
      final reopened = await lane.submit(
        _job(3, _open(second), sources: [second], keep: {0}),
      );
      expect(
        _ok(reopened.bytes),
        isTrue,
        reason: 'a new open works after a reset',
      );
      expect(peekResponseHandleId(reopened.bytes), old);

      // Reset again: the registry is empty, so the lease may change hands.
      await lane.reset();
      await lane.reset();
      expect(
        bindings.executorLeaseBegin(lane.executorKey),
        greaterThan(lane.lease),
      );
    }, timeout: t(10));

    test(
      'a second job waits in the mailbox until the first has posted',
      () async {
        final slow = SlowSource(minimalPdf);
        final first = lane.submit(_job(1, _open(slow), sources: [slow]));
        await slow.firstRead;

        var secondReads = 0;
        final source = TestSource(minimalPdf, onRead: (_, _) => secondReads++);
        final second = lane.submit(_job(2, _open(source), sources: [source]));

        await Future<void>.delayed(const Duration(milliseconds: 200));
        expect(secondReads, 0, reason: 'the executor ran two jobs at once');

        slow.release();
        expect(_ok((await first).bytes), isTrue);
        expect(_ok((await second).bytes), isTrue);
        expect(secondReads, greaterThan(0));
      },
      timeout: t(10),
    );
  });
}
