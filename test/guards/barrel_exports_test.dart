// The barrels export exactly the approved public names. A `show` list that
// grew, a `hide` that was dropped, or a new `export` line changes what an
// app can import; this wall turns the change into a failing test that prints
// the difference. Edit the lists below in the same change as the barrel.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:test/test.dart';

/// Names `package:pdf_manipulator/pdf_manipulator.dart` exports, grouped by
/// the file that declares them.
const _pdfManipulator = <String>[
  // src/io/memory_sink.dart
  'MemorySink',
  // src/io/memory_source.dart
  'MemorySource',
  // src/ops/pdf.dart
  'Pdf',
  'PdfBuilder',
  'PdfBuilderHandle',
  'PdfDocument',
  'PdfDocumentHandle',
  'PdfEditor',
  'PdfEditorHandle',
  'PdfEdits',
  'PdfFormFieldEditor',
  'PdfPageBuilder',
  'PdfReads',
  // src/types/data_sink.dart
  'DataSink',
  // src/types/data_source.dart
  'DataSource',
  // src/types/errors.dart
  'PdfCancelled',
  'PdfCorrupted',
  'PdfCryptoFailed',
  'PdfEngineFailed',
  'PdfError',
  'PdfIncrementalRefused',
  'PdfIoFailed',
  'PdfNotFound',
  'PdfPageOutOfRange',
  'PdfPasswordRequired',
  'PdfUnsupported',
  'PdfWrongPassword',
  // src/types/pdf_annotation.dart
  'PdfAnnotationInfo',
  'PdfAnnotationSubtype',
  // src/types/pdf_attachment.dart
  'PdfAttachment',
  'PdfAttachmentRelationship',
  // src/types/pdf_config.dart
  'PdfConfig',
  // src/types/pdf_enums.dart
  'PdfALevel',
  'PdfAlignment',
  'PdfOfficeFormat',
  'PdfDocumentKind',
  'PdfEncryptionAlgorithm',
  'PdfExtractionFormat',
  'PdfFallbackFontKind',
  'PdfImageFit',
  'PdfIoMode',
  'PdfMarkupKind',
  'PdfPageKind',
  'PdfRotation',
  'PdfStamp',
  'PdfUaLevel',
  // src/types/pdf_bitmap.dart
  'PdfBitmap',
  // src/types/pdf_field_scripts.dart
  'PdfFieldScripts',
  // src/types/pdf_form_field.dart
  'PdfCheckedValue',
  'PdfChoiceValue',
  'PdfFormDataFormat',
  'PdfFormField',
  'PdfFormFieldFlag',
  'PdfFormFieldKind',
  'PdfFormFieldValue',
  'PdfMultiChoiceValue',
  'PdfNoValue',
  'PdfTextValue',
  // src/types/pdf_image_info.dart
  'PdfImageInfo',
  // src/types/pdf_image_page.dart
  'PdfImagePage',
  'PdfImagePageSize',
  // src/types/pdf_image_policy.dart
  'PdfChromaSubsampling',
  'PdfImagePolicy',
  // src/types/pdf_image_report.dart
  'PdfImageAction',
  'PdfImageColor',
  'PdfImageEncoding',
  'PdfImageKeepReason',
  'PdfImageOutcome',
  'PdfImageReport',
  // src/types/pdf_document_info.dart
  'PdfEncryptionInfo',
  'PdfDocumentInfo',
  // src/types/pdf_input.dart
  'PdfInput',
  // src/types/pdf_link.dart
  'PdfLink',
  'PdfMarkup',
  // src/types/pdf_matrix.dart
  'PdfMatrix',
  // src/types/pdf_metadata.dart
  'PdfMetadata',
  // src/types/pdf_image_placement.dart
  'PdfImagePlacement',
  // src/types/pdf_page_info.dart
  'PdfPageInfo',
  // src/types/pdf_page_size.dart
  'PdfPageSize',
  // src/types/pdf_pages.dart
  'PdfPageOrder',
  'PdfPages',
  // src/types/pdf_params.dart
  'PdfBookmarkRange',
  'PdfColor',
  'PdfCorner',
  'PdfDeflate',
  'PdfEncryption',
  'PdfPageClassification',
  'PdfPermissions',
  'PdfSave',
  'PdfSigningCredentials',
  'PdfValidationReport',
  'PdfWatermarkLayer',
  'PdfWatermarkPosition',
  'PdfWatermarkStyle',
  // src/types/pdf_point.dart
  'PdfPoint',
  // src/types/pdf_radio_option.dart
  'PdfRadioOption',
  // src/types/pdf_rect.dart
  'PdfRect',
  // src/types/pdf_redaction_overlay.dart
  'PdfRedactionOverlay',
  // src/types/pdf_redaction_report.dart
  'PdfRedactionReport',
  // src/types/pdf_render.dart
  'PdfRenderFormat',
  'PdfRenderSize',
  'PdfRenderedPage',
  // src/types/pdf_search_hit.dart
  'PdfSearchHit',
  // src/types/pdf_signature.dart
  'PdfCertificateInfo',
  'PdfSignatureInfo',
  // src/types/pdf_split.dart
  'PdfSplitPart',
  'PdfSplitRule',
  // src/types/pdf_stroke.dart
  'PdfStroke',
  // src/types/pdf_table.dart
  'PdfTableCell',
  'PdfTableColumn',
  'PdfTableOptions',
  'PdfTableSizing',
  // src/types/pdf_task.dart
  'PdfTask',
  // src/types/pdf_text_style.dart
  'PdfTextStyle',
  // src/types/pdf_xfa_info.dart
  'PdfXfaInfo',
];

