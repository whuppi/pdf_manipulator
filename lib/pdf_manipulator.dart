/// Cross-platform PDF manipulation.
///
/// Powered by pdf_oxide (Rust, MIT/Apache-2.0). Zero main-thread blocking
/// on every platform including web. No `dart:io`. No `dart:ffi`.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
///
/// // One-shot operations
/// final pdf = Pdf();
/// final merged = await pdf.merge([bytesA, bytesB]);
/// pdf.dispose();
///
/// // Batch editing
/// final editor = await Pdf.edit(bytes);
/// await editor.setTitle('Report');
/// final result = await editor.save();
/// editor.dispose();
///
/// // Create from scratch
/// final builder = await Pdf.build();
/// final page = await builder.addA4Page();
/// await page.text('Hello');
/// await page.done();
/// final result = await builder.save();
/// builder.dispose();
/// ```
library;

export 'src/core/errors.dart';
export 'src/core/pdf_image.dart';
export 'src/core/pdf_info.dart';
export 'src/core/pdf_rect.dart';
export 'src/core/pdf_signature.dart';
export 'src/core/pdf_sink.dart';
export 'src/core/pdf_source.dart';
export 'src/core/search_result.dart';
export 'src/document/pdf.dart';
export 'src/document/pdf_doc.dart';
export 'src/editor/pdf_editor.dart';
export 'src/builder/pdf_builder.dart';
export 'src/page/pdf_page_info.dart';
