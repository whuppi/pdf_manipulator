// Cross-verification: EngineOp wire names vs wasm_worker.js case names.
//
// Reads JS files from disk — requires dart:io, VM only.
//
// Run: dart test test/bridge/protocol/wire_sync_test.dart

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/protocol/op.dart';
import 'package:test/test.dart';

void main() {
  late String wasmWorkerSource;
  late Set<String> jsCaseNames;

  setUpAll(() {
    final file = File('web_assets/wasm_worker.js');
    if (!file.existsSync()) {
      fail('web_assets/wasm_worker.js not found — run from package root');
    }
    wasmWorkerSource = file.readAsStringSync();

    // Extract all case 'xxx': patterns from the JS file
    final casePattern = RegExp(r"case\s+'([a-zA-Z]+)'");
    jsCaseNames = casePattern
        .allMatches(wasmWorkerSource)
        .map((m) => m.group(1)!)
        .toSet();
  });

  // Ops that go through the coordinator to wasm_worker.js
  // Editor/builder handle ops are dispatched by the worker
  // These are the ops WebBridge._submit sends
  final webDispatchedOps = EngineOp.values.where((op) {
    final w = op.wire;
    // 'read' is native-only (generic dispatch with opCode)
    // All others should have JS cases
    return w != 'read';
  }).toList();

  group('EngineOp ↔ wasm_worker.js sync', () {
    for (final op in webDispatchedOps) {
      test('JS has case for EngineOp.${op.name} (wire: "${op.wire}")', () {
        // Some ops are dispatched as sub-operations (editorMutate dispatches
        // to applyEditOp which has its own case table). We check the top-level
        // dispatch AND the sub-dispatch tables.
        final found = jsCaseNames.contains(op.wire);
        if (!found) {
          // Check if it's an edit sub-op dispatched through editorMutate
          // or a page sub-op dispatched through builderPageOp
          // These are legitimate — the wire name goes as args.editOp or args.pageOp
          final isEditSubOp = op.wire.startsWith('editor') && op != EngineOp.editorOpen;
          final isPageSubOp = op == EngineOp.builderPageOp || op == EngineOp.builderPageDone;

          if (!isEditSubOp && !isPageSubOp) {
            fail('EngineOp.${op.name} (wire: "${op.wire}") has no matching '
                'case in wasm_worker.js. Add it to the dispatch.');
          }
        }
      });
    }

    test('JS has no orphan cases unknown to EngineOp', () {
      final dartWireNames = EngineOp.values.map((op) => op.wire).toSet();

      // Known JS-only cases (edit sub-ops dispatched within applyEditOp/applyPageOp)
      final jsOnlyKnown = {
        // Edit sub-ops dispatched via applyEditOp
        'setTitle', 'setAuthor', 'setSubject', 'setKeywords',
        'cropMargins', 'convertToPdfA', 'flattenAllAnnotations',
        'setFormFieldValue', 'unembedStandardFonts', 'resizeImage',
        'addRedaction', 'redactionCount', 'applyRedactions', 'scrubMetadata',
        // Format sub-cases inside convertTo/convertToPdf
        'docx', 'pptx', 'xlsx',
        // Page builder sub-ops dispatched via applyPageOp
        'font', 'at', 'text', 'heading', 'paragraph', 'space',
        'horizontalRule', 'image', 'watermark', 'textField',
        'checkbox', 'comboBox', 'pushButton', 'signatureField',
        'radioGroup', 'fieldKeystroke', 'fieldFormat', 'fieldValidate',
        'fieldCalculate', 'linkUrl', 'linkPage', 'footnote', 'columns',
        'newline', 'newPageSameSize',
        // Extraction format sub-cases (inside case 'extract')
        'markdown', 'html', 'plainText',
        // Web-only op (multi-reader merge — no native equivalent needed)
        'mergeFromReaders',
        // Worker lifecycle messages (not EngineOp)
        'init', 'exec', 'readAtResponse',
      };

      final orphans = jsCaseNames
          .difference(dartWireNames)
          .difference(jsOnlyKnown);

      expect(orphans, isEmpty,
          reason: 'JS has case names not in EngineOp or jsOnlyKnown: $orphans');
    });
  });

  group('coordinator.js message types', () {
    late String coordinatorSource;
    late Set<String> coordinatorCases;

    setUpAll(() {
      final file = File('web_assets/coordinator.js');
      if (!file.existsSync()) {
        fail('web_assets/coordinator.js not found');
      }
      coordinatorSource = file.readAsStringSync();
      final casePattern = RegExp(r"case\s+'([a-zA-Z.]+)'");
      coordinatorCases = casePattern
          .allMatches(coordinatorSource)
          .map((m) => m.group(1)!)
          .toSet();
    });

    test('coordinator handles readAt from WASM workers', () {
      expect(coordinatorCases, contains('readAt'));
    });

    test('coordinator handles chunk forwarding', () {
      expect(coordinatorCases, contains('chunk'));
    });

    test('coordinator handles item forwarding', () {
      expect(coordinatorCases, contains('item'));
    });

    test('coordinator handles itemDone forwarding', () {
      expect(coordinatorCases, contains('itemDone'));
    });

    test('coordinator handles result forwarding', () {
      expect(coordinatorCases, contains('result'));
    });

    test('coordinator handles error forwarding', () {
      expect(coordinatorCases, contains('error'));
    });

    test('coordinator handles init from main', () {
      expect(coordinatorCases, contains('init'));
    });

    test('coordinator handles readAtResponse from main', () {
      expect(coordinatorCases, contains('readAtResponse'));
    });

    test('coordinator handles submit from main', () {
      expect(coordinatorCases, contains('submit'));
    });

    test('coordinator handles cancel from main', () {
      expect(coordinatorCases, contains('cancel'));
    });

    test('coordinator handles dispose from main', () {
      expect(coordinatorCases, contains('dispose'));
    });

    test('coordinator handles OPFS write', () {
      expect(coordinatorCases, contains('opfs.write'));
    });

    test('coordinator handles OPFS finalize', () {
      expect(coordinatorCases, contains('opfs.finalize'));
    });
  });
}
