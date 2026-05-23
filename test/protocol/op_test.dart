// Tests for the shared EngineOp enum + helper functions.
// Pure unit tests — no bridge, no FFI, no WASM.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/protocol/op.dart';
import 'package:test/test.dart';

void main() {
  group('EngineOp enum', () {
    test('every value has a non-empty wire name', () {
      for (final op in EngineOp.values) {
        expect(op.wire, isNotEmpty, reason: '${op.name} has empty wire');
      }
    });

    test('no two values share the same wire name', () {
      final seen = <String>{};
      for (final op in EngineOp.values) {
        expect(seen.add(op.wire), isTrue,
            reason: 'Duplicate wire name: ${op.wire}');
      }
    });

    test('wire names are camelCase (no underscores, no spaces)', () {
      for (final op in EngineOp.values) {
        expect(op.wire, matches(RegExp(r'^[a-zA-Z]+$')),
            reason: '${op.wire} contains non-alpha chars');
      }
    });

    test('has all 43 expected operations', () {
      expect(EngineOp.values.length, 43);
    });

    test('contains every structural op', () {
      final structural = [
        'merge', 'extractPages', 'deletePages', 'reorderPages',
        'movePage', 'rotatePages', 'rotateAllPages',
      ];
      for (final name in structural) {
        expect(
          EngineOp.values.any((op) => op.wire == name),
          isTrue,
          reason: 'Missing structural op: $name',
        );
      }
    });

    test('contains every editor handle op', () {
      final editorOps = [
        'editorOpen', 'editorDispose', 'editorMutate', 'editorSave',
        'editorGetMetadata', 'editorPageMediaBox', 'editorExtractPages',
        'editorMergeFrom',
      ];
      for (final name in editorOps) {
        expect(
          EngineOp.values.any((op) => op.wire == name),
          isTrue,
          reason: 'Missing editor op: $name',
        );
      }
    });

    test('contains every builder handle op', () {
      final builderOps = [
        'builderCreate', 'builderDispose', 'builderSetMetadata',
        'builderAddPage', 'builderPageOp', 'builderPageDone', 'builderSave',
      ];
      for (final name in builderOps) {
        expect(
          EngineOp.values.any((op) => op.wire == name),
          isTrue,
          reason: 'Missing builder op: $name',
        );
      }
    });
  });

  group('resolvePageIndices', () {
    test('PdfPages.all() generates 0..pageCount-1', () {
      expect(resolvePageIndices(const PdfPages.all(), 5), [0, 1, 2, 3, 4]);
    });

    test('PdfPages.all() with 0 pages returns empty', () {
      expect(resolvePageIndices(const PdfPages.all(), 0), isEmpty);
    });

    test('PdfPages.single returns one-element list', () {
      expect(resolvePageIndices(const PdfPages.single(3), 10), [3]);
    });

    test('PdfPages.list passes through', () {
      expect(resolvePageIndices(const PdfPages.list([0, 5, 9]), 10), [0, 5, 9]);
    });

    test('PdfPages.range generates start..end-1', () {
      expect(resolvePageIndices(const PdfPages.range(2, 6), 10), [2, 3, 4, 5]);
    });

    test('PdfPages.range with equal start/end returns empty', () {
      expect(resolvePageIndices(const PdfPages.range(3, 3), 10), isEmpty);
    });
  });

  group('encodeRegions', () {
    test('encodes list of PdfRect to list of maps', () {
      final regions = [
        const PdfRect(x: 10, y: 20, width: 100, height: 50),
        const PdfRect(x: 0, y: 0, width: 50, height: 50),
      ];
      final encoded = encodeRegions(regions);
      expect(encoded, hasLength(2));
      expect(encoded[0], {'x': 10.0, 'y': 20.0, 'width': 100.0, 'height': 50.0});
      expect(encoded[1], {'x': 0.0, 'y': 0.0, 'width': 50.0, 'height': 50.0});
    });

    test('empty list returns empty', () {
      expect(encodeRegions([]), isEmpty);
    });
  });

  group('encodeWatermarkArgs', () {
    test('includes all watermark fields', () {
      final args = encodeWatermarkArgs('DRAFT', const PdfWatermarkStyle());
      expect(args['text'], 'DRAFT');
      expect(args.containsKey('opacity'), isTrue);
      expect(args.containsKey('fontSize'), isTrue);
      expect(args.containsKey('rotation'), isTrue);
      expect(args.containsKey('r'), isTrue);
      expect(args.containsKey('g'), isTrue);
      expect(args.containsKey('b'), isTrue);
    });

    test('uses provided style values', () {
      final style = PdfWatermarkStyle(
        opacity: 0.5,
        fontSize: 72,
        rotation: 30,
        color: const PdfColor(1.0, 0.0, 0.0),
      );
      final args = encodeWatermarkArgs('SECRET', style);
      expect(args['opacity'], 0.5);
      expect(args['fontSize'], 72);
      expect(args['rotation'], 30);
      expect(args['r'], 1.0);
      expect(args['g'], 0.0);
      expect(args['b'], 0.0);
    });
  });

  group('encodeRectArgs', () {
    test('encodes all four rect fields', () {
      final args = encodeRectArgs(const PdfRect(x: 1, y: 2, width: 3, height: 4));
      expect(args, {'x': 1.0, 'y': 2.0, 'width': 3.0, 'height': 4.0});
    });
  });

  group('encodeSaveArgs', () {
    test('encodes default save options', () {
      final args = encodeSaveArgs(const PdfSaveOptions());
      expect(args['compress'], isTrue);
      expect(args['garbageCollect'], isTrue);
      expect(args['linearize'], isFalse);
    });

    test('encodes custom save options', () {
      const opts = PdfSaveOptions(compress: false, garbageCollect: false, linearize: true);
      final args = encodeSaveArgs(opts);
      expect(args['compress'], isFalse);
      expect(args['garbageCollect'], isFalse);
      expect(args['linearize'], isTrue);
    });
  });

  group('encodeExtractionFormat', () {
    test('markdown', () {
      expect(encodeExtractionFormat(PdfExtractionFormat.markdown), 'markdown');
    });

    test('html', () {
      expect(encodeExtractionFormat(PdfExtractionFormat.html), 'html');
    });

    test('plainText', () {
      expect(encodeExtractionFormat(PdfExtractionFormat.plainText), 'plainText');
    });

    test('auto falls back to plainText', () {
      expect(encodeExtractionFormat(PdfExtractionFormat.auto), 'plainText');
    });
  });
}
