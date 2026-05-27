// Cross-verification: EngineOp wire names vs both platform dispatchers.
//
// Reads coordinator.dart and worker.js from disk, extracts every
// case 'xxx': string, and checks parity with the EngineOp enum.
//
// If an op exists in one layer but not the others, this test fails.
// This is the guard against the native/web parity drift that caused
// isEncrypted to be hardcoded false on web.

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:test/test.dart';

Set<String> _extractCases(String source) {
  final pattern = RegExp(r"case\s+'([a-zA-Z.]+)'");
  return pattern.allMatches(source).map((m) => m.group(1)!).toSet();
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

  // ── Native worker sync ──

  group('native coordinator.dart ↔ EngineOp', () {
    late Set<String> nativeCases;

    setUpAll(() {
      final file = File('lib/src/transport/native/coordinator.dart');
      if (!file.existsSync()) fail('coordinator.dart not found');
      nativeCases = _extractCases(file.readAsStringSync());
    });

    test('native worker has a case for every EngineOp', () {
      const subDispatchOps = {'editorIsModified', 'editorPageMediaBox'};

      for (final op in EngineOp.values) {
        if (subDispatchOps.contains(op.wire)) continue;
        expect(nativeCases, contains(op.wire),
            reason: 'EngineOp.${op.name} (${op.wire}) missing from coordinator.dart');
      }
    });

    test('native worker has no orphan cases', () {
      final dartWires = EngineOp.values.map((op) => op.wire).toSet();
      const nativeOnly = {'signPem'};

      final orphans = nativeCases.difference(dartWires).difference(nativeOnly);
      expect(orphans, isEmpty,
          reason: 'coordinator.dart has unknown cases: $orphans');
    });
  });

  // ── Web worker sync ──

  group('web worker.js ↔ EngineOp', () {
    late Set<String> jsCases;

    setUpAll(() {
      final file = File('web_assets/worker.js');
      if (!file.existsSync()) fail('worker.js not found');
      jsCases = _extractCases(file.readAsStringSync());
    });

    test('JS worker has a case for every EngineOp', () {
      // These ops are dispatched as sub-commands inside editorMutate or
      // builderPageOp — they exist in applyEditOp/applyPageOp, not top-level.
      const subDispatchOps = {
        'editorDispose', 'editorMutate', 'editorSave',
        'editorGetMetadata', 'editorIsModified', 'editorPageMediaBox',
        'editorRedactionCount', 'editorQuery', 'editorMergeFrom',
        'builderPageOp', 'builderPageDone',
      };

      for (final op in EngineOp.values) {
        if (subDispatchOps.contains(op.wire)) continue;
        expect(jsCases, contains(op.wire),
            reason: 'EngineOp.${op.name} (${op.wire}) missing from worker.js');
      }
    });

    test('JS worker has no orphan cases', () {
      final dartWires = EngineOp.values.map((op) => op.wire).toSet();

      // Cases that exist in JS sub-dispatch tables (applyEditOp, applyPageOp,
      // format sub-cases) or are worker lifecycle messages.
      const jsSubDispatch = {
        // applyEditOp mutations
        'selectPages', 'deletePages', 'reorderPages', 'movePage',
        'rotatePages', 'rotateAllPages', 'flattenForms', 'applyRedactions',
        'compress', 'embedFile', 'eraseRegions', 'watermark',
        'encrypt', 'decrypt', 'addStamp', 'addImageStamp',
        'setTitle', 'setAuthor', 'setSubject', 'setKeywords',
        'cropMargins', 'convertToPdfA', 'flattenAllAnnotations',
        'setFormFieldValue', 'unembedStandardFonts', 'resizeImage',
        'addRedaction', 'redactionCount', 'scrubMetadata', 'optimizeImages',
        // format sub-cases
        'docx', 'pptx', 'xlsx',
        // applyPageOp sub-cases
        'font', 'at', 'text', 'heading', 'paragraph', 'space',
        'horizontalRule', 'image', 'textField', 'checkbox', 'comboBox',
        'pushButton', 'signatureField', 'radioGroup',
        'fieldKeystroke', 'fieldFormat', 'fieldValidate', 'fieldCalculate',
        'linkUrl', 'linkPage', 'footnote', 'columns', 'newline', 'newPageSameSize',
        // extraction format sub-cases
        'markdown', 'html', 'plainText',
        // web-only ops
        'mergeFromReaders',
        // worker lifecycle
        'init', 'exec', 'readAtResponse',
      };

      final orphans = jsCases.difference(dartWires).difference(jsSubDispatch);
      expect(orphans, isEmpty,
          reason: 'worker.js has unknown cases: $orphans');
    });
  });

  // ── Coordinator sync ──

  group('coordinator.js message types', () {
    late Set<String> coordCases;

    setUpAll(() {
      final file = File('web_assets/coordinator.js');
      if (!file.existsSync()) fail('coordinator.js not found');
      coordCases = _extractCases(file.readAsStringSync());
    });

    test('coordinator handles all required message types', () {
      const required = {
        // From WASM worker
        'readAt', 'chunk', 'item', 'itemDone', 'result', 'error',
        // From main thread
        'init', 'readAtResponse', 'submit', 'cancel', 'dispose',
        // OPFS
        'opfs.write', 'opfs.finalize',
      };
      for (final msg in required) {
        expect(coordCases, contains(msg), reason: 'Missing: $msg');
      }
    });
  });
}
