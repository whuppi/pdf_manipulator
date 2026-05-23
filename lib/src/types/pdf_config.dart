/// Configuration for a Pdf instance.
class PdfConfig {
  /// Custom web worker URL. Ignored on native.
  final String? webWorkerUrl;
  const PdfConfig({this.webWorkerUrl});
}
