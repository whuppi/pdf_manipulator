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

// Public API
export 'src/ops/pdf.dart';
export 'src/ops/pdf_editor.dart';
export 'src/ops/pdf_builder.dart';

// Types
export 'src/types/errors.dart';
export 'src/types/pdf_config.dart';
export 'src/types/pdf_doc.dart';
export 'src/types/pdf_enums.dart';
export 'src/types/pdf_image.dart';
export 'src/types/pdf_page_info.dart';
export 'src/types/pdf_pages.dart';
export 'src/types/pdf_params.dart';
export 'src/types/pdf_rect.dart';
export 'src/types/pdf_signature.dart';
export 'src/types/pdf_sink.dart';
export 'src/types/pdf_source.dart';
export 'src/types/search_result.dart';
