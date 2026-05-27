// Timing comparison — measures per-call overhead for editor mutations.
// Registered by both native_runner_test.dart and web_runner_test.dart.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerTimingTests(Pdf Function() createPdf) {
  group('timing', () {
    late Uint8List testPdf;

    setUpAll(() async {
      testPdf = await buildLargePdf(createPdf, pageCount: 50);
    });

    test('10x setTitle', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      final sw = Stopwatch()..start();
      for (var i = 0; i < 10; i++) {
        await ed.setTitle('t$i');
      }
      print('10x setTitle: ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 10}ms/call)');
      await ed.dispose();
    });

    test('10x watermark', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      final sw = Stopwatch()..start();
      for (var i = 0; i < 10; i++) {
        await ed.addWatermark(i, 'W');
      }
      print('10x watermark: ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 10}ms/call)');
      await ed.dispose();
    });

    test('50x watermark (all pages)', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      final sw = Stopwatch()..start();
      for (var i = 0; i < 50; i++) {
        await ed.addWatermark(i, 'W');
      }
      print('50x watermark: ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 50}ms/call)');
      await ed.dispose();
    });

    test('editor open + save', () async {
      final pdf = createPdf();
      final sw = Stopwatch()..start();
      final ed = await pdf.edit(src(testPdf));
      final t1 = sw.elapsedMilliseconds;
      final sink = TestSink();
      await ed.save(sink);
      final t2 = sw.elapsedMilliseconds;
      await ed.dispose();
      print('open: ${t1}ms, save: ${t2 - t1}ms');
    });

    test('3x extractPages (probe cost)', () async {
      final pdf = createPdf();
      final sw = Stopwatch()..start();
      for (var i = 0; i < 3; i++) {
        final sink = TestSink();
        await pdf.extractPages(src(testPdf), sink, pages: List.generate(10, (j) => j));
      }
      print('3x extractPages(10): ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 3}ms/call)');
    });

    test('50x flattenForms (no-op mutation, measures pure round-trip)', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      final sw = Stopwatch()..start();
      for (var i = 0; i < 50; i++) {
        await ed.flattenForms();
      }
      print('50x flattenForms: ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 50}ms/call)');
      await ed.dispose();
    });

    test('50x deletePage(0) then undo via selectPages — heavy engine work', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      final sw = Stopwatch()..start();
      for (var i = 0; i < 50; i++) {
        await ed.rotatePage(i, degrees: 90);
      }
      print('50x rotatePage: ${sw.elapsedMilliseconds}ms (${sw.elapsedMilliseconds / 50}ms/call)');
      await ed.dispose();
    });

    test('per-call microsecond breakdown (10 calls each)', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));

      // Warm up — first call may have lazy init cost
      await ed.setTitle('warmup');
      await ed.addWatermark(0, 'warmup');

      // Measure 10 calls of each, collect individual times
      final setTitleTimes = <int>[];
      for (var i = 0; i < 10; i++) {
        final sw = Stopwatch()..start();
        await ed.setTitle('t$i');
        sw.stop();
        setTitleTimes.add(sw.elapsedMicroseconds);
      }

      final watermarkTimes = <int>[];
      for (var i = 0; i < 10; i++) {
        final sw = Stopwatch()..start();
        await ed.addWatermark(i + 1, 'W');
        sw.stop();
        watermarkTimes.add(sw.elapsedMicroseconds);
      }

      final rotateTimes = <int>[];
      for (var i = 0; i < 10; i++) {
        final sw = Stopwatch()..start();
        await ed.rotatePage(i + 1, degrees: 90);
        sw.stop();
        rotateTimes.add(sw.elapsedMicroseconds);
      }

      final flattenTimes = <int>[];
      for (var i = 0; i < 10; i++) {
        final sw = Stopwatch()..start();
        await ed.flattenForms();
        sw.stop();
        flattenTimes.add(sw.elapsedMicroseconds);
      }

      void report(String name, List<int> times) {
        times.sort();
        final avg = times.reduce((a, b) => a + b) / times.length;
        final median = times[times.length ~/ 2];
        final min = times.first;
        final max = times.last;
        print('$name: avg=${avg.toStringAsFixed(0)}µs median=$medianµs min=$min µs max=$max µs');
      }

      final mediaBoxTimes = <int>[];
      for (var i = 0; i < 10; i++) {
        final sw = Stopwatch()..start();
        await ed.getPageMediaBox(i + 1);
        sw.stop();
        mediaBoxTimes.add(sw.elapsedMicroseconds);
      }

      report('setTitle       ', setTitleTimes);
      report('watermark      ', watermarkTimes);
      report('rotatePage     ', rotateTimes);
      report('flattenForms   ', flattenTimes);
      report('getPageMediaBox', mediaBoxTimes);

      await ed.dispose();
    });

    test('WASM-side timing breakdown (web only — measures inside worker.js)', () async {
      final pdf = createPdf();
      final ed = await pdf.edit(src(testPdf));
      // Warm up
      await ed.addWatermark(0, 'warmup');
      // Call the debug timing op — it runs 10 iterations of each inside worker.js
      // and returns per-call microsecond measurements
      await ed.setTitle('debugTimingWatermark');
      // The debug op runs via editorMutate with editOp='debugTimingWatermark'
      // Results are printed by worker.js console.log — we can't capture them
      // in dart test, so this test just proves it doesn't crash.
      // Check browser devtools console for the actual numbers.
      await ed.dispose();
    });
  });
}
