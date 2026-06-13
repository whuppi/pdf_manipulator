import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Configuration for a PDF engine instance.
class PdfConfig {
  /// Creates a PDF configuration.
  const PdfConfig({this.webLaneWorkerUrl, this.webIoMode, this.maxLanes});

  /// Custom lane worker JS URL for web. Ignored on native.
  final String? webLaneWorkerUrl;

  /// Force a specific web I/O mode. Ignored on native. Null = auto-detect.
  ///
  /// Auto-detection priority (best first):
  ///   1. [PdfIoMode.jspi] — if `WebAssembly.Suspending` is available
  ///   2. [PdfIoMode.atomics] — if `SharedArrayBuffer` is available
  ///   3. [PdfIoMode.opfs] — universal fallback
  final PdfIoMode? webIoMode;

  /// Maximum concurrent lanes (isolated execution units) for this
  /// instance. Null = `max(2, cores / 2)`. A process-wide budget
  /// below this cap queues work instead of failing — exceeding any
  /// cap never errors.
  final int? maxLanes;
}
