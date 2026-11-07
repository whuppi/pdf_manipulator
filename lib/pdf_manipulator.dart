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
/// final doc = await pdf.open(PdfInput(source));
/// print('${doc.pageCount} pages');
/// final text = await doc.text();
/// final hits = await doc.search('hello');
/// await doc.dispose();
///
/// // One-call operations
/// await pdf.merge([PdfInput(sourceA), PdfInput(sourceB)], outputSink);
/// await pdf.sign(PdfInput(source), outputSink, credentials: creds);
///
/// await pdf.dispose();
/// ```
library;

// Public API
export 'src/ops/pdf.dart';

// Types
export 'src/types/data_sink.dart';
export 'src/types/data_source.dart' show DataSource;
export 'src/types/errors.dart';
export 'src/types/pdf_annotation.dart';
export 'src/types/pdf_attachment.dart';
export 'src/types/pdf_bitmap.dart';
export 'src/types/pdf_config.dart';
export 'src/types/pdf_document_info.dart';
export 'src/types/pdf_enums.dart';
export 'src/types/pdf_field_scripts.dart';
export 'src/types/pdf_form_field.dart';
export 'src/types/pdf_image_info.dart';
export 'src/types/pdf_image_page.dart' show PdfImagePage, PdfImagePageSize;
export 'src/types/pdf_image_placement.dart';
export 'src/types/pdf_image_policy.dart';
export 'src/types/pdf_image_report.dart';
export 'src/types/pdf_input.dart' show PdfInput;
export 'src/types/pdf_link.dart' show PdfLink, PdfMarkup;
export 'src/types/pdf_matrix.dart';
export 'src/types/pdf_metadata.dart';
export 'src/types/pdf_page_info.dart';
export 'src/types/pdf_page_size.dart' show PdfPageSize;
export 'src/types/pdf_pages.dart' show PdfPageOrder, PdfPages;
export 'src/types/pdf_params.dart'
    hide
        PdfEncryptionKeep,
        PdfEncryptionPassword,
        PdfEncryptionRemove,
        PdfPemCredentials,
        PdfPkcs12Credentials,
        PdfSaveFull,
        PdfSaveIncremental,
        PdfSaveIncrementalOrFull,
        PdfWatermarkCenter,
        PdfWatermarkCorner,
        PdfWatermarkExact,
        PdfWatermarkTiled;
export 'src/types/pdf_point.dart';
export 'src/types/pdf_radio_option.dart';
export 'src/types/pdf_rect.dart';
export 'src/types/pdf_redaction_overlay.dart' show PdfRedactionOverlay;
export 'src/types/pdf_redaction_report.dart';
export 'src/types/pdf_render.dart'
    show PdfRenderFormat, PdfRenderSize, PdfRenderedPage;
export 'src/types/pdf_search_hit.dart';
export 'src/types/pdf_signature.dart';
export 'src/types/pdf_split.dart' show PdfSplitPart, PdfSplitRule;
export 'src/types/pdf_stroke.dart';
export 'src/types/pdf_table.dart';
export 'src/types/pdf_task.dart' show PdfTask;
export 'src/types/pdf_text_style.dart';
export 'src/types/pdf_xfa_info.dart';

// Ready-made sources & sinks. Memory-backed helpers ship here (pure
// `dart:typed_data`, web-safe). File-backed helpers need `dart:io`, so
// they live in the separate `package:pdf_manipulator/io.dart` entry point
// to keep this library `dart:io`-free.
export 'src/io/memory_sink.dart';
export 'src/io/memory_source.dart';
