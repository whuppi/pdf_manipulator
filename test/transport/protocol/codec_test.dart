// Tests for the shared protocol codec.
// Verifies request encoding and response decoding — the shared contract
// that both native and web bridges depend on.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/transport/protocol/codec.dart';
import 'package:pdf_manipulator/src/transport/protocol/op.dart';
import 'package:test/test.dart';

void main() {
  // ════════════════════════════════════════════════════
  // REQUEST ENCODING
  // ════════════════════════════════════════════════════

  group('request encoding', () {
    test('openOp', () {
      expect(openOp().op, EngineOp.open);
      expect(openOp(password: 'pw').args['password'], 'pw');
    });

    test('extractOp', () {
      final req = extractOp(format: PdfExtractionFormat.markdown, page: 0);
      expect(req.op, EngineOp.extract);
      expect(req.args['format'], 'markdown');
      expect(req.args['page'], 0);
    });

    test('searchOp', () {
      final req = searchOp(query: 'hello', page: 2);
      expect(req.op, EngineOp.search);
      expect(req.args['query'], 'hello');
    });

    test('signOp with PKCS12', () {
      final cert = Uint8List.fromList([1, 2, 3]);
      final req = signOp(
        credentials: PdfSigningCredentials.pkcs12(cert, 'p'),
        reason: 'test',
      );
      expect(req.op, EngineOp.sign);
      expect(req.args['certificate'], cert);
      expect(req.args['certificatePassword'], 'p');
    });

    test('signOp with PEM', () {
      final req = signOp(
        credentials: const PdfSigningCredentials.pem('CERT', 'KEY'),
      );
      expect(req.args['certPem'], 'CERT');
      expect(req.args['keyPem'], 'KEY');
    });

    test('convertToOp', () {
      final req = convertToOp(format: PdfDocumentFormat.docx);
      expect(req.op, EngineOp.convertTo);
      expect(req.args['format'], 'docx');
    });

    test('renderOp', () {
      final req = renderOp(pageIndices: [0, 1], maxWidth: 800);
      expect(req.op, EngineOp.render);
      expect(req.args['pageIndices'], [0, 1]);
      expect(req.args['maxWidth'], 800);
    });

    test('editorSaveOp encodes encryption', () {
      final req = editorSaveOp(handleId: 1, options: const PdfSaveOptions.fullRewrite(
        encryption: PdfEncryption.config(ownerPassword: 'ow'),
      ));
      expect(req.args['encryptMode'], 2);
      expect(req.args['encryptOwnerPw'], 'ow');
    });

    test('editorMutateOp', () {
      final req = editorMutateOp(handleId: 1, editOp: 'setTitle', extra: {'value': 'T'});
      expect(req.args['editOp'], 'setTitle');
      expect(req.args['value'], 'T');
    });

    test('builderSetMetadataOp partial', () {
      final req = builderSetMetadataOp(handleId: 1, title: 'Only Title');
      expect(req.args['title'], 'Only Title');
      expect(req.args.containsKey('author'), isFalse);
    });
  });

  // ════════════════════════════════════════════════════
  // RESPONSE DECODING
  // ════════════════════════════════════════════════════

  group('decodePageList', () {
    test('parses complete page list', () {
      final pages = decodePageList({
        'pages': [
          {'index': 0, 'width': 612.0, 'height': 792.0, 'rotation': 0},
          {'index': 1, 'width': 612.0, 'height': 792.0, 'rotation': 90},
          {'index': 2, 'width': 842.0, 'height': 595.0, 'rotation': 0},
        ],
      });
      expect(pages, hasLength(3));
      expect(pages[1].rotation, 90);
      expect(pages[2].width, 842.0);
    });

    test('handles missing pages key', () {
      final pages = decodePageList({});
      expect(pages, isEmpty);
    });
  });

  group('decodeEncryptionAlgorithm', () {
    test('parses known algorithms', () {
      expect(decodeEncryptionAlgorithm(1), PdfEncryptionAlgorithm.rc4_40);
      expect(decodeEncryptionAlgorithm(4), PdfEncryptionAlgorithm.aes256);
      expect(decodeEncryptionAlgorithm(0), isNull);
      expect(decodeEncryptionAlgorithm(99), isNull);
    });
  });

  group('decodePermissions', () {
    test('parses permission bits', () {
      final perms = decodePermissions(0xFF);
      expect(perms.print, isTrue);
      expect(perms.copy, isTrue);
    });

    test('zero bits means all false', () {
      final perms = decodePermissions(0);
      expect(perms.print, isFalse);
      expect(perms.copy, isFalse);
    });
  });

  group('decodeSearchResults', () {
    test('parses hits', () {
      final results = decodeSearchResults({
        'hits': [
          {'page': 0, 'text': 'hello', 'x': 72.0, 'y': 700.0, 'width': 50.0, 'height': 12.0},
        ],
      });
      expect(results, hasLength(1));
      expect(results[0].text, 'hello');
      expect(results[0].rect.x, 72.0);
    });

    test('handles empty', () {
      expect(decodeSearchResults({}), isEmpty);
    });
  });

  group('decodeSignatures', () {
    test('parses signature list', () {
      final sigs = decodeSignatures({
        'signatures': [
          {'signerName': 'Alice', 'reason': 'Approval', 'isValid': true},
        ],
      });
      expect(sigs, hasLength(1));
      expect(sigs[0].signerName, 'Alice');
      expect(sigs[0].isValid, isTrue);
    });
  });

  group('decodeValidationResult', () {
    test('parses compliant', () {
      final r = decodeValidationResult({'compliant': true, 'errors': 0, 'warnings': 2});
      expect(r.compliant, isTrue);
      expect(r.warnings, 2);
    });
  });

  group('decodeRenderedPage', () {
    test('parses with ByteBuffer', () {
      final bytes = Uint8List.fromList([1, 2, 3]);
      final page = decodeRenderedPage({'width': 800, 'height': 600, 'data': bytes.buffer});
      expect(page.width, 800);
      expect(page.data, hasLength(3));
    });
  });

  group('decodePdfImage', () {
    test('parses full image', () {
      final img = decodePdfImage({
        'width': 100, 'height': 200, 'format': 'jpeg',
        'colorSpace': 'RGB', 'bitsPerComponent': 8,
        'data': Uint8List(5),
      });
      expect(img.format, 'jpeg');
    });
  });

  group('decodeEditorMetadata', () {
    test('parses all fields', () {
      final m = decodeEditorMetadata({
        'pageCount': 5, 'version': '1.4', 'title': 'My Doc',
        'author': 'DC', 'subject': 'Sub', 'keywords': 'kw',
      });
      expect(m.pageCount, 5);
      expect(m.title, 'My Doc');
    });
  });

  group('decodeMediaBox', () {
    test('parses rect', () {
      final r = decodeMediaBox({'x': 0.0, 'y': 0.0, 'width': 612.0, 'height': 792.0});
      expect(r.width, 612.0);
    });
  });

  // ════════════════════════════════════════════════════
  // HELPERS
  // ════════════════════════════════════════════════════

  group('helpers', () {
    test('resolvePageIndices all', () {
      expect(resolvePageIndices(const PdfPages.all(), 3), [0, 1, 2]);
    });

    test('resolvePageIndices range', () {
      expect(resolvePageIndices(const PdfPages.range(2, 5), 10), [2, 3, 4]);
    });

    test('encodeRegions', () {
      final r = encodeRegions([const PdfRect(x: 1, y: 2, width: 3, height: 4)]);
      expect(r, [1.0, 2.0, 3.0, 4.0]);
    });

    test('encodeWatermarkArgs includes style + position + layer', () {
      final args = encodeWatermarkArgs(
        'DRAFT',
        const PdfWatermarkStyle(),
        const PdfWatermarkPosition.center(),
        PdfWatermarkLayer.foreground,
      );
      expect(args['text'], 'DRAFT');
      expect(args['opacity'], 0.3);
      expect(args['fontSize'], 48);
      expect(args['rotation'], 45);
      expect(args['layer'], 0);
      expect(args['posType'], 0);
    });

    test('encodeWatermarkArgs corner position includes corner + margins', () {
      final args = encodeWatermarkArgs(
        'X',
        const PdfWatermarkStyle(),
        const PdfWatermarkPosition.corner(PdfCorner.topRight, marginX: 10, marginY: 15),
        PdfWatermarkLayer.background,
      );
      expect(args['posType'], 1);
      expect(args['corner'], 1); // topRight index
      expect(args['marginX'], 10);
      expect(args['marginY'], 15);
      expect(args['layer'], 1); // background
    });

    test('encodeWatermarkArgs tiled position includes columns + rows', () {
      final args = encodeWatermarkArgs(
        'T',
        const PdfWatermarkStyle(),
        const PdfWatermarkPosition.tiled(columns: 5, rows: 6),
        PdfWatermarkLayer.foreground,
      );
      expect(args['posType'], 2);
      expect(args['columns'], 5);
      expect(args['rows'], 6);
    });

    test('encodeWatermarkArgs exact position includes coordinates', () {
      final args = encodeWatermarkArgs(
        'E',
        const PdfWatermarkStyle(),
        const PdfWatermarkPosition.exact(x: 10, y: 20, width: 300, height: 50),
        PdfWatermarkLayer.foreground,
      );
      expect(args['posType'], 3);
      expect(args['posX'], 10);
      expect(args['posY'], 20);
      expect(args['posW'], 300);
      expect(args['posH'], 50);
    });
  });
}
