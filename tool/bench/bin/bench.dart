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
    'case,kind,engine,preset,in_bytes,out_bytes,ratio,ms,'
    'pages_in,pages_out,worst_rmse,worst_rmse_page,worst_ssim,'
    'worst_ssim_page,text_equal,error';

Future<void> main(List<String> args) async {
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
      final source = await pdf.open(MemorySource(input.bytes));
      try {
        for (final config in configs(pdf)) {
          stdout.writeln(
            '${input.id} ${config.engine} ${config.preset} '
            '(${rows.length} rows)',
          );
          rows.add(
            await _measure(pdf, source, input, config, maxPages: maxPages),
          );
        }
      } finally {
        await source.dispose();
      }
    }
  } finally {
    await pdf.dispose();
  }

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

Future<Row> _measure(
  Pdf pdf,
  PdfDoc source,
  ({String id, String kind, Uint8List bytes}) input,
  Config config, {
  required int maxPages,
}) async {
  final stopwatch = Stopwatch()..start();
  try {
    final result = await config.run(input.bytes);
    stopwatch.stop();
    final candidate = await pdf.open(MemorySource(result));
    try {
      final f = await compare(source, candidate, maxPages: maxPages);
      return Row(
        id: input.id,
        kind: input.kind,
        engine: config.engine,
        preset: config.preset,
        inBytes: input.bytes.length,
        outBytes: result.length,
        ms: stopwatch.elapsedMilliseconds,
        pagesIn: source.pageCount,
        pagesOut: candidate.pageCount,
        fidelity: f,
      );
    } finally {
      await candidate.dispose();
    }
  } on Object catch (e) {
    return Row(
      id: input.id,
      kind: input.kind,
      engine: config.engine,
      preset: config.preset,
      inBytes: input.bytes.length,
      ms: stopwatch.elapsedMilliseconds,
      pagesIn: source.pageCount,
      error: e.toString().split('\n').first,
    );
  }
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
  final int ms;
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
      '| case | engine | preset | in | out | ratio | ms | pages '
      '| rmse (page) | ssim (page) | text |',
    )
    ..writeln('|---|---|---|---:|---:|---:|---:|---|---|---|---|');
  for (final r in rows) {
    final f = r.fidelity;
    if (f == null) {
      b.writeln(
        '| ${r.id} | ${r.engine} | ${r.preset} | ${r.inBytes} '
        '| - | - | ${r.ms} | ${r.pagesIn} | - | - | ERR: ${r.error} |',
      );
      continue;
    }
    b.writeln(
      '| ${r.id} | ${r.engine} | ${r.preset} | ${r.inBytes} '
      '| ${r.outBytes} | ${r.ratio!.toStringAsFixed(3)} | ${r.ms} '
      '| ${r.pagesIn}/${r.pagesOut} '
      '| ${f.worstRmse.toStringAsFixed(3)} (${f.worstRmsePage}) '
      '| ${f.worstSsim.toStringAsFixed(4)} (${f.worstSsimPage}) '
      '| ${f.textEqual ? 'same' : 'DIFF'} |',
    );
  }
  return b.toString();
}
