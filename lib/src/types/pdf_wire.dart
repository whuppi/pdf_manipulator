/// An enum that crosses the engine boundary by name.
///
/// Every public enum implements it. The name is stable: renaming a Dart
/// value never changes it, and the engine reads and writes the same string.
/// The codec accepts only a [PdfWire], never an enum's index.
abstract interface class PdfWire {
  /// The name the engine reads and writes for this value.
  String get wire;
}
