// Cross-verification: the Dart EngineOp enum vs the Rust dispatcher.
//
// Parses bridge_api.rs from disk, extracts every match arm
// programmatically, and checks parity with EngineOp. If an op exists
// in one layer but not the other, this test fails. One known
// exception: 'watermark' appears in both edit and page dispatch
// (editor watermark + builder page watermark).
//
// The web twin (lane_worker.js ↔ lane protocol) lives in
// test/runtime/web/lane_worker_sync_test.dart.

@TestOn('vm')
library;

// io-exempt: reads bridge_api.rs from disk to check Dart/Rust op parity.
import 'dart:io';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:test/test.dart';

// ── Source extractors ──

/// Extracts the three Rust op sets. Top-level ops are op_unit!
/// declarations under host/ops/ (the registry's unit files); the
/// editorMutate / builderPageOp sub-commands are still match arms inside
/// bridge_api.rs handlers. An op missing its unit fails the top-level
/// test; a unit missing its registry row surfaces as an unused-static
/// warning in `make analyze`.
({Set<String> topLevel, Set<String> editMutate, Set<String> pageOp})
_parseRustDispatch(String source) {
  final unitPattern = RegExp(
    r'op_unit!\(\s*\w+,\s*"([a-zA-Z]+)"',
    multiLine: true,
  );
  final topLevel = <String>{};
  final opsDir = Directory('vendor/pdf_oxide/src/host/ops');
  if (!opsDir.existsSync()) throw StateError('host/ops/ not found');
  for (final f in opsDir.listSync().whereType<File>()) {
    if (!f.path.endsWith('.rs')) continue;
    final s = f.readAsStringSync();
    topLevel.addAll(unitPattern.allMatches(s).map((m) => m.group(1)!));
  }
  if (topLevel.isEmpty) throw StateError('no op_unit! declarations found');

  // Sub-dispatched ops keep their match arms inside bridge_api.rs.
  final mutateStart = source.indexOf('fn handle_editor_mutate(');
  final pageOpStart = source.indexOf('fn handle_builder_page_op(');

  if (mutateStart == -1) throw StateError('handle_editor_mutate not found');
  if (pageOpStart == -1) throw StateError('handle_builder_page_op not found');

  final mutateSource = source.substring(mutateStart, pageOpStart);
  final pageSource = source.substring(pageOpStart);

  final armPattern = RegExp(r'"([a-zA-Z]+)"\s*=>');

  return (
    topLevel: topLevel,
    editMutate: armPattern
        .allMatches(mutateSource)
        .map((m) => m.group(1)!)
        .toSet(),
    pageOp: armPattern.allMatches(pageSource).map((m) => m.group(1)!).toSet(),
  );
}

