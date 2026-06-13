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

import 'dart:io';

import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:test/test.dart';

// ── Source extractors ──

/// Splits bridge_api.rs into three disjoint sets by function boundary.
({Set<String> topLevel, Set<String> editMutate, Set<String> pageOp})
_parseRustDispatch(String source) {
  // Find function boundaries
  final mutateStart = source.indexOf('fn handle_editor_mutate(');
  final pageOpStart = source.indexOf('fn handle_builder_page_op(');

  if (mutateStart == -1) throw StateError('handle_editor_mutate not found');
  if (pageOpStart == -1) throw StateError('handle_builder_page_op not found');

  // Top-level = everything before handle_editor_mutate
  final topSource = source.substring(0, mutateStart);
  final mutateSource = source.substring(mutateStart, pageOpStart);
  final pageSource = source.substring(pageOpStart);

  final armPattern = RegExp(r'"([a-zA-Z]+)"\s*=>');

  return (
    topLevel: armPattern.allMatches(topSource).map((m) => m.group(1)!).toSet(),
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

  // ── Web worker.js sync ──
}
