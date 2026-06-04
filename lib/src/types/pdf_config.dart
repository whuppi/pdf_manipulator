import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Configuration for a PDF engine instance.
class PdfConfig {
  /// Creates a PDF configuration.
  const PdfConfig({
    this.webCoordinatorUrl,
    this.webWorkerUrl,
    this.webIoMode,
  });

  /// Custom coordinator JS URL for web. Ignored on native.
  final String? webCoordinatorUrl;

  /// Custom WASM worker JS URL for web. Ignored on native.
  final String? webWorkerUrl;

  /// Force a specific web I/O mode. Ignored on native. Null = auto-detect.
  ///
  /// Auto-detection priority (best first):
  ///   1. [PdfIoMode.jspi] — if `WebAssembly.Suspending` is available
  ///   2. [PdfIoMode.atomics] — if `SharedArrayBuffer` is available
  ///   3. [PdfIoMode.opfs] — universal fallback
  final PdfIoMode? webIoMode;
}
