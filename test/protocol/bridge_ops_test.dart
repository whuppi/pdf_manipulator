// Tests for shared EngineRequest builder functions.
// Verifies every builder returns the correct EngineOp and expected arg keys.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/protocol/bridge_ops.dart';
import 'package:pdf_manipulator/src/protocol/op.dart';
import 'package:test/test.dart';

void main() {
  // ── Inspect ──

  group('openOp', () {
    test('has correct op', () {
      expect(openOp().op, EngineOp.open);
    });

    test('password is null by default', () {
      expect(openOp().args['password'], isNull);
    });

    test('password is set when provided', () {
      expect(openOp(password: 'secret').args['password'], 'secret');
    });
  });

  // ── Structural ──

  group('structural ops', () {
    test('mergeOp', () {
      final req = mergeOp();
      expect(req.op, EngineOp.merge);
      expect(req.args.containsKey('password'), isTrue);
    });

    test('mergeOp with secondaries', () {
      final buf = Uint8List(10).buffer;
      final req = mergeOp(secondaries: [buf]);
      expect(req.args['secondaries'], hasLength(1));
    });

    test('extractPagesOp', () {
      final req = extractPagesOp(pages: [0, 1, 2]);
      expect(req.op, EngineOp.extractPages);
      expect(req.args['pages'], [0, 1, 2]);
    });

    test('deletePagesOp', () {
      final req = deletePagesOp(pages: [3]);
      expect(req.op, EngineOp.deletePages);
      expect(req.args['pages'], [3]);
    });

    test('reorderPagesOp', () {
      final req = reorderPagesOp(order: [2, 0, 1]);
      expect(req.op, EngineOp.reorderPages);
      expect(req.args['order'], [2, 0, 1]);
    });

    test('movePageOp', () {
      final req = movePageOp(from: 0, to: 5);
      expect(req.op, EngineOp.movePage);
      expect(req.args['from'], 0);
      expect(req.args['to'], 5);
    });

    test('rotatePagesOp', () {
      final req = rotatePagesOp(rotations: {0: 90, 1: 180});
      expect(req.op, EngineOp.rotatePages);
      expect(req.args['rotations'], {0: 90, 1: 180});
    });

    test('rotateAllPagesOp', () {
      final req = rotateAllPagesOp(degrees: 270);
      expect(req.op, EngineOp.rotateAllPages);
      expect(req.args['degrees'], 270);
    });
  });

  // ── Content ──

  group('content ops', () {
    test('flattenFormsOp', () {
      expect(flattenFormsOp().op, EngineOp.flattenForms);
      expect(flattenFormsOp().args, isEmpty);
    });

    test('applyRedactionsOp', () {
      expect(applyRedactionsOp().op, EngineOp.applyRedactions);
    });

    test('embedFileOp', () {
      final req = embedFileOp(name: 'test.txt', fileData: Uint8List(5));
      expect(req.op, EngineOp.embedFile);
      expect(req.args['name'], 'test.txt');
      expect(req.args['fileData'], isA<Uint8List>());
    });

    test('eraseRegionsOp', () {
      final req = eraseRegionsOp(
        page: 0,
        regions: [const PdfRect(x: 10, y: 20, width: 100, height: 50)],
      );
      expect(req.op, EngineOp.eraseRegions);
      expect(req.args['page'], 0);
      expect((req.args['regions'] as List), hasLength(1));
    });

    test('compressOp with defaults', () {
      final req = compressOp();
      expect(req.op, EngineOp.compress);
      expect(req.args['imageQuality'], 75);
      expect(req.args['garbageCollect'], isTrue);
      expect(req.args['linearize'], isFalse);
    });

    test('compressOp with custom values', () {
      final req = compressOp(imageQuality: 50, garbageCollect: false, linearize: true);
      expect(req.args['imageQuality'], 50);
      expect(req.args['garbageCollect'], isFalse);
      expect(req.args['linearize'], isTrue);
    });
  });

  // ── Extraction ──

  group('extraction ops', () {
    test('extractOp with page', () {
      final req = extractOp(format: PdfExtractionFormat.markdown, page: 0);
      expect(req.op, EngineOp.extract);
      expect(req.args['format'], 'markdown');
      expect(req.args['page'], 0);
    });

    test('extractOp without page', () {
      final req = extractOp(format: PdfExtractionFormat.plainText);
      expect(req.args.containsKey('page'), isFalse);
    });

    test('searchOp', () {
      final req = searchOp(query: 'hello', page: 2);
      expect(req.op, EngineOp.search);
      expect(req.args['query'], 'hello');
      expect(req.args['page'], 2);
    });
  });

  // ── Security ──

  group('security ops', () {
    test('watermarkOp', () {
      final req = watermarkOp(text: 'DRAFT');
      expect(req.op, EngineOp.watermark);
      expect(req.args['text'], 'DRAFT');
      expect(req.args.containsKey('opacity'), isTrue);
    });

    test('encryptOp', () {
      const cfg = PdfEncryptionConfig(ownerPassword: 'o', userPassword: 'u');
      final req = encryptOp(encryption: cfg);
      expect(req.op, EngineOp.encrypt);
      expect(req.args['ownerPassword'], 'o');
      expect(req.args['userPassword'], 'u');
    });

    test('decryptOp', () {
      final req = decryptOp(password: 'pw');
      expect(req.op, EngineOp.decrypt);
      expect(req.args['password'], 'pw');
    });

    test('signOp with PKCS12', () {
      final cert = Uint8List.fromList([1, 2, 3]);
      final req = signOp(
        credentials: PdfSigningCredentials.pkcs12(cert, 'p'),
        reason: 'test',
        location: 'here',
      );
      expect(req.op, EngineOp.sign);
      expect(req.args['certificate'], cert);
      expect(req.args['certificatePassword'], 'p');
      expect(req.args['reason'], 'test');
      expect(req.args['location'], 'here');
    });

    test('signOp with PEM', () {
      final req = signOp(
        credentials: const PdfSigningCredentials.pem('CERT', 'KEY'),
        reason: 'r',
      );
      expect(req.op, EngineOp.sign);
      expect(req.args['certPem'], 'CERT');
      expect(req.args['keyPem'], 'KEY');
      expect(req.args['reason'], 'r');
      expect(req.args.containsKey('certificate'), isFalse);
    });
  });

  // ── Stamps ──

  group('stamp ops', () {
    test('addStampOp', () {
      final req = addStampOp(
        page: 0,
        type: PdfStampType.approved,
        rect: const PdfRect(x: 10, y: 20, width: 100, height: 50),
      );
      expect(req.op, EngineOp.addStamp);
      expect(req.args['page'], 0);
      expect(req.args.containsKey('stampType'), isTrue);
      expect(req.args.containsKey('x'), isTrue);
    });

    test('addImageStampOp', () {
      final req = addImageStampOp(
        page: 1,
        imageBytes: Uint8List(5),
        rect: const PdfRect(x: 0, y: 0, width: 50, height: 50),
      );
      expect(req.op, EngineOp.addImageStamp);
      expect(req.args['imageBytes'], isA<Uint8List>());
    });
  });

  // ── Creation ──

  group('creation ops', () {
    test('imagesToPdfOp', () {
      final images = [Uint8List(10), Uint8List(20)];
      final req = imagesToPdfOp(images: images);
      expect(req.op, EngineOp.imagesToPdf);
      expect((req.args['images'] as List), hasLength(2));
    });
  });

  // ── Streaming ──

  group('streaming ops', () {
    test('renderOp with size', () {
      final req = renderOp(pageIndices: [0, 1], maxWidth: 800, maxHeight: 600);
      expect(req.op, EngineOp.render);
      expect(req.args['pageIndices'], [0, 1]);
      expect(req.args['maxWidth'], 800);
      expect(req.args['maxHeight'], 600);
    });

    test('renderOp without size', () {
      final req = renderOp(pageIndices: [0]);
      expect(req.args.containsKey('maxWidth'), isFalse);
      expect(req.args.containsKey('maxHeight'), isFalse);
    });

    test('extractImagesOp', () {
      final req = extractImagesOp(pageIndices: [0, 2, 4]);
      expect(req.op, EngineOp.extractImages);
      expect(req.args['pageIndices'], [0, 2, 4]);
    });
  });

  // ── Signatures / Validation ──

  group('signature and validation ops', () {
    test('getSignaturesOp', () {
      expect(getSignaturesOp().op, EngineOp.getSignatures);
    });

    test('verifySignaturesOp', () {
      expect(verifySignaturesOp().op, EngineOp.verifySignatures);
    });

    test('validatePdfAOp', () {
      final req = validatePdfAOp(level: 3);
      expect(req.op, EngineOp.validatePdfA);
      expect(req.args['level'], 3);
    });

    test('validatePdfUaOp', () {
      final req = validatePdfUaOp(level: 2);
      expect(req.op, EngineOp.validatePdfUa);
      expect(req.args['level'], 2);
    });
  });

  // ── Editor handle ops ──

  group('editor handle ops', () {
    test('editorOpenOp', () {
      final req = editorOpenOp(password: 'pw');
      expect(req.op, EngineOp.editorOpen);
      expect(req.args['password'], 'pw');
    });

    test('editorDisposeOp', () {
      final req = editorDisposeOp(handleId: 42);
      expect(req.op, EngineOp.editorDispose);
      expect(req.args['handleId'], 42);
    });

    test('editorMutateOp', () {
      final req = editorMutateOp(handleId: 1, editOp: 'setTitle', extra: {'value': 'Test'});
      expect(req.op, EngineOp.editorMutate);
      expect(req.args['handleId'], 1);
      expect(req.args['editOp'], 'setTitle');
      expect(req.args['value'], 'Test');
    });

    test('editorSaveOp with default options', () {
      final req = editorSaveOp(handleId: 1);
      expect(req.op, EngineOp.editorSave);
      expect(req.args['compress'], isTrue);
    });

    test('editorGetMetadataOp', () {
      final req = editorGetMetadataOp(handleId: 7);
      expect(req.op, EngineOp.editorGetMetadata);
      expect(req.args['handleId'], 7);
    });

    test('editorPageMediaBoxOp', () {
      final req = editorPageMediaBoxOp(handleId: 1, page: 3);
      expect(req.op, EngineOp.editorPageMediaBox);
      expect(req.args['page'], 3);
    });

    test('editorExtractPagesOp', () {
      final req = editorExtractPagesOp(handleId: 1, pages: [0, 2]);
      expect(req.op, EngineOp.editorExtractPages);
      expect(req.args['pages'], [0, 2]);
    });

    test('editorMergeFromOp', () {
      final req = editorMergeFromOp(handleId: 1, otherBytes: Uint8List(10));
      expect(req.op, EngineOp.editorMergeFrom);
      expect(req.args['otherBytes'], isA<Uint8List>());
    });
  });

  // ── Builder handle ops ──

  group('builder handle ops', () {
    test('builderCreateOp', () {
      expect(builderCreateOp().op, EngineOp.builderCreate);
      expect(builderCreateOp().args, isEmpty);
    });

    test('builderDisposeOp', () {
      final req = builderDisposeOp(handleId: 5);
      expect(req.args['handleId'], 5);
    });

    test('builderSetMetadataOp with all fields', () {
      final req = builderSetMetadataOp(
        handleId: 1,
        title: 'T',
        author: 'A',
        subject: 'S',
        keywords: 'K',
      );
      expect(req.op, EngineOp.builderSetMetadata);
      expect(req.args['title'], 'T');
      expect(req.args['author'], 'A');
      expect(req.args['subject'], 'S');
      expect(req.args['keywords'], 'K');
    });

    test('builderSetMetadataOp with partial fields', () {
      final req = builderSetMetadataOp(handleId: 1, title: 'Only Title');
      expect(req.args['title'], 'Only Title');
      expect(req.args.containsKey('author'), isFalse);
    });

    test('builderAddPageOp a4', () {
      final req = builderAddPageOp(handleId: 1, pageType: 'a4');
      expect(req.op, EngineOp.builderAddPage);
      expect(req.args['pageType'], 'a4');
    });

    test('builderAddPageOp custom size', () {
      final req = builderAddPageOp(handleId: 1, width: 500, height: 700);
      expect(req.args['width'], 500);
      expect(req.args['height'], 700);
    });

    test('builderPageOpReq', () {
      final req = builderPageOpReq(handleId: 3, pageOp: 'text', extra: {'text': 'Hello'});
      expect(req.op, EngineOp.builderPageOp);
      expect(req.args['pageOp'], 'text');
      expect(req.args['text'], 'Hello');
    });

    test('builderPageDoneOp', () {
      final req = builderPageDoneOp(handleId: 3);
      expect(req.op, EngineOp.builderPageDone);
    });

    test('builderSaveOp', () {
      final req = builderSaveOp(handleId: 1);
      expect(req.op, EngineOp.builderSave);
    });
  });
}
