// Names the naming walls let through, each with the reason. A row that no
// longer matches a member fails the wall that reads it.

/// `Owner.member` of a method returning a list that is not plural, to the
/// reason (N4). `pageInfo` is the design's own exception.
const pluralExceptions = <String, String>{
  'PdfReads.pageInfo':
      'returns a list under a singular name; pageInfos is worse and pages '
      'collides with its own parameter',
  'PdfEditor.pageInfo': 'the same fact as PdfReads.pageInfo, on the editor',
  'PdfReads.search': 'a verb: its list is the hits the search found',
  'PdfReads.render': 'a verb: its list is the pages the render wrote',
  'PdfReads.extractImages': 'a verb: its list is the images it wrote',
  'Pdf.split': 'a verb: its list is the parts the split wrote',
};

/// Owners whose task names are the content they write, as the elements of a
/// page are named in a document, not verbs (N2). Their `set*`, `move*` and
/// `append*` names still follow N3.
const contentNounOwners = <String, String>{
  'PdfPageBuilder':
      'a page builder call names the element it writes: text, image, table',
};

/// `Owner.member` of a task whose name is not a verb because N10 names it, to
/// the reason (N2).
const verbExceptions = <String, String>{
  'Pdf.imagesToPdf': 'the `<Thing>ToPdf` form N10 allows',
};

/// `Owner.member` of a `set*` that takes more than one value, to the reason
/// (N3).
const setExceptions = <String, String>{
  'PdfPageBuilder.setFont': 'one property, set by two values: name and size',
  'PdfEdits.setImageBounds': 'names its target, then the value',
};

/// Public bool names that are imperatives of a save or policy knob, to the
/// reason (N8).
const boolVerbs = <String, String>{
  'garbageCollect': 'a save knob',
  'useObjectStreams': 'a save knob',
  'mergeDuplicates': 'a save knob',
  'pruneResources': 'a save knob',
  'stripMetadata': 'a sanitize knob',
  'stripJavaScript': 'a sanitize knob',
  'stripAttachments': 'a sanitize knob',
  'deflate': 'a save knob',
  'allowLossy': 'a policy knob of PdfImagePolicy',
  'convertCmykToRgb': 'a policy knob of PdfImagePolicy',
  'recompressJpeg': 'a policy knob of PdfImagePolicy',
  'bold': 'a style flag of PdfTextStyle, as in CSS',
  'italic': 'a style flag of PdfTextStyle, as in CSS',
  'header': 'a table option, as in HTML',
  'repeatHeader': 'a table option',
};

/// Exported names ending `Options`, to the reason (N5).
const optionsNames = <String, String>{
  'PdfTableOptions':
      'the table types were not judged by the naming pass and the design '
      'lists them as unchanged',
};
