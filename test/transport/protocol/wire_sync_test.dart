// Cross-verification: EngineOp wire names vs Rust + JS dispatchers.
//
// Parses bridge_api.rs, worker.js, and coordinator.js from disk,
// extracts every match/case arm programmatically, and checks parity
// with the Dart EngineOp enum.
//
// If an op exists in one layer but not the others, this test fails.
// All ops extracted from source. One known exception: 'watermark' appears
// in both edit and page dispatch (editor watermark + builder page watermark).

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:test/test.dart';

// ── Source extractors ──

Set<String> _extractJsCases(String source) {
  final pattern = RegExp(r"case\s+'([a-zA-Z.]+)'");
  return pattern.allMatches(source).map((m) => m.group(1)!).toSet();
}

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
    editMutate:
        armPattern.allMatches(mutateSource).map((m) => m.group(1)!).toSet(),
    pageOp: armPattern.allMatches(pageSource).map((m) => m.group(1)!).toSet(),
  );
}

void main() {
  // ── EngineOp enum integrity ──

  group('EngineOp enum', () {
    test('every value has a non-empty camelCase wire name', () {
      for (final op in EngineOp.values) {
        expect(op.wire, isNotEmpty, reason: '${op.name} has empty wire');
        expect(op.wire, matches(RegExp(r'^[a-zA-Z]+$')),
            reason: '${op.wire} contains non-alpha chars');
      }
    });

    test('no duplicate wire names', () {
      final seen = <String>{};
      for (final op in EngineOp.values) {
        expect(seen.add(op.wire), isTrue,
            reason: 'Duplicate wire name: ${op.wire}');
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
      // These EngineOps are dispatched as sub-commands inside editorMutate
      // or builderPageOp — they won't appear in the top-level match.
      // Instead of hardcoding which ones, we dynamically know: if the op's
      // wire name appears in editMutate or pageOp sets, it's sub-dispatched.
      final subDispatched = rustEditMutate.union(rustPageOp);

      for (final op in EngineOp.values) {
        if (subDispatched.contains(op.wire)) continue;
        expect(rustTopLevel, contains(op.wire),
            reason:
                'EngineOp.${op.name} (${op.wire}) missing from top-level dispatch');
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

      expect(orphans, isEmpty,
          reason:
              'Rust has cases not in EngineOp and not in any sub-dispatch: $orphans');
    });

    test('edit sub-dispatch and page sub-dispatch are disjoint', () {
      final overlap = rustEditMutate.intersection(rustPageOp);
      // 'watermark' legitimately appears in both (editor watermark + builder page watermark)
      final realOverlap = overlap.difference({'watermark'});
      expect(realOverlap, isEmpty,
          reason: 'Unexpected overlap between edit and page ops: $realOverlap');
    });

    test('sub-dispatch ops are NOT duplicated in top-level', () {
      final editInTop = rustTopLevel.intersection(rustEditMutate);
      final pageInTop = rustTopLevel.intersection(rustPageOp);
      // Some ops like 'watermark' may appear at top-level too (different handler)
      // but pure sub-ops should not appear at top-level
      expect(editInTop, isEmpty,
          reason:
              'Edit sub-ops leaked into top-level dispatch: $editInTop');
      expect(pageInTop, isEmpty,
          reason:
              'Page sub-ops leaked into top-level dispatch: $pageInTop');
    });
  });

  // ── Web worker.js sync ──

  group('worker.js ↔ unified bridge', () {
    late String workerSource;

    setUpAll(() {
      final file = File('web_assets/worker.js');
      if (!file.existsSync()) fail('worker.js not found');
      workerSource = file.readAsStringSync();
    });

    test('worker calls bridge_execute', () {
      expect(workerSource, contains('bridge_execute'),
          reason: 'worker.js must call bridge_execute from pdf_oxide.js');
    });

    test('worker has no per-op dispatch switch', () {
      final perOpPattern =
          RegExp(r"case\s+'(open|extract|search|render|sign)'");
      expect(perOpPattern.hasMatch(workerSource), isFalse,
          reason: 'worker.js should not have per-op cases — all go through bridge_execute');
    });
  });

  // ── Coordinator.js sync ──

  group('coordinator.js message types', () {
    late Set<String> coordCases;

    setUpAll(() {
      final file = File('web_assets/coordinator.js');
      if (!file.existsSync()) fail('coordinator.js not found');
      coordCases = _extractJsCases(file.readAsStringSync());
    });

    test('coordinator handles all required message types', () {
      const required = {
        'readAt', 'chunk', 'result', 'error',
        'init', 'readAtResponse', 'submit', 'cancel', 'dispose',
        'opfs.write', 'opfs.finalize',
      };
      for (final msg in required) {
        expect(coordCases, contains(msg), reason: 'Missing: $msg');
      }
    });
  });
}
