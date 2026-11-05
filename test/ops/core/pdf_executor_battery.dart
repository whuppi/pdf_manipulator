// CHARTER — this battery alone proves what an executor carries from one
// owner to the next: the keep line retires an executor whose engine peaked
// past it and a fresh one boots, the engine heap an idle executor reports
// does not grow with use, and a fallback font one instance registered does
// not reach the instance that gets the same executor after it. It drives the
// real executors of the platform under test through a pool and a Router.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../fixtures/subset_fallback_font.dart';
import '../../harness/extract_text.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A host that keeps the real executors it boots, so the test reads their
/// heap.
class _TrackingHost implements LaneHost {
  _TrackingHost(this._inner);

  final LaneHost _inner;
  final executors = <Executor>[];

  @override
  PdfIoMode get mode => _inner.mode;

  @override
  int get defaultLaneCount => _inner.defaultLaneCount;

  @override
  String get poolKey => _inner.poolKey;

  @override
  Future<Executor> boot() async {
    final executor = await _inner.boot();
    executors.add(executor);
    return executor;
  }
}

Uint8List _open(int length) =>
    bin.encodeRequest('open', {'sourceLength': length, 'password': null});

Uint8List _info(int handle) =>
    bin.encodeRequest('pageInfo', {'handleId': handle, 'pageIndices': <int>[]});

Uint8List _dispose(int handle) =>
    bin.encodeRequest('docDispose', {'handleId': handle});

/// The field name of the form the flatten proof fills.
const _fieldName = 'Prüfstraße und Hausnr';

/// A figure read twice must agree within this many bytes.
const _flatBytes = 64 * 1024;

void registerExecutorTests(
  Pdf Function() createPdf, {
  PdfConfig? Function()? config,

  /// The worker's WASM linear memory, on the platform that has one.
  int? Function(Executor executor)? memBytes,
}) {
  group('executors', () {
    late _TrackingHost host;
    late ExecutorPool pool;
    late Router router;

    void make({int? keepBytes}) {
      host = _TrackingHost(createLaneHost(config: config?.call()));
      pool = keepBytes == null
          ? ExecutorPool(host: host)
          : ExecutorPool(host: host, keepBytes: keepBytes);
      router = Router(pool: pool, maxLanes: 1, mode: host.mode);
    }

    /// Every executor the pool holds is idle: the lane that used the last
    /// one has been reset or retired.
    Future<void> settle() async {
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (pool.liveCount != pool.idleCount) {
        if (DateTime.now().isAfter(deadline)) {
          fail('the pool never took its executors back');
        }
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
    }

    Future<void> booted() async {
      pool.prewarm();
      while (pool.idleCount == 0) {
        await Future<void>.delayed(const Duration(milliseconds: 5));
      }
    }

    /// One small op on one fixture: open it, read it, dispose it. Returns
    /// the heap the executor reports while the document is open.
    Future<int> cycle() async {
      final source = src(minimalPdf);
      final opened = await router.execute(
        _open(source.length),
        sources: [source],
        keepSources: {0},
      );
      bin.decodeResponse(opened.bytes);
      final handle = peekResponseHandleId(opened.bytes)!;
      await router.execute(_info(handle));
      final open = host.executors.last.heapBytes;
      await router.execute(_dispose(handle));
      for (final resource in opened.resourceIds.values) {
        await router.releaseSource(resource);
      }
      return open;
    }

    tearDown(() async {
      await router.dispose();
      pool.dispose();
    });

    test('an executor whose engine peaked past the keep line is retired and '
        'a fresh one boots', () async {
      make(keepBytes: 1);
      await booted();
      final boots = ExecutorPool.totalBoots;

      final open = await cycle();
      expect(open, greaterThan(0), reason: 'an op that allocates reports it');
      await settle();
      expect(pool.liveCount, 0, reason: 'the executor was kept');
      expect(ExecutorPool.totalBoots, boots, reason: 'no one asked yet');

      await cycle();
      await settle();
      expect(
        ExecutorPool.totalBoots - boots,
        1,
        reason: 'the next op did not boot a fresh executor',
      );
    }, timeout: t(10));

    test('an executor under the keep line serves the next op', () async {
      make();
      await booted();
      final boots = ExecutorPool.totalBoots;

      await cycle();
      await settle();
      await cycle();
      await settle();
      expect(ExecutorPool.totalBoots, boots);
      expect(pool.liveCount, 1);
    }, timeout: t(10));

    test('the heap an idle executor reports does not grow over fifty cycles '
        'with a dispose every ten', () async {
      make();
      final idle = <int>[];
      final mem = <int?>[];
      for (var i = 1; i <= 50; i++) {
        await cycle();
        await settle();
        final executor = host.executors.single;
        idle.add(executor.heapBytes);
        mem.add(memBytes?.call(executor));
        if (i % 10 == 0 && i < 50) {
          await router.dispose();
          router = Router(pool: pool, maxLanes: 1, mode: host.mode);
        }
      }
      printOnFailure('idle heap by cycle: $idle');
      printOnFailure('mem by cycle: $mem');
      expect(host.executors, hasLength(1), reason: 'the executor was replaced');
      final settled = idle[10];
      for (var i = 10; i < 50; i++) {
        expect(
          (idle[i] - settled).abs(),
          lessThanOrEqualTo(_flatBytes),
          reason: 'cycle ${i + 1} reports ${idle[i]}, cycle 11 $settled',
        );
      }
      if (mem[9] != null) {
        expect(mem[49], mem[9], reason: 'the worker grew after cycle 10');
      }
    }, timeout: t(30));
  });

  group('executors across instances', () {
    /// Fills the form with a CJK value, flattens it and reads the result.
    Future<String> flattened(Pdf pdf) async {
      final editor = await pdf.edit(PdfInput(src(formUtf16NamePdf)));
      await editor
          .field(_fieldName)
          .setValue(PdfFormFieldValue.text('東京都渋谷区 123'));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      await doc.dispose();
      return text;
    }

    test('a fallback font one instance registered does not reach the next '
        'instance', () async {
      final a = createPdf();
      await a.registerFallbackFont(
        PdfFallbackFontKind.cjk,
        src(subsetCjkFallbackFont),
      );
      expect(
        await flattened(a),
        contains('東京都渋谷区'),
        reason: 'the registration did not reach its own instance',
      );
      await a.dispose();

      final b = createPdf();
      final text = await flattened(b);
      await b.dispose();
      expect(
        text,
        isNot(contains('東京都渋谷区')),
        reason: 'the second instance baked the first one\'s font',
      );
    }, timeout: t(10));
  });
}