void main() {
  // ── EngineOp enum integrity ──

  group('EngineOp enum', () {
    test('every value has a non-empty camelCase wire name', () {
      for (final op in EngineOp.values) {
        expect(op.wire, isNotEmpty, reason: '${op.name} has empty wire');
        expect(
          op.wire,
          matches(RegExp(r'^[a-zA-Z]+$')),
          reason: '${op.wire} contains non-alpha chars',
        );
      }
    });

    test('no duplicate wire names', () {
      final seen = <String>{};
      for (final op in EngineOp.values) {
        expect(
          seen.add(op.wire),
          isTrue,
          reason: 'Duplicate wire name: ${op.wire}',
        );
      }
    });
  });

  // ── Rust bridge_api.rs sync ──

  group('bridge_api.rs ↔ EngineOp', () {
    late Set<String> rustTopLevel;
    late Set<String> rustEditMutate;
    late Set<String> rustPageOp;

    setUpAll(() {
      final file = File('vendor/pdf_oxide/src/host/bridge_api.rs');
      if (!file.existsSync()) fail('bridge_api.rs not found');
      final parsed = _parseRustDispatch(file.readAsStringSync());
      rustTopLevel = parsed.topLevel;
      rustEditMutate = parsed.editMutate;
      rustPageOp = parsed.pageOp;
    });

    test('Rust top-level has a case for every top-level EngineOp', () {
      // EngineOps dispatched as sub-commands inside editorMutate or
      // builderPageOp never appear in the top-level match. The parsed
      // sets decide which — an op whose wire name appears in the
      // editMutate or pageOp set is sub-dispatched, not hardcoded here.
      final subDispatched = rustEditMutate.union(rustPageOp);

      for (final op in EngineOp.values) {
        if (subDispatched.contains(op.wire)) continue;
        expect(
          rustTopLevel,
          contains(op.wire),
          reason:
              'EngineOp.${op.name} (${op.wire}) missing from top-level dispatch',
        );
      }
    });

    test('every Rust case maps to an EngineOp or a known sub-dispatch', () {
      final dartWires = EngineOp.values.map((op) => op.wire).toSet();
      final allRust = rustTopLevel.union(rustEditMutate).union(rustPageOp);
      final unmapped = allRust.difference(dartWires);

      // Sub-dispatch ops (edit mutations + page builder ops) don't have
      // their own EngineOp — they're dispatched inside editorMutate /
      // builderPageOp. That's correct. But they must be ONLY in those
      // sub-dispatch functions, never orphaned in top-level.
      final legitimateSubs = rustEditMutate.union(rustPageOp);
      final orphans = unmapped.difference(legitimateSubs);

      expect(
        orphans,
        isEmpty,
        reason:
            'Rust has cases not in EngineOp and not in any sub-dispatch: $orphans',
      );
    });

    test('edit sub-dispatch and page sub-dispatch are disjoint', () {
      final overlap = rustEditMutate.intersection(rustPageOp);
      // 'watermark' legitimately appears in both (editor watermark + builder page watermark)
      final realOverlap = overlap.difference({'watermark'});
      expect(
        realOverlap,
        isEmpty,
        reason: 'Unexpected overlap between edit and page ops: $realOverlap',
      );
    });

    test('sub-dispatch ops are NOT duplicated in top-level', () {
      final editInTop = rustTopLevel.intersection(rustEditMutate);
      final pageInTop = rustTopLevel.intersection(rustPageOp);
      // Some ops like 'watermark' may appear at top-level too (different handler)
      // but pure sub-ops should not appear at top-level
      expect(
        editInTop,
        isEmpty,
        reason: 'Edit sub-ops leaked into top-level dispatch: $editInTop',
      );
      expect(
        pageInTop,
        isEmpty,
        reason: 'Page sub-ops leaked into top-level dispatch: $pageInTop',
      );
    });
  });

  // ── Error kinds ──

  test('WireErrorKind matches Rust ErrorKind name for name, byte for byte', () {
    final source = File(
      'vendor/pdf_oxide/src/host/binary_codec.rs',
    ).readAsStringSync();
    final start = source.indexOf('pub enum ErrorKind {');
    if (start == -1) fail('pub enum ErrorKind not found');
    final body = source.substring(start, source.indexOf('}', start));
    final rust = {
      for (final m in RegExp(
        r'^\s*(\w+) = (\d+),',
        multiLine: true,
      ).allMatches(body))
        int.parse(m.group(2)!): m.group(1)!,
    };
    final dart = {
      for (final k in WireErrorKind.values)
        k.index: '${k.name[0].toUpperCase()}${k.name.substring(1)}',
    };
    expect(rust, isNotEmpty);
    expect(dart, rust);
  });

  group('WasmPageOp ↔ host PageOp parity', () {
    /// Variant names of a Rust enum body: every line whose first token
    /// starts an uppercase identifier (variant vs field/comment lines).
    Set<String> variants(String body) {
      final names = <String>{};
      for (final line in body.split('\n')) {
        final t = line.trim();
        if (t == '}' || t.isEmpty) continue;
        final first = RegExp(r'^([A-Z][A-Za-z0-9]*)').firstMatch(t);
        if (first != null) names.add(first.group(1)!);
      }
      return names;
    }

    test('every WasmPageOp variant has a host PageOp', () {
      final wasmSource = File(
        'vendor/pdf_oxide/src/wasm.rs',
      ).readAsStringSync();
      final hostSource = File(
        'vendor/pdf_oxide/src/host/dispatch.rs',
      ).readAsStringSync();

      final wasmStart = wasmSource.indexOf('enum WasmPageOp {');
      if (wasmStart == -1) fail('enum WasmPageOp not found');
      final wasmBody = wasmSource.substring(
        wasmStart,
        wasmSource.indexOf('\n}', wasmStart),
      );
      final hostStart = hostSource.indexOf('pub enum PageOp {');
      if (hostStart == -1) fail('pub enum PageOp not found');
      final hostBody = hostSource.substring(
        hostStart,
        hostSource.indexOf('\n}', hostStart),
      );
      final hostVariants = variants(hostBody);

      // Engine variants the host intentionally does not mirror as its
      // own PageOp call:
      // * BarcodeImage — same engine call as Image (image_from_bytes).
      // * ImageWithAlt — the existing `image` page op.
      const explained = {'BarcodeImage', 'ImageWithAlt'};

      final missing = variants(
        wasmBody,
      ).difference(hostVariants).difference(explained).toList();
      expect(
        missing,
        isEmpty,
        reason:
            'WasmPageOp variants without a host PageOp — add every '
            'variant to host/dispatch.rs so the Dart page builder can '
            'reach it too: $missing',
      );
    });
  });

  // ── Web worker.js sync ──
}
