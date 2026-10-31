// Compression benchmark: pdf_manipulator compress against pdf_document
// optimize, over the corpus in corpus.toml plus generated cases.
//
//   fvm dart run bin/bench.dart [--only id,id] [--max-pages N]
//
// Run from tool/bench (`make bench` does).

import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator_bench/corpus.dart';
import 'package:pdf_manipulator_bench/engines.dart';
import 'package:pdf_manipulator_bench/fidelity.dart';
import 'package:pdf_manipulator_bench/generated.dart';

const _csvHeader =
    'case,kind,engine,preset,in_bytes,out_bytes,ratio,ms,ms_median,'
    'deterministic,'
    'pages_in,pages_out,worst_rmse,worst_rmse_page,worst_ssim,'
    'worst_ssim_page,text_equal,error';

/// Compress runs per config; the row reports the fastest and the median.
const _runs = 5;

Future<void> main(List<String> args) async {
  final totalWatch = Stopwatch()..start();
  final only = _option(args, '--only')?.split(',').toSet();
  final maxPages = int.parse(_option(args, '--max-pages') ?? '0');
  final root = Directory.current;
  final corpus = Corpus(
    manifest: File('${root.path}/corpus.toml'),
    cacheDir: Directory('${root.path}/.corpus'),
  );
  final out = Directory('${root.path}/out')..createSync(recursive: true);

  final pdf = Pdf();
  final rows = <Row>[];
  try {
    final entries = {for (final e in corpus.entries()) e.id: e};
    final inputs = <({String id, String kind, Uint8List bytes})>[];
    for (final entry in entries.values) {
      if (only != null && !only.contains(entry.id)) continue;
      try {
        inputs.add((
          id: entry.id,
          kind: entry.kind,
          bytes: await corpus.bytes(entry),
        ));
      } on Object catch (e) {
        stdout.writeln('skip ${entry.id}: $e');
      }
    }
    for (final generated in generatedCases) {
      if (only != null && !only.contains(generated.id)) continue;
      final base = entries[generated.baseId];
      if (base == null) continue;
      try {
        inputs.add((
          id: generated.id,
          kind: generated.kind,
          bytes: await generated.build(pdf, await corpus.bytes(base)),
        ));
      } on Object catch (e) {
        stdout.writeln('skip ${generated.id}: $e');
      }
    }

    for (final input in inputs) {
      final caseWatch = Stopwatch()..start();
      final source = await pdf.open(MemorySource(input.bytes));
      try {
        rows.addAll(await _measureCase(pdf, source, input, maxPages: maxPages));
        stdout.writeln(
          '${input.id}: ${source.pageCount} pages, '
          '${(caseWatch.elapsedMilliseconds / 1000).toStringAsFixed(1)}s '
          '(${rows.length} rows)',
        );
      } finally {
        await source.dispose();
      }
    }
  } finally {
    await pdf.dispose();
  }
  stdout.writeln(
    'total ${(totalWatch.elapsedMilliseconds / 1000).toStringAsFixed(1)}s',
  );

  final csv = StringBuffer()..writeln(_csvHeader);
  for (final row in rows) {
    csv.writeln(row.csv());
  }
  File('${out.path}/bench.csv').writeAsStringSync(csv.toString());
  final table = _markdown(rows);
  File('${out.path}/bench.md').writeAsStringSync(table);
  stdout.write('\n$table');
}

String? _option(List<String> args, String name) {
  final i = args.indexOf(name);
  return i < 0 || i + 1 >= args.length ? null : args[i + 1];
}

typedef _Timed = ({
  int index,
  Uint8List out,
  int ms,
  int median,
  bool deterministic,
});

