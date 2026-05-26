/// Configuration for a Pdf instance.
class PdfConfig {
  /// Custom coordinator JS URL for web. Ignored on native.
  final String? webCoordinatorUrl;

  /// Custom WASM worker JS URL for web. Ignored on native.
  final String? webWorkerUrl;

  const PdfConfig({this.webCoordinatorUrl, this.webWorkerUrl});
}
