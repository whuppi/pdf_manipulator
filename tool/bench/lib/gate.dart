// The bench gate: reads out/bench.csv (the header is `_csvHeader` in
// bin/bench.dart) and decides whether pdf_manipulator still holds its ground
// against pdf_document. bin/gate.dart is the thin main; the rules live here so
// a test can call them.

const _oursEngine = 'pdf_manipulator';
const _theirsEngine = 'pdf_document';

/// Columns before the free-text `error`; the error may hold commas, so
/// everything after these fields is the error.
const _fixedFields = 17;

/// One bench.csv row, the columns the gate reads.
class GateRow {
  const GateRow({
    required this.id,
    required this.engine,
    required this.preset,
    required this.outBytes,
    required this.pagesIn,
    required this.pagesOut,
    required this.worstRmse,
    required this.textEqual,
    required this.error,
  });

  final String id;
  final String engine;
  final String preset;
  final int? outBytes;
  final String pagesIn;
  final String pagesOut;
  final String worstRmse;
  final String textEqual;
  final String error;
}

/// Parses bench.csv text. The first line is the header and is skipped.
List<GateRow> parseBenchCsv(String text) {
  final lines = text.split('\n').where((l) => l.trim().isNotEmpty).skip(1);
  final rows = <GateRow>[];
  for (final line in lines) {
    final fields = line.trimRight().split(',');
    if (fields.length < _fixedFields + 1) {
      throw FormatException('bench.csv row has too few fields', line);
    }
    final error = fields.sublist(_fixedFields).join(',');
    rows.add(
      GateRow(
        id: fields[0],
        engine: fields[2],
        preset: fields[3],
        outBytes: int.tryParse(fields[5]),
        pagesIn: fields[10],
        pagesOut: fields[11],
        worstRmse: fields[12],
        textEqual: fields[16],
        error: error.startsWith('"') && error.endsWith('"') && error.length > 1
            ? error.substring(1, error.length - 1)
            : error,
      ),
    );
  }
  return rows;
}

/// What the gate found: one line per failure, one per note, and the summary
/// table.
class GateReport {
  const GateReport({
    required this.failures,
    required this.notes,
    required this.table,
  });

  final List<String> failures;
  final List<String> notes;
  final String table;
}

/// Applies the gate rules to [rows].
///
/// A case and preset fails when our row has an error, loses pages or text, a
/// lossless row differs from its source, or pdf_document's output is smaller
/// than ours. An error in pdf_document's row alone is a note.
GateReport evaluateGate(List<GateRow> rows) {
  final ours = <String, GateRow>{};
  final theirs = <String, GateRow>{};
  for (final row in rows) {
    final key = '${row.id}\u0000${row.preset}';
    if (row.engine == _oursEngine) ours[key] = row;
    if (row.engine == _theirsEngine) theirs[key] = row;
  }

  final failures = <String>[];
  final notes = <String>[];
  final table = StringBuffer()
    ..writeln('| case | preset | ours | theirs | ratio (ours/theirs) |')
    ..writeln('|---|---|---:|---:|---:|');

  for (final entry in ours.entries) {
    final our = entry.value;
    final their = theirs[entry.key];
    final ourSize = our.outBytes;
    final theirSize = their?.outBytes;
    final sizes =
        'ours ${ourSize ?? '-'} bytes, theirs ${theirSize ?? '-'} bytes';
    void fail(String why) =>
        failures.add('FAIL ${our.id} ${our.preset}: $why ($sizes)');

    if (their != null && their.error.isNotEmpty) {
      notes.add(
        'note ${their.id} ${their.preset}: pdf_document errored: '
        '${their.error}',
      );
    }

    final ratio = ourSize != null && theirSize != null && theirSize > 0
        ? (ourSize / theirSize).toStringAsFixed(3)
        : '-';
    table.writeln(
      '| ${our.id} | ${our.preset} | ${ourSize ?? '-'} '
      '| ${theirSize ?? '-'} | $ratio |',
    );

    if (our.error.isNotEmpty) {
      fail('pdf_manipulator errored: ${our.error}');
      continue;
    }
    if (our.pagesOut != our.pagesIn) {
      fail('pages ${our.pagesIn} became ${our.pagesOut}');
    }
    if (our.textEqual != 'true') {
      fail('extracted text differs from the source');
    }
    if (our.preset == 'lossless' && our.worstRmse != '0.0000') {
      fail('lossless render differs from the source (rmse ${our.worstRmse})');
    }
    if (ourSize != null && theirSize != null && theirSize < ourSize) {
      fail('pdf_document is smaller');
    }
  }

  return GateReport(failures: failures, notes: notes, table: table.toString());
}