/// Times every config of one case, then scores each distinct output once:
/// configs whose output bytes are identical share one candidate document and
/// one fidelity result.
Future<List<Row>> _measureCase(
  Pdf pdf,
  PdfDoc source,
  ({String id, String kind, Uint8List bytes}) input, {
  required int maxPages,
}) async {
  final all = configs(pdf);
  final rows = List<Row?>.filled(all.length, null);
  final timed = <_Timed>[];
  for (var i = 0; i < all.length; i++) {
    final config = all[i];
    Row failed(int ms, String message) => Row(
      id: input.id,
      kind: input.kind,
      engine: config.engine,
      preset: config.preset,
      inBytes: input.bytes.length,
      ms: ms,
      msMedian: ms,
      deterministic: true,
      pagesIn: source.pageCount,
      error: message,
    );
    final times = <int>[];
    var deterministic = true;
    Uint8List? first;
    try {
      for (var run = 0; run < _runs; run++) {
        final stopwatch = Stopwatch()..start();
        final result = await config.run(input.bytes);
        stopwatch.stop();
        times.add(stopwatch.elapsedMicroseconds);
        if (first == null) {
          first = result;
        } else if (!_sameBytes(first, result)) {
          deterministic = false;
        }
      }
    } on Object catch (e) {
      rows[i] = failed(
        times.isEmpty ? 0 : _min(times) ~/ 1000,
        e.toString().split('\n').first,
      );
    }
    if (rows[i] != null) continue;
    final sorted = [...times]..sort();
    timed.add((
      index: i,
      out: first!,
      ms: sorted.first ~/ 1000,
      median: sorted[sorted.length ~/ 2] ~/ 1000,
      deterministic: deterministic,
    ));
  }

  final groups = <List<_Timed>>[];
  for (final t in timed) {
    final group = groups.firstWhere(
      (g) => _sameBytes(g.first.out, t.out),
      orElse: () {
        final created = <_Timed>[];
        groups.add(created);
        return created;
      },
    );
    group.add(t);
  }

  final candidates = <PdfDoc>[];
  final opened = <List<_Timed>>[];
  try {
    for (final group in groups) {
      try {
        candidates.add(await pdf.open(MemorySource(group.first.out)));
        opened.add(group);
      } on Object catch (e) {
        for (final t in group) {
          rows[t.index] = Row(
            id: input.id,
            kind: input.kind,
            engine: all[t.index].engine,
            preset: all[t.index].preset,
            inBytes: input.bytes.length,
            ms: t.ms,
            msMedian: t.median,
            deterministic: t.deterministic,
            pagesIn: source.pageCount,
            error: e.toString().split('\n').first,
          );
        }
      }
    }
    final outcomes = await compareAll(source, candidates, maxPages: maxPages);
    for (var g = 0; g < opened.length; g++) {
      for (final t in opened[g]) {
        rows[t.index] = Row(
          id: input.id,
          kind: input.kind,
          engine: all[t.index].engine,
          preset: all[t.index].preset,
          inBytes: input.bytes.length,
          outBytes: t.out.length,
          ms: t.ms,
          msMedian: t.median,
          deterministic: t.deterministic,
          pagesIn: source.pageCount,
          pagesOut: candidates[g].pageCount,
          fidelity: outcomes[g].fidelity,
          error: outcomes[g].error,
        );
      }
    }
  } finally {
    for (final candidate in candidates) {
      await candidate.dispose();
    }
  }
  return rows.cast<Row>();
}

int _min(List<int> values) => values.reduce((a, b) => a < b ? a : b);

bool _sameBytes(Uint8List a, Uint8List b) {
  if (a.length != b.length) return false;
  for (var i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// One measured (case, engine, preset) cell.
class Row {
  Row({
    required this.id,
    required this.kind,
    required this.engine,
    required this.preset,
    required this.inBytes,
    required this.ms,
    required this.msMedian,
    required this.deterministic,
    required this.pagesIn,
    this.outBytes,
    this.pagesOut,
    this.fidelity,
    this.error,
  });

  final String id;
  final String kind;
  final String engine;
  final String preset;
  final int inBytes;
  final int? outBytes;

  /// Fastest and median of the [_runs] compress runs.
  final int ms;
  final int msMedian;

  /// Whether all [_runs] outputs were byte-identical; the row reports the
  /// first.
  final bool deterministic;
  final int pagesIn;
  final int? pagesOut;
  final Fidelity? fidelity;
  final String? error;

  double? get ratio => outBytes == null ? null : outBytes! / inBytes;

  String csv() {
    final f = fidelity;
    return [
      id,
      kind,
      engine,
      preset,
      inBytes,
      outBytes ?? '',
      ratio?.toStringAsFixed(4) ?? '',
      ms,
      msMedian,
      deterministic ? 'yes' : 'no',
      pagesIn,
      pagesOut ?? '',
      f?.worstRmse.toStringAsFixed(4) ?? '',
      f?.worstRmsePage ?? '',
      f?.worstSsim.toStringAsFixed(5) ?? '',
      f?.worstSsimPage ?? '',
      f?.textEqual ?? '',
      '"${(error ?? '').replaceAll('"', "'")}"',
    ].join(',');
  }
}

String _markdown(List<Row> rows) {
  final b = StringBuffer()
    ..writeln(
      '| case | engine | preset | in | out | ratio | ms '
      '| deterministic | pages | rmse (page) | ssim (page) | text |',
    )
    ..writeln('|---|---|---|---:|---:|---:|---:|---|---|---|---|---|');
  for (final r in rows) {
    final f = r.fidelity;
    if (f == null) {
      b.writeln(
        '| ${r.id} | ${r.engine} | ${r.preset} | ${r.inBytes} '
        '| - | - | ${r.ms} | ${r.deterministic ? 'yes' : 'no'} '
        '| ${r.pagesIn} | - | - | ERR: ${r.error} |',
      );
      continue;
    }
    b.writeln(
      '| ${r.id} | ${r.engine} | ${r.preset} | ${r.inBytes} '
      '| ${r.outBytes} | ${r.ratio!.toStringAsFixed(3)} | ${r.ms} '
      '| ${r.deterministic ? 'yes' : 'no'} '
      '| ${r.pagesIn}/${r.pagesOut} '
      '| ${f.worstRmse.toStringAsFixed(3)} (${f.worstRmsePage}) '
      '| ${f.worstSsim.toStringAsFixed(4)} (${f.worstSsimPage}) '
      '| ${f.textEqual ? 'same' : 'DIFF'} |',
    );
  }
  return b.toString();
}
