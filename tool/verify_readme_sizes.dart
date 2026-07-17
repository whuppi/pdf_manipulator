// Verifies that every size number in README.md matches reality.
//
// Direction matters: this tool takes MEASUREMENTS, formats each one the
// way the README renders sizes, and asserts the README CONTAINS that
// string. The README is checked against reality — there is no stored
// expectation list that could itself go stale.
//
//   dart run tool/verify_readme_sizes.dart            # verify
//   dart run tool/verify_readme_sizes.dart --strict   # SKIPs become failures
//
// Measured claims:
//   * full + core-only native library and the trim percentage — read from
//     tool/.shake_sizes.json, which `make shake-audit` (the single
//     builder/measurer) writes. Missing/stale file → run the audit first.
//   * default wasm engine, raw and gzipped (web_assets/pdf_oxide_bg.wasm —
//     the contributor-built artifact; CI rebuilds it at release).
//
// Exit 1 on any mismatch (always) or any SKIP (--strict). Run after an
// engine bump and before a release, right after `make shake-audit`.

import 'dart:convert';
import 'dart:io';

const _readmePath = 'README.md';
const _sizesPath = 'tool/.shake_sizes.json';
const _wasmPath = 'web_assets/pdf_oxide_bg.wasm';

void main(List<String> args) {
  final strict = args.contains('--strict');
  final readme = File(_readmePath).readAsStringSync();
  final failures = <String>[];
  final skips = <String>[];
  final passes = <String>[];

  void check(String label, List<String> candidates) {
    if (candidates.any(readme.contains)) {
      passes.add('$label → ${candidates.join(' | ')}');
    } else {
      failures.add(
        '$label — README contains none of: ${candidates.join(' | ')}',
      );
    }
  }

  // ── native full / core / percentage (from the shake-audit record) ──
  final sizesFile = File(_sizesPath);
  if (!sizesFile.existsSync()) {
    skips.add('native sizes ($_sizesPath missing — run `make shake-audit`)');
  } else {
    final sizes =
        jsonDecode(sizesFile.readAsStringSync()) as Map<String, dynamic>;
    final full = sizes['nativeFull'] as int;
    final core = sizes['nativeCore'] as int;
    check('native full', [_mb(full)]);
    check('native core', [_mb(core)]);
    final pct = ((full - core) * 100 / full).round();
    check('trim percentage', ['about $pct% of the native library']);
    final wasmCoreRaw = sizes['wasmCoreRaw'] as int?;
    final wasmCoreGz = sizes['wasmCoreGz'] as int?;
    if (wasmCoreRaw == null || wasmCoreGz == null) {
      skips.add('wasm core sizes (run `SHAKE_AUDIT_WASM=1 make shake-audit`)');
    } else {
      check('wasm core raw', [_mb(wasmCoreRaw)]);
      check('wasm core gzipped', [_mb(wasmCoreGz)]);
    }
    for (final (label, key) in [
      ('render cost', 'capRender'),
      ('signatures cost', 'capSignatures'),
      ('pdfa cost', 'capPdfa'),
      ('office cost', 'capOffice'),
      ('extract cost', 'capExtract'),
    ]) {
      final cost = sizes[key] as int?;
      if (cost == null) {
        skips.add('$label (run `SHAKE_AUDIT_CAPS=1 make shake-audit`)');
      } else {
        check(label, ['+${(cost / 1000000).toStringAsFixed(1)} MB']);
      }
    }
  }

  // ── default wasm, raw + gzipped ──
  final wasm = File(_wasmPath);
  if (!wasm.existsSync()) {
    skips.add(
      'wasm sizes ($_wasmPath missing — bash tool/compile_rust.sh wasm)',
    );
  } else {
    check('wasm raw', [_mb(wasm.lengthSync())]);
    final gz = gzip.encode(wasm.readAsBytesSync()).length;
    check('wasm gzipped', [_mb(gz)]);
  }

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

/// Formats bytes the way the README quotes sizes: `~N.N MB`, decimal
/// megabytes (1 MB = 1,000,000 bytes) — the convention every size in this
/// repo's prose uses.
String _mb(int bytes) => '~${(bytes / 1000000).toStringAsFixed(1)} MB';
