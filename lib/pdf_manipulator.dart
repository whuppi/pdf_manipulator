/// Cross-platform PDF manipulation.
///
/// Powered by pdf_oxide (Rust, MIT/Apache-2.0). Zero main-thread blocking
/// on every platform including web. No `dart:io`. No `dart:ffi`.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
///
/// final doc = await Pdf.open(pdfBytes);
/// print('${doc.pageCount} pages');
///
/// final merged = await Pdf.merge([pdfA, pdfB]);
///
/// final editor = await PdfEditor.open(bytes);
/// await editor.setTitle('Updated');
/// await editor.rotatePage(0, degrees: 90);
/// final result = await editor.save();
/// await editor.dispose();
/// ```
library;

// Core types
export 'src/core/errors.dart';
export 'src/core/pdf_image.dart';
export 'src/core/pdf_info.dart';
export 'src/core/pdf_rect.dart';
export 'src/core/pdf_signature.dart';
export 'src/core/search_result.dart';

// Document — static async API
export 'src/document/pdf.dart';
export 'src/document/pdf_doc.dart';

// Editor — mutable editing session
export 'src/editor/pdf_editor.dart';

// Builder — create PDFs from scratch
export 'src/builder/pdf_builder.dart';

// Page
export 'src/page/pdf_page_info.dart';
