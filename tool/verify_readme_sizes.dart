// Verifies that every size number in README.md matches reality.
//
// Direction matters: this tool takes MEASUREMENTS, formats each one the
// way the README renders sizes, and reads the README at the one place that
// number lives: a cell of the size table, the last cell of a capability
// row, or a sentence quoted whole. A number that appears somewhere else in
// the README never passes for it. There is no stored expectation list that
// could itself go stale.
//
//   dart tool/verify_readme_sizes.dart            # verify
//   dart tool/verify_readme_sizes.dart --strict   # SKIPs become failures
//
// Plain `dart`, never `dart run`: `dart run` runs the package's build hooks,
// which compile the native engine from source at version 0.0.0.
//
// Measured claims:
//   * native and wasm sizes of every keep-set and build mode, the trim
//     percentage and each capability's build, whose cost is that build less
//     core's — read from
//     tool/.shake_sizes.json, which `make shake-audit` (the single
//     builder/measurer) writes. Missing/stale file → run the audit first.
//   * the default and Asyncify wasm engines, raw and gzipped, of the whole
//     module their parts join to (web_assets/pdf_oxide_manifest.json — the
//     contributor-built artifact; CI rebuilds it at release).
//
// Exit 1 on any mismatch (always) or any SKIP (--strict). Run after an
// engine bump and before a release, right after `make shake-audit`.

import 'dart:convert';
import 'dart:io';

import 'package:pdf_manipulator/src/hook/wasm_parts.dart';

const _readmePath = 'README.md';
const _sizesPath = 'tool/.shake_sizes.json';
const _wasmDir = 'web_assets';
const _wasmPath = '$_wasmDir/pdf_oxide_manifest.json';

