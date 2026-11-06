import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Configuration for a PDF engine instance.
final class PdfConfig {
  /// Creates a PDF configuration.
  const PdfConfig({this.webWorkerUrl, this.webIoMode, this.concurrency});

  /// Custom worker JS URL for web. Ignored on native.
  final String? webWorkerUrl;

  /// Force a specific web I/O mode. Ignored on native. Null = auto-detect.
  /// A mode the browser cannot run, or the Asyncify mode when the app did not
  /// ship the Asyncify wasm (no entry for it in `pdf_oxide_manifest.json`),
  /// throws [PdfUnsupported] when the instance initializes; there is no silent
  /// fallback.
  ///
  /// Auto-detection priority (best first):
  ///   1. [PdfIoMode.atomics] — if `SharedArrayBuffer` is available
  ///   2. [PdfIoMode.jspi] — if `WebAssembly.Suspending` is available
  ///   3. [PdfIoMode.opfs] — if the origin private file system opens
  ///   4. [PdfIoMode.asyncify] — if the app shipped the Asyncify wasm
  final PdfIoMode? webIoMode;

  /// How many operations run at once for this
  /// instance. Null = `max(2, cores / 2)`. A process-wide budget
  /// below this cap queues work instead of failing — exceeding any
  /// cap never errors.
  final int? concurrency;

  @override
  bool operator ==(Object other) =>
      other is PdfConfig &&
      other.webWorkerUrl == webWorkerUrl &&
      other.webIoMode == webIoMode &&
      other.concurrency == concurrency;

  @override
  int get hashCode => Object.hash(webWorkerUrl, webIoMode, concurrency);
}
