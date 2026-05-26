import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// A parsed PDF document — read-only inspection handle.
@immutable
class PdfDoc {
  final int pageCount;
  final String version;
  final List<PdfPageInfo> pages;
  final String? title;
  final String? author;
  final String? subject;
  final String? keywords;
  final bool isEncrypted;
  final bool requiresPassword;
  final bool isTagged;
  final PdfEncryptionAlgorithm? encryptionAlgorithm;
  final PdfPermissions? permissions;

  const PdfDoc({
    required this.pageCount,
    required this.version,
    required this.pages,
    this.title,
    this.author,
    this.subject,
    this.keywords,
    this.isEncrypted = false,
    this.requiresPassword = false,
    this.isTagged = false,
    this.encryptionAlgorithm,
    this.permissions,
  });
}