/// Names `package:pdf_manipulator/io.dart` exports.
const _io = <String>[
  // src/io/file_sink.dart
  'FileSink',
  // src/io/file_source.dart
  'FileSource',
  // src/io/pdf_file.dart
  'PdfFile',
];

/// The public names [barrel] (a path under lib) exports, sorted.
Future<List<String>> exportedNames(
  AnalysisContextCollection analysis,
  String barrel,
) async {
  final path = '${Directory.current.absolute.path}/lib/$barrel';
  final session = analysis.contextFor(path).currentSession;
  final result = await session.getLibraryByUri(
    'package:pdf_manipulator/$barrel',
  );
  if (result is! LibraryElementResult) {
    fail('the analyzer could not resolve $barrel: $result');
  }
  return result.element.exportNamespace.definedNames2.keys
      .where((name) => !name.startsWith('_'))
      .toList()
    ..sort();
}

/// What is exported and not approved, then what is approved and not
/// exported; empty when they match.
String difference(List<String> actual, List<String> approved) {
  final extra = actual.toSet().difference(approved.toSet()).toList()..sort();
  final missing = approved.toSet().difference(actual.toSet()).toList()..sort();
  return [
    for (final name in extra) '+ $name (exported, not on the list)',
    for (final name in missing) '- $name (on the list, not exported)',
  ].join('\n');
}

void main() {
  late AnalysisContextCollection analysis;

  setUpAll(() {
    analysis = AnalysisContextCollection(
      includedPaths: ['${Directory.current.absolute.path}/lib'],
    );
  });

  for (final (barrel, approved) in [
    ('pdf_manipulator.dart', _pdfManipulator),
    ('io.dart', _io),
  ]) {
    test('$barrel exports exactly the approved names', () async {
      final actual = await exportedNames(analysis, barrel);
      expect(
        difference(actual, approved),
        isEmpty,
        reason:
            'change the list in barrel_exports_test.dart with the barrel, '
            'in the same change',
      );
    });
  }

  test('the lists hold no duplicate', () {
    expect(_pdfManipulator.toSet(), hasLength(_pdfManipulator.length));
    expect(_io.toSet(), hasLength(_io.length));
  });

  group('difference', () {
    test('names an added and a removed symbol', () {
      expect(
        difference(['A', 'B'], ['B', 'C']),
        '+ A (exported, not on the list)\n'
        '- C (on the list, not exported)',
      );
    });

    test('is empty when the lists match', () {
      expect(difference(['A', 'B'], ['B', 'A']), isEmpty);
    });
  });
}
