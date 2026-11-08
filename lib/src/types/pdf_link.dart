import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// Where a link over text goes. `PdfPageBuilder.text` and `appendText` take
/// one and anchor it on the text they write.
sealed class PdfLink {
  const PdfLink();

  /// An external hyperlink to [url].
  const factory PdfLink.url(String url) = PdfLinkUrl;

  /// An internal link to the page at [page] (0-based).
  const factory PdfLink.page(int page) = PdfLinkPage;

  /// A link to the named [destination].
  const factory PdfLink.named(String destination) = PdfLinkNamed;

  /// A link that runs the JavaScript [script].
  const factory PdfLink.javaScript(String script) = PdfLinkJavaScript;
}

/// [PdfLink.url].
@immutable
final class PdfLinkUrl extends PdfLink {
  /// Creates a link to [url].
  const PdfLinkUrl(this.url);

  /// The address the link opens.
  final String url;

  @override
  bool operator ==(Object other) => other is PdfLinkUrl && other.url == url;

  @override
  int get hashCode => Object.hash(PdfLinkUrl, url);
}

/// [PdfLink.page].
@immutable
final class PdfLinkPage extends PdfLink {
  /// Creates a link to [page].
  const PdfLinkPage(this.page);

  /// Zero-based index of the page the link goes to.
  final int page;

  @override
  bool operator ==(Object other) => other is PdfLinkPage && other.page == page;

  @override
  int get hashCode => Object.hash(PdfLinkPage, page);
}

/// [PdfLink.named].
@immutable
final class PdfLinkNamed extends PdfLink {
  /// Creates a link to [destination].
  const PdfLinkNamed(this.destination);

  /// The named destination the link goes to.
  final String destination;

  @override
  bool operator ==(Object other) =>
      other is PdfLinkNamed && other.destination == destination;

  @override
  int get hashCode => Object.hash(PdfLinkNamed, destination);
}

/// [PdfLink.javaScript].
@immutable
final class PdfLinkJavaScript extends PdfLink {
  /// Creates a link that runs [script].
  const PdfLinkJavaScript(this.script);

  /// The JavaScript the link runs.
  final String script;

  @override
  bool operator ==(Object other) =>
      other is PdfLinkJavaScript && other.script == script;

  @override
  int get hashCode => Object.hash(PdfLinkJavaScript, script);
}

/// A text markup annotation drawn over the text a `PdfPageBuilder.text` call
/// writes: its [kind] and its [color], or the kind's own colour when
/// [color] is null.
@immutable
final class PdfMarkup {
  /// Creates a markup of [kind] in [color].
  const PdfMarkup(this.kind, {this.color});

  /// The kind of markup.
  final PdfMarkupKind kind;

  /// The markup's colour, or null for the kind's own: yellow for a
  /// highlight, black for the rest.
  final PdfColor? color;

  @override
  bool operator ==(Object other) =>
      other is PdfMarkup && other.kind == kind && other.color == color;

  @override
  int get hashCode => Object.hash(kind, color);
}