void main(List<String> args) {
  final strict = args.contains('--strict');
  final readme = File(_readmePath).readAsStringSync();
  final lines = readme.split('\n');
  final failures = <String>[];
  final skips = <String>[];
  final passes = <String>[];

  final sizesFile = File(_sizesPath);
  final sizes = sizesFile.existsSync()
      ? jsonDecode(sizesFile.readAsStringSync()) as Map<String, dynamic>
      : <String, dynamic>{};
  if (!sizesFile.existsSync()) {
    skips.add('$_sizesPath missing — run `make shake-audit`');
  }
  int? measured(String key) => sizes[key] as int?;

  // The default and Asyncify modules as the app ships them.
  int? wasmRaw, wasmGz, asyncifyRaw, asyncifyGz;
  if (File(_wasmPath).existsSync()) {
    final manifest = WasmManifest.load(Directory(_wasmDir));
    final normal = manifest.modules[normalWasmModule]!;
    wasmRaw = normal.bytes;
    wasmGz = gzipBytesOf(joinWasmParts(Directory(_wasmDir), normal));
    final asyncify = manifest.modules[asyncifyWasmModule];
    if (asyncify != null) {
      asyncifyRaw = asyncify.bytes;
      asyncifyGz = gzipBytesOf(joinWasmParts(Directory(_wasmDir), asyncify));
    } else {
      skips.add(
        'Asyncify wasm sizes ($_wasmPath has no $asyncifyWasmModule — '
        'make web_assets/pdf_oxide_asyncify_bg.0.wasm)',
      );
    }
  } else {
    skips.add('wasm sizes ($_wasmPath missing — dart tool/compile.dart wasm)');
  }

  /// The trimmed cells of the README table row that starts with [row].
  List<String>? cellsOf(String row) {
    final line = lines.where((l) => l.startsWith(row)).firstOrNull;
    if (line == null) return null;
    return line.split('|').map((c) => c.trim()).toList()
      ..removeAt(0)
      ..removeLast();
  }

  /// Checks the cell at [index] of [row] against [expected], built from
  /// [values]; skips when a value is not measured.
  void cell(
    String label,
    String row,
    int index,
    List<int?> values,
    String Function(List<String>) expected,
  ) {
    if (values.contains(null)) {
      skips.add('$label (not measured)');
      return;
    }
    final want = expected([for (final v in values) _mb(v!)]);
    final cells = cellsOf(row);
    if (cells == null || cells.length <= index) {
      failures.add('$label — the README has no row "$row" with that cell');
    } else if (cells[index] != want) {
      failures.add('$label — README says "${cells[index]}", measured "$want"');
    } else {
      passes.add('$label → $want');
    }
  }

  /// Checks that the README holds [sentence] whole, built from [values].
  void sentence(
    String label,
    List<int?> values,
    String Function(List<String>) build,
  ) {
    if (values.contains(null)) {
      skips.add('$label (not measured)');
      return;
    }
    final want = build([for (final v in values) _mb(v!)]);
    if (readme.contains(want)) {
      passes.add('$label → $want');
    } else {
      failures.add('$label — README does not say: $want');
    }
  }

  // ── the size table: native, then web raw (gzipped) ──
  for (final (row, native, raw, gz) in [
    ('| Full · `speed` (default) |', measured('nativeFull'), wasmRaw, wasmGz),
    (
      '| Full · `size` |',
      measured('nativeFullSize'),
      measured('wasmFullSizeRaw'),
      measured('wasmFullSizeGz'),
    ),
    (
      '| Core only · `speed` |',
      measured('nativeCore'),
      measured('wasmCoreRaw'),
      measured('wasmCoreGz'),
    ),
    (
      '| Core only · `size` |',
      measured('nativeCoreSize'),
      measured('wasmCoreSizeRaw'),
      measured('wasmCoreSizeGz'),
    ),
  ]) {
    cell('$row native', row, 1, [native], (v) => v[0]);
    cell('$row web', row, 2, [raw, gz], (v) => '${v[0]} (${v[1]})');
  }

  // ── what `keep` removes ──
  final full = measured('nativeFull'), core = measured('nativeCore');
  if (full == null || core == null) {
    skips.add('trim percentage (not measured)');
  } else {
    final pct = ((full - core) * 100 / full).round();
    final want = 'full → core is about $pct% of the native library gone';
    if (readme.contains(want)) {
      passes.add('trim percentage → $want');
    } else {
      failures.add('trim percentage — README does not say: $want');
    }
  }

  // ── each capability's cost: the last cell of its row. A cost is the
  // capability's build less the build it adds to: core, or core+extract for
  // office, which requires extract. ──
  cell('core size', '| `core` |', 3, [
    core,
  ], (v) => 'always included (~${v[0]})');
  int? cost(String key, int? base) {
    final total = measured(key);
    return total == null || base == null ? null : total - base;
  }

  final extractTotal = measured('nativeCoreExtract');
  for (final (capability, size, suffix) in [
    ('render', cost('nativeCoreRender', core), ''),
    ('signatures', cost('nativeCoreSignatures', core), ''),
    ('pdfa', cost('nativeCorePdfa', core), ''),
    ('extract', cost('nativeCoreExtract', core), ''),
    (
      'office',
      cost('nativeCoreExtractOffice', extractTotal),
      ' on top of `extract`',
    ),
  ]) {
    cell('$capability cost', '| `$capability` |', 3, [
      size,
    ], (v) => '+${v[0]}$suffix');
  }

  // ── the web sections' sentences ──
  sentence('self-hosted download', [
    wasmGz,
    asyncifyGz,
  ], (v) => 'the parts total ${v[0]} gzipped (${v[1]} with Asyncify)');
  sentence(
    'Asyncify trade-off',
    [asyncifyRaw, wasmRaw, asyncifyGz, wasmGz],
    (v) =>
        '(the full engine: ${v[0]} against ${v[1]}, ${v[2]} against ${v[3]} '
        'gzipped)',
  );

  for (final p in passes) {
    stdout.writeln('PASS  $p');
  }
  for (final s in skips) {
    stdout.writeln('SKIP  $s');
  }
  for (final f in failures) {
    stdout.writeln('FAIL  $f');
  }
  if (failures.isNotEmpty || (strict && skips.isNotEmpty)) {
    exitCode = 1;
  }
}

/// Formats bytes the way the README quotes sizes: `N.N MB`, decimal
/// megabytes (1 MB = 1,000,000 bytes).
String _mb(int bytes) => '${(bytes / 1000000).toStringAsFixed(1)} MB';
