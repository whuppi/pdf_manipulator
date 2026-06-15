/// Cross-platform PDF manipulation.
///
/// Powered by pdf_oxide (Rust, MIT/Apache-2.0). Zero main-thread blocking
/// on every platform including web. No `dart:io`. No `dart:ffi`.
///
/// ```dart
/// import 'package:pdf_manipulator/pdf_manipulator.dart';
///
/// final pdf = Pdf();
///
/// // Open once, query many times, dispose when done
/// final doc = await pdf.open(source);
/// print('${doc.pageCount} pages');
/// final text = await doc.extract(pages: PdfPages.all());
/// final hits = await doc.search(query: 'hello', pages: PdfPages.all());
/// await doc.dispose();
///
/// // One-shot operations
/// await pdf.merge([sourceA, sourceB], outputSink);
/// await pdf.sign(source, outputSink, credentials: creds);
///
/// await pdf.dispose();
/// ```
library;

// Public API
export 'src/ops/pdf.dart';
export 'src/ops/pdf_doc.dart';
export 'src/ops/pdf_editor.dart';
export 'src/ops/pdf_builder.dart';
export 'src/ops/pdf_standalone.dart';
export 'src/ops/pdf_sugar.dart';

// Types
export 'src/types/errors.dart';
export 'src/types/pdf_config.dart';
export 'src/types/pdf_enums.dart';
export 'src/types/pdf_image.dart';
export 'src/types/pdf_page_info.dart';
export 'src/types/pdf_pages.dart';
export 'src/types/pdf_params.dart';
export 'src/types/pdf_rect.dart';
export 'src/types/pdf_signature.dart';
export 'src/types/data_sink.dart';
export 'src/types/data_source.dart';
export 'src/types/search_result.dart';
export 'src/types/pdf_task.dart' show PdfTask;

// Ready-made sources & sinks. Memory-backed helpers ship here (pure
// `dart:typed_data`, web-safe). File-backed helpers need `dart:io`, so
// they live in the separate `package:pdf_manipulator/io.dart` entry point
// to keep this library `dart:io`-free.
export 'src/io/memory_sink.dart';
export 'src/io/memory_source.dart';
