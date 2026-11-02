// The metadata of a document.

/// A document's metadata: the fields the info dictionary carries.
///
/// Every field is optional: a null field is left out of the document.
final class PdfMetadata {
  /// Creates a metadata value.
  const PdfMetadata({
    this.title,
    this.author,
    this.subject,
    this.keywords,
    this.creator,
    this.producer,
    this.creationDate,
    this.modDate,
  });

  /// The document's title.
  final String? title;

  /// The document's author.
  final String? author;

  /// The document's subject.
  final String? subject;

  /// The document's keywords.
  final String? keywords;

  /// The application that created the document.
  final String? creator;

  /// The application that produced the document.
  final String? producer;

  /// When the document was created.
  final DateTime? creationDate;

  /// When the document was last modified.
  final DateTime? modDate;

  /// A copy with the given fields replaced. A field left out keeps its
  /// value; to remove one, build a new [PdfMetadata] without it.
  PdfMetadata copyWith({
    String? title,
    String? author,
    String? subject,
    String? keywords,
    String? creator,
    String? producer,
    DateTime? creationDate,
    DateTime? modDate,
  }) => PdfMetadata(
    title: title ?? this.title,
    author: author ?? this.author,
    subject: subject ?? this.subject,
    keywords: keywords ?? this.keywords,
    creator: creator ?? this.creator,
    producer: producer ?? this.producer,
    creationDate: creationDate ?? this.creationDate,
    modDate: modDate ?? this.modDate,
  );

  @override
  bool operator ==(Object other) =>
      other is PdfMetadata &&
      other.title == title &&
      other.author == author &&
      other.subject == subject &&
      other.keywords == keywords &&
      other.creator == creator &&
      other.producer == producer &&
      other.creationDate == creationDate &&
      other.modDate == modDate;

  @override
  int get hashCode => Object.hash(
    title,
    author,
    subject,
    keywords,
    creator,
    producer,
    creationDate,
    modDate,
  );

  @override
  String toString() =>
      'PdfMetadata(title: $title, author: $author, subject: $subject, '
      'keywords: $keywords, creator: $creator, producer: $producer, '
      'creationDate: $creationDate, modDate: $modDate)';
}
