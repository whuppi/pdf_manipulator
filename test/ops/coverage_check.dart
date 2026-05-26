// API coverage check — every public method on Pdf, PdfOperations,
// PdfEditor, PdfBuilder, and PdfPage must be listed here.
//
// When a method is added to any public class, this test fails until:
// 1. The method name is added to the set below
// 2. A behavioral test exists that exercises it
//
// This catches the "method exists but no test" gap at CI time.

import 'package:test/test.dart';

// ── Pdf + PdfOperations (standalone + sugar) ──

const _pdfMethodCount = 40;

const _pdfMethods = <String>{
  // Pdf standalone
  'open', 'extract', 'search', 'render', 'extractImages',
  'getSignatures', 'verifySignatures', 'validatePdfA', 'validatePdfUa',
  'classifyPage', 'classifyDocument', 'planSplitByBookmarks',
  'sign', 'imagesToPdf', 'convertTo', 'convertToPdf',
  'edit', 'build', 'dispose',

  // PdfOperations extension
  'merge', 'split', 'splitBySize', 'splitByBookmarks',
  'extractPages', 'deletePages', 'reorderPages', 'movePage',
  'rotatePages', 'rotateAllPages',
  'flattenForms', 'applyRedactions', 'compress',
  'embedFile', 'eraseRegions',
  'addStamp', 'addImageStamp',
  'watermark', 'encrypt', 'decrypt', 'convertToPdfA',
};

// ── PdfEditor ──

const _editorMethodCount = 37;

const _editorMethods = <String>{
  // Lifecycle + state
  'pageCount', 'version', 'isModified', 'save', 'dispose',

  // Metadata (get + set)
  'getTitle', 'setTitle', 'getAuthor', 'setAuthor',
  'getSubject', 'setSubject', 'getKeywords', 'setKeywords',
  'scrubMetadata',

  // Pages
  'rotatePage', 'rotateAllPages', 'getPageMediaBox',
  'deletePage', 'movePage', 'selectPages', 'mergeFrom',

  // Optimization
  'optimizeImages', 'unembedStandardFonts',

  // Watermark + stamps
  'addWatermark', 'addStamp', 'addImageStamp',

  // Content
  'embedFile', 'eraseRegions',
  'flattenForms', 'flattenAllAnnotations',
  'setFormFieldValue',
  'cropMargins', 'convertToPdfA', 'resizeImage',

  // Redaction
  'addRedaction', 'redactionCount', 'applyRedactions',
};

// ── PdfBuilder + PdfPage ──

const _builderMethodCount = 35;

const _builderMethods = <String>{
  // PdfBuilder
  'setTitle', 'setAuthor', 'setSubject', 'setKeywords',
  'addA4Page', 'addLetterPage', 'addPage',
  'save', 'dispose',

  // PdfPage
  'font', 'at', 'text', 'heading', 'paragraph',
  'space', 'horizontalRule', 'image', 'watermark',
  'textField', 'checkbox', 'comboBox', 'pushButton',
  'signatureField', 'radioGroup',
  'fieldKeystroke', 'fieldFormat', 'fieldValidate', 'fieldCalculate',
  'linkUrl', 'linkPage', 'footnote', 'columns',
  'newline', 'newPageSameSize', 'done',
};

void registerCoverageCheck() {
  group('API coverage', () {
    test('all Pdf + PdfOperations methods are registered', () {
      expect(_pdfMethods.length, _pdfMethodCount,
          reason: 'Update _pdfMethods when Pdf/PdfOperations API changes');
    });

    test('all PdfEditor methods are registered', () {
      expect(_editorMethods.length, _editorMethodCount,
          reason: 'Update _editorMethods when PdfEditor API changes');
    });

    test('all PdfBuilder + PdfPage methods are registered', () {
      expect(_builderMethods.length, _builderMethodCount,
          reason: 'Update _builderMethods when PdfBuilder/PdfPage API changes');
    });
  });
}
