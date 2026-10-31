import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../fixtures/handwritten.dart';
import 'test_source_sink.dart';
import 'timeouts.dart';

/// Registers a stress test with two separate checks.
///
/// The test timeout is a hang guard only (`t(60)`). Speed is checked by
/// [expectWithinBudget], which compares [body] with a calibration run on
/// the same machine, so machine load slows both and cancels out.
///
/// [createPdf] supplies the calibration instance. Set [ownsPdf] when each
/// call returns a new instance, so the test disposes it afterwards.
void stressTest(
  String name,
  Pdf Function() createPdf,
  Future<void> Function() body, {
  required double ratio,
  bool ownsPdf = false,
}) {
  test(name, () async {
    final pdf = createPdf();
    try {
      await expectWithinBudget(name, body, pdf: pdf, ratio: ratio);
    } finally {
      if (ownsPdf) await pdf.dispose();
    }
  }, timeout: t(60));
}

/// Runs [op] and fails when it took more than [ratio] times the
/// calibration workload, measured right before and right after it.
///
/// The calibration is one fixed engine call through [pdf]: open and
/// extract `minimalPdf` 20 times, after one untimed warm-up run, as the
/// minimum of 3 timed samples. It is the divisor because [op] is
/// engine-bound too, so a busy machine or a busy engine thread slows
/// both alike. A fixed CPU loop is measured as well and printed: it
/// tracks core contention only, not engine-thread scheduling, so it
/// does not divide.
Future<void> expectWithinBudget(
  String name,
  Future<void> Function() op, {
  required Pdf pdf,
  required double ratio,
}) async {
  final before = await _calibrate(pdf);
  final sw = Stopwatch()..start();
  await op();
  sw.stop();
  final after = await _calibrate(pdf);

  final opMs = sw.elapsedMicroseconds / 1000;
  final calMs = before.engineMs > after.engineMs
      ? before.engineMs
      : after.engineMs;
  final cpuMs = before.cpuMs > after.cpuMs ? before.cpuMs : after.cpuMs;
  final observed = opMs / calMs;
  print(
    'PERF $name op=${opMs.toStringAsFixed(1)}ms '
    'cal=${calMs.toStringAsFixed(1)}ms '
    'samples=${[...before.samples, ...after.samples].map((s) => s.toStringAsFixed(1)).join('/')} '
    'cpu=${cpuMs.toStringAsFixed(1)}ms '
    'ratio=${observed.toStringAsFixed(2)} budget=$ratio',
  );
  expect(
    observed,
    lessThanOrEqualTo(ratio),
    reason:
        '$name: op ${opMs.toStringAsFixed(1)} ms vs calibration '
        '${calMs.toStringAsFixed(1)} ms = ${observed.toStringAsFixed(2)}x, '
        'over the $ratio x budget',
  );
}

class _Calibration {
  const _Calibration(this.samples, this.cpuMs);
  final List<double> samples;
  final double cpuMs;

  /// Noise only adds time to a fixed workload, so the minimum is the
  /// estimate.
  double get engineMs => samples.reduce((a, b) => a < b ? a : b);
}

Future<_Calibration> _calibrate(Pdf pdf) async {
  final cpu = Stopwatch()..start();
  var x = 0x2545F491;
  for (var i = 0; i < 2000000; i++) {
    x = (x * 1103515245 + 12345) & 0x7fffffff;
  }
  cpu.stop();
  // Reads the loop result so no compiler can drop the loop as dead code.
  if (x < 0) throw StateError('the loop masks x to a non-negative value');

  // The first run pays lane spin-up and first-open costs, so it is not timed.
  await _engineWorkload(pdf);
  final samples = <double>[];
  for (var i = 0; i < 3; i++) {
    final engine = Stopwatch()..start();
    await _engineWorkload(pdf);
    engine.stop();
    samples.add(engine.elapsedMicroseconds / 1000);
  }
  return _Calibration(samples, cpu.elapsedMicroseconds / 1000);
}

Future<void> _engineWorkload(Pdf pdf) async {
  for (var i = 0; i < 20; i++) {
    final doc = await pdf.open(src(minimalPdf));
    await doc.extract(pages: const PdfPages.all());
    await doc.dispose();
  }
}
