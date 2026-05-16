import 'package:meta/meta.dart';

/// Quick probe result — validates structure without full parse.
@immutable
class PdfInfo {
  final bool isValid;
  final int? pageCount;
  final bool isEncrypted;
  final bool requiresPassword;
  final bool isTagged;
  final String? version;

  const PdfInfo({
    required this.isValid,
    this.pageCount,
    this.isEncrypted = false,
    this.requiresPassword = false,
    this.isTagged = false,
    this.version,
  });
}
