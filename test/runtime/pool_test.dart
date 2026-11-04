// ExecutorPool and PooledLane — unit-tested against fake executors.
//
// Lease exclusivity, the drop of a stale lease, drain-then-reset, dead
// executors never reused, smallest heap first, prewarm once, waiters served
// in arrival order, a budget that frees the moment an executor is
// terminated, and the lazy lease of a lane. No engine, no platform.

@TestOn('vm')
library;

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:test/test.dart';

import '../harness/fake_executors.dart';

Future<void> flush() => Future<void>.delayed(Duration.zero);

LaneJob job(int id) => LaneJob(
  jobId: id,
  request: Uint8List(0),
  sources: const [],
  sinks: const [],
  keepSources: const {},
);

void main() {
  late FakeLaneHost host;

  setUp(() => host = FakeLaneHost());

  ExecutorPool pool({
    int maxExecutors = 4,
    int maxIdle = 4,
    int keepBytes = 1 << 30,
  }) => ExecutorPool(
    host: host,
    maxExecutors: maxExecutors,
    maxIdle: maxIdle,
    keepBytes: keepBytes,
  );

  /// A lease for the next request, with the pool given time to boot.
  Future<ExecutorLease> take(ExecutorPool pool) async {
    final request = pool.acquire();
    await flush();
    return request.lease;
  }

  group('leases', () {
    test('one lane per executor: a second request boots another', () async {
      final p = pool();
      final a = await take(p);
      final b = await take(p);
      expect(host.executors, hasLength(2));
      expect(identical(a.lane, b.lane), isFalse);
      expect(a.state, ExecutorState.leased);
      expect(b.state, ExecutorState.leased);
      expect(p.idleCount, 0);
    });

    test(
      'an executor is leased again only after its lease is released',
      () async {
        final p = pool();
        final a = await take(p);
        final b = p.acquire();
        await flush();
        expect(
          host.executors,
          hasLength(2),
          reason: 'the leased one is not free',
        );

        await p.release(a);
        final c = await take(p);
        expect(host.executors, hasLength(2));
        expect(identical(c.lane, a.lane), isTrue, reason: 'executor 0 again');
        await b.lease;
      },
    );

    test('a stale lease is dropped after release and re-lease', () async {
      final p = pool();
      final first = await take(p);
      final executor = host.executors.single;
      final stale = (first.lane as FakeLane).lease;
      expect(executor.accepts(stale), isTrue);

      await p.release(first);
      final second = await take(p);
      final current = (second.lane as FakeLane).lease;

      expect(current, greaterThan(stale));
      expect(executor.accepts(stale), isFalse, reason: 'its tenure ended');
      expect(executor.accepts(current), isTrue);
    });

    test('release is idempotent', () async {
      final p = pool();
      final a = await take(p);
      await p.release(a);
      await p.release(a);
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
      expect(p.idleCount, 1);
    });
  });

  group('returning an executor', () {
    test(
      'a busy executor drains first and resets only when its job posted',
      () async {
        final p = pool();
        final a = await take(p);
        final fake = a.lane as FakeLane;
        fake.drainGate = Completer<bool>();

        final released = p.release(a);
        await flush();
        expect(a.state, ExecutorState.draining);
        expect(fake.resets, 0, reason: 'the job has not posted');
        expect(p.idleCount, 0);

        fake.drainGate!.complete(true);
        await released;
        expect(fake.executor.events, ['lease', 'drain', 'reset']);
        expect(a.state, ExecutorState.idle);
        expect(p.idleCount, 1);
      },
    );

    test('a lane that ran nothing is not reset', () async {
      final p = pool();
      final a = await take(p);
      await p.release(a, untouched: true);
      expect(host.executors.single.events, ['lease', 'drain']);
      expect(p.idleCount, 1);
    });

    test('a dead engine is terminated and never reused', () async {
      final p = pool();
      final a = await take(p);
      (a.lane as FakeLane).engineDead = true;
      await p.release(a);

      expect(host.executors.single.terminated, isTrue);
      expect(a.state, ExecutorState.dead);
      expect(p.liveCount, 0);

      final next = await take(p);
      expect(host.executors, hasLength(2));
      expect(identical(next.lane, host.executors[0].lane), isFalse);
    });

    test('a failed reset ends the executor', () async {
      final p = pool();
      final a = await take(p);
      host.executors.single.failReset = true;
      await p.release(a);
      expect(host.executors.single.terminated, isTrue);
      expect(p.liveCount, 0);
    });

    test('an idle executor that refuses its lease is dropped, the next is '
        'tried', () async {
      final p = pool();
      final a = await take(p);
      final b = await take(p);
      await p.release(a);
      await p.release(b);
      host.executors[0].failLease = true;
      host.executors[1].heap = 5;

      final c = await take(p);
      expect(host.executors[0].terminated, isTrue);
      expect(identical(c.lane, host.executors[1].lane), isTrue);
      expect(host.executors, hasLength(2), reason: 'no new boot was needed');
    });

    test('an executor at the keep line is terminated, not pooled', () async {
      final p = pool(keepBytes: 100);
      final a = await take(p);
      host.executors.single.heap = 100;
      await p.release(a);
      expect(host.executors.single.terminated, isTrue);
      expect(p.liveCount, 0);

      final b = await take(p);
      expect(host.executors, hasLength(2), reason: 'a replacement booted');
      host.executors.last.heap = 99;
      await p.release(b);
      expect(host.executors.last.terminated, isFalse);
    });

    test('idle executors beyond the shelf are terminated', () async {
      final p = pool(maxIdle: 1);
      final a = await take(p);
      final b = await take(p);
      await p.release(a);
      await p.release(b);
      expect(p.idleCount, 1);
      expect(host.executors.where((e) => e.terminated), hasLength(1));
      expect(p.liveCount, 1);
    });
  });

  group('choosing an executor', () {
    test('the smallest heap goes first', () async {
      final p = pool();
      final a = await take(p);
      final b = await take(p);
      final c = await take(p);
      host.executors[0].heap = 30;
      host.executors[1].heap = 10;
      host.executors[2].heap = 20;
      await p.release(a);
      await p.release(b);
      await p.release(c);

      final first = await take(p);
      final second = await take(p);
      final third = await take(p);
      expect(identical(first.lane, host.executors[1].lane), isTrue);
      expect(identical(second.lane, host.executors[2].lane), isTrue);
      expect(identical(third.lane, host.executors[0].lane), isTrue);
    });

    test('a request waits for an executor that is being returned, never '
        'boots another', () async {
      final p = pool();
      final a = await take(p);
      final fake = a.lane as FakeLane;
      fake.drainGate = Completer<bool>();
      final returned = p.release(a);
      final request = p.acquire();
      await flush();
      expect(host.bootAttempts, 1, reason: 'the draining one covers it');
      expect(p.waitingCount, 1);

      fake.drainGate!.complete(true);
      await returned;
      final lease = await request.lease;
      expect(identical(lease.lane, a.lane), isTrue);
      expect(host.bootAttempts, 1);
    });

    test(
      'a request boots after all, when the executor it waited for dies',
      () async {
        final p = pool();
        final a = await take(p);
        final fake = a.lane as FakeLane;
        fake.drainGate = Completer<bool>();
        final returned = p.release(a);
        final request = p.acquire();
        await flush();

        fake.drainGate!.complete(false);
        await returned;
        await request.lease;
        expect(host.bootAttempts, 2);
        expect(host.executors.first.terminated, isTrue);
      },
    );

    test('an idle executor is taken before a boot', () async {
      final p = pool();
      final a = await take(p);
      await p.release(a);
      await take(p);
      expect(host.executors, hasLength(1));
      expect(p.bootCount, 1);
    });
  });

  group('prewarm', () {
    test('boots one executor, once', () async {
      final p = pool();
      p.prewarm();
      p.prewarm();
      await flush();
      expect(host.executors, hasLength(1));
      expect(p.idleCount, 1);

      final a = await take(p);
      expect(host.executors, hasLength(1), reason: 'the first lease is warm');
      await p.release(a);
      p.prewarm();
      await flush();
      expect(host.executors, hasLength(1), reason: 'paid once per pool');
    });

    test('a request that arrives while it boots waits for it', () async {
      final p = pool();
      host.bootGate = Completer<void>();
      p.prewarm();
      final request = p.acquire();
      await flush();
      expect(host.bootAttempts, 1, reason: 'one boot covers the one request');
      host.bootGate!.complete();
      await request.lease;
      expect(host.executors, hasLength(1));
    });

    test(
      'a failed prewarm is silent and the next request boots again',
      () async {
        final p = pool();
        host.bootError = StateError('no wasm');
        p.prewarm();
        await flush();
        expect(p.liveCount, 0);

        host.bootError = null;
        final a = await take(p);
        expect(a.state, ExecutorState.leased);
      },
    );

    test('the shared pool of a host is created once and prewarmed', () async {
      final shared = FakeLaneHost(key: 'shared-pool-test');
      final first = ExecutorPool.of(shared);
      final second = ExecutorPool.of(FakeLaneHost(key: 'shared-pool-test'));
      await flush();
      expect(identical(first, second), isTrue);
      expect(shared.executors, hasLength(1));
      first.dispose();
    });
  });

  group('waiting for a slot', () {
    test('waiters are served in arrival order', () async {
      final p = pool(maxExecutors: 1);
      final a = await take(p);
      final order = <String>[];
      final b = p.acquire();
      final c = p.acquire();
      final d = p.acquire();
      unawaited(b.lease.then((_) => order.add('b')));
      unawaited(c.lease.then((_) => order.add('c')));
      unawaited(d.lease.then((_) => order.add('d')));
      await flush();
      expect(order, isEmpty);
      expect(p.waitingCount, 3);

      await p.release(a);
      await flush();
      expect(order, ['b']);
      await p.release(await b.lease);
      await flush();
      expect(order, ['b', 'c']);
      await p.release(await c.lease);
      await flush();
      expect(order, ['b', 'c', 'd']);
    });

    test('a request that arrives later never overtakes a waiter', () async {
      final p = pool(maxExecutors: 1);
      final a = await take(p);
      final early = p.acquire();
      await p.release(a);
      final late = p.acquire();
      await flush();

      final earlyLease = await early.lease;
      expect(earlyLease.state, ExecutorState.leased);
      expect(p.waitingCount, 1, reason: 'the later request still waits');
      await p.release(earlyLease);
      await late.lease;
    });

    test('the budget frees the moment an executor is terminated', () async {
      final p = pool(maxExecutors: 1);
      final a = await take(p);
      final waiting = p.acquire();
      await flush();
      expect(host.executors, hasLength(1));
      expect(p.liveCount, 1);

      (a.lane as FakeLane).engineDead = true;
      await p.release(a);
      expect(p.liveCount, 1, reason: 'the dead one left, a new one booted');
      await waiting.lease;
      expect(host.executors, hasLength(2));
      expect(host.executors[0].terminated, isTrue);
    });

    test('a waiter that gives up leaves the queue and takes nothing', () async {
      final p = pool(maxExecutors: 1);
      final a = await take(p);
      final gone = p.acquire();
      final stays = p.acquire();
      await flush();
      gone.cancel();
      expect(p.waitingCount, 1);

      await p.release(a);
      final lease = await stays.lease;
      expect(lease.state, ExecutorState.leased);
    });

    test('the budget is never exceeded while boots are in flight', () async {
      final p = pool(maxExecutors: 2);
      host.bootGate = Completer<void>();
      final requests = [for (var i = 0; i < 5; i++) p.acquire()];
      await flush();
      expect(host.bootAttempts, 2);
      expect(p.liveCount, 2);
      host.bootGate!.complete();
      await flush();
      expect(p.waitingCount, 3);
      expect(requests, hasLength(5));
    });

    test('a failed boot fails the oldest waiter, frees its slot, and boots '
        'again for the rest', () async {
      final p = pool(maxExecutors: 1);
      host.bootFailures = 1;
      final first = p.acquire();
      final second = p.acquire();
      Object? firstError;
      unawaited(
        first.lease.then<void>((_) {}, onError: (Object e) => firstError = e),
      );
      await flush();
      expect(firstError, isA<StateError>());
      expect(host.bootAttempts, 2, reason: 'the failure freed the slot');

      final lease = await second.lease;
      expect(lease.state, ExecutorState.leased);
      expect(p.liveCount, 1);
    });
  });

  group('dispose', () {
    test('terminates every executor and refuses later requests', () async {
      final p = pool();
      final a = await take(p);
      final b = await take(p);
      await p.release(b);
      p.dispose();

      expect(host.executors.every((e) => e.terminated), isTrue);
      expect(a.state, ExecutorState.dead);
      expect(p.liveCount, 0);
      await expectLater(p.acquire().lease, throwsStateError);
    });
  });

  group('a lane over the pool', () {
    PooledLane laneOver(ExecutorPool p, {List<LaneJob> Function()? prelude}) =>
        PooledLane(p, prelude: prelude ?? () => const []);

    test('leases at its first job, not before', () async {
      final p = pool();
      final lane = laneOver(p);
      await flush();
      expect(host.executors, isEmpty);
      expect(lane.leased, isFalse);

      final result = lane.submit(job(1));
      await flush();
      expect(lane.leased, isTrue);
      expect(host.lanes.single.submitted.single.jobId, 1);
      host.lanes.single.completeAllPending();
      await result;
    });

    test('jobs wait for the lease in submission order', () async {
      final p = pool();
      host.bootGate = Completer<void>();
      final lane = laneOver(p);
      final results = [lane.submit(job(1)), lane.submit(job(2))];
      await flush();
      expect(host.executors, isEmpty);

      host.bootGate!.complete();
      await flush();
      expect(host.lanes.single.submitted.map((j) => j.jobId), [1, 2]);
      host.lanes.single.completeAllPending();
      await Future.wait(results);
    });

    test(
      'a cancelled waiting job completes at once and asks for nothing',
      () async {
        final p = pool();
        host.bootGate = Completer<void>();
        final lane = laneOver(p);
        final result = lane.submit(job(1));
        lane.cancelJob(1);
        expect((await result).bytes, buildCancelledResponse());

        host.bootGate!.complete();
        await flush();
        expect(lane.leased, isFalse);
        expect(p.idleCount, 1, reason: 'the executor booted and went idle');
        expect(host.lanes.single.submitted, isEmpty);
      },
    );

    test('release hands the executor back reset, and the next job leases '
        'again', () async {
      final p = pool();
      final lane = laneOver(p);
      final first = lane.submit(job(1));
      await flush();
      host.lanes.single.completeAllPending();
      await first;

      lane.release();
      await flush();
      expect(lane.leased, isFalse);
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);

      final second = lane.submit(job(2));
      await flush();
      expect(host.executors, hasLength(1), reason: 'the same executor');
      expect(host.executors.single.events.last, 'lease');
      host.lanes.single.completeAllPending();
      await second;
    });

    test(
      'a job submitted while the executor is returned waits for the reset',
      () async {
        final p = pool();
        final lane = laneOver(p);
        final first = lane.submit(job(1));
        await flush();
        host.lanes.single.completeAllPending();
        await first;

        host.lanes.single.drainGate = Completer<bool>();
        lane.release();
        final second = lane.submit(job(2));
        await flush();
        expect(host.lanes.single.submitted.map((j) => j.jobId), [1]);

        host.lanes.single.drainGate!.complete(true);
        await flush();
        host.lanes.single.drainGate = null;
        expect(host.lanes.single.submitted.map((j) => j.jobId), [1, 2]);
        host.lanes.single.completeAllPending();
        await second;
      },
    );

    test('a lease that never ran a job is not reset', () async {
      final p = pool();
      final lane = laneOver(p);
      final result = lane.submit(job(1));
      await flush();
      lane.cancelJob(1);
      // The job reached the lane, so it counts as ran: reset follows.
      host.lanes.single.completeAllPending();
      await result;
      lane.release();
      await flush();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);

      final idleLane = laneOver(p);
      idleLane.release();
      await flush();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
    });

    test('the prelude runs first under every new lease', () async {
      final p = pool();
      var built = 0;
      final lane = laneOver(p, prelude: () => [job(100 + built++)]);
      final first = lane.submit(job(1));
      await flush();
      expect(host.lanes.single.submitted.map((j) => j.jobId), [100, 1]);
      host.lanes.single.completeAllPending();
      await first;

      lane.release();
      await flush();
      final second = lane.submit(job(2));
      await flush();
      expect(host.lanes.single.submitted.map((j) => j.jobId), [100, 1, 101, 2]);
      host.lanes.single.completeAllPending();
      await second;
    });

    test('end cancels waiting and running jobs at once and returns the '
        'executor', () async {
      final p = pool();
      final lane = laneOver(p);
      final running = lane.submit(job(1));
      await flush();

      lane.end();
      expect((await running).bytes, buildCancelledResponse());
      await flush();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
      expect(p.idleCount, 1);

      final after = await lane.submit(job(2));
      expect(after.bytes, buildCancelledResponse());
      expect(host.executors, hasLength(1));
    });

    test('end during a boot returns the executor that arrives', () async {
      final p = pool();
      host.bootGate = Completer<void>();
      final lane = laneOver(p);
      final waiting = lane.submit(job(1));
      lane.end();
      expect((await waiting).bytes, buildCancelledResponse());

      host.bootGate!.complete();
      await flush();
      expect(p.idleCount, 1);
      expect(host.lanes.single.submitted, isEmpty);
    });

    test(
      'a failed boot fails the waiting jobs and the next job tries again',
      () async {
        final p = pool();
        host.bootError = StateError('wasm missing');
        final lane = laneOver(p);
        final failed = await lane.submit(job(1));
        expect(failed.bytes[0], isNot(1));
        expect(p.liveCount, 0);

        host.bootError = null;
        final next = lane.submit(job(2));
        await flush();
        expect(lane.leased, isTrue);
        host.lanes.single.completeAllPending();
        expect((await next).bytes[0], 1);
      },
    );
  });
}
