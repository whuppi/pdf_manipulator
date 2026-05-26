// Web wire decoder — JS object Map → typed Dart results.
//
// Symmetric with native/wire.dart which does binary → Map → typed.
// Web receives JS objects that are already Maps, so this file just
// calls codec.dart for the Map → typed step.
//
// Both bridges call wireDecodeXxx() and get typed results back.
// Neither bridge imports codec.dart directly.

import 'package:pdf_manipulator/src/transport/protocol/codec.dart';
import 'package:pdf_manipulator/src/types/pdf_doc.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

PdfDoc wireDecodeOpen(Map<String, Object?> r) => decodeOpenResult(r);

String wireDecodeText(Map<String, Object?> r) => r['text'] as String? ?? '';

List<SearchResult> wireDecodeSearch(Map<String, Object?> r) => decodeSearchResults(r);

List<PdfSignatureInfo> wireDecodeSignatures(Map<String, Object?> r) => decodeSignatures(r);

bool wireDecodeVerifySignatures(Map<String, Object?> r) => r['valid'] as bool? ?? false;

PdfValidationResult wireDecodeValidation(Map<String, Object?> r) => decodeValidationResult(r);

bool wireDecodeValidatePdfUa(Map<String, Object?> r) => r['accessible'] as bool? ?? false;

List<PdfBookmarkSplit> wireDecodeBookmarkSplits(Map<String, Object?> r) => decodeBookmarkSplits(r);

PdfPageClassification wireDecodeClassifyPage(Map<String, Object?> r) => decodeClassifyPage(r);

PdfDocumentClassification wireDecodeClassifyDocument(Map<String, Object?> r) => decodeClassifyDocument(r);

RenderedPage wireDecodeRenderedPage(Map<String, Object?> r) => decodeRenderedPage(r);

PdfImage wireDecodeImage(Map<String, Object?> r) => decodePdfImage(r);

({int pageCount, String version, String title, String author, String subject, String keywords})
    wireDecodeEditorMetadata(Map<String, Object?> r) => decodeEditorMetadata(r);
