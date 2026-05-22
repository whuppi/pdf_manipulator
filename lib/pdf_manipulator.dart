/// Cross-platform PDF manipulation.
///
/// Powered by pdf_oxide (Rust, MIT/Apache-2.0). Zero main-thread blocking
/// on every platform including web. No `dart:io`. No `dart:ffi`.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
///
/// final pdf = Pdf();
/// final doc = await pdf.open(source);
/// print('${doc.pageCount} pages');
///
/// await pdf.merge([sourceA, sourceB], outputSink);
/// final text = await pdf.extract(source, pages: PdfPages.all());
///
/// await for (final page in pdf.render(source, pages: PdfPages.single(0))) {
///   // process one page at a time
/// }
///
/// await pdf.dispose();
/// ```
library;

// Layer 1 — public API
export 'src/api/pdf.dart';
export 'src/api/pdf_builder.dart';
export 'src/api/pdf_editor.dart';
export 'src/api/pdf_sink.dart';
export 'src/api/pdf_source.dart';
export 'src/api/types/pdf_config.dart';
export 'src/api/types/pdf_enums.dart';
export 'src/api/types/pdf_errors.dart';
export 'src/api/types/pdf_pages.dart';
export 'src/api/types/pdf_params.dart';

// Shared data types (used by bridge, exposed to consumers)
export 'src/core/errors.dart';
export 'src/core/pdf_image.dart';
export 'src/core/pdf_rect.dart';
export 'src/core/pdf_signature.dart';
export 'src/core/search_result.dart';
export 'src/document/pdf_doc.dart';
export 'src/page/pdf_page_info.dart';
