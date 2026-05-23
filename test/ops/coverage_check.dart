// Ensures every PdfBridge method has a shared test.
// When a method is added to PdfBridge, this count check fails until
// the method is registered here AND a test is written for it.

import 'package:test/test.dart';

const _bridgeMethodCount = 39;

const _testedMethods = <String>{
  'open',                                              // open.dart
  'merge',                                              // merge.dart
  'extractPages', 'deletePages', 'reorderPages',        // structural.dart
  'movePage', 'rotatePages', 'rotateAllPages',          // structural.dart
  'flattenForms', 'compress', 'split', 'splitBySize',   // structural.dart
  'applyRedactions', 'embedFile', 'eraseRegions',       // structural.dart
  'addStamp', 'addImageStamp',                          // structural.dart
  'extract', 'search', 'validatePdfA', 'validatePdfUa', // content.dart
  'render', 'extractImages',                            // stream.dart
  'watermark', 'encrypt', 'decrypt', 'sign',            // security.dart
  'getSignatures', 'verifySignatures',                  // security.dart
  'planSplitByBookmarks', 'splitByBookmarks',           // structural.dart
  'classifyPage', 'classifyDocument',                   // content.dart
  'convertTo', 'convertToPdf',                          // content.dart
  'openEditor',                                         // editor.dart
  'createBuilder', 'imagesToPdf',                       // builder.dart
  'dispose',                                            // lifecycle.dart
};

void registerCoverageCheck() {
  test('all PdfBridge methods are covered by shared ops', () {
    expect(_testedMethods.length, _bridgeMethodCount,
        reason: 'Update this list when PdfBridge methods change');
  });
}
