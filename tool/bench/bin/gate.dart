// Fails when pdf_document beats pdf_manipulator on size, or our output lost
// fidelity, in out/bench.csv.
//
//   fvm dart run bin/gate.dart [path/to/bench.csv]
//
// Run from tool/bench (`make bench-gate` does, after `make bench`).

import 'dart:io';

import 'package:pdf_manipulator_bench/gate.dart';

void main(List<String> args) {
  final file = File(args.isEmpty ? 'out/bench.csv' : args.first);
  if (!file.existsSync()) {
    stderr.writeln('bench gate: ${file.path} not found; run make bench first');
    exit(1);
  }
  final report = evaluateGate(parseBenchCsv(file.readAsStringSync()));
  stdout
    ..write(report.table)
    ..writeln();
  report.notes.forEach(stdout.writeln);
  report.failures.forEach(stdout.writeln);
  stdout.writeln('bench gate: ${report.failures.length} failures');
  if (report.failures.isNotEmpty) exit(1);
}
