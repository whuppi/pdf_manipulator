// Every `throw` under lib/src/ops, by file and thrown type, with how many
// there are and the reason a type could not have refused the value instead.
// The throw-site test fails on a throw that is not here and on an entry that
// no longer matches.

/// A count and a reason.
typedef ThrowReason = ({int count, String reason});

const _disposed = 'a disposed handle or Pdf: no value type knows it';
const _regions =
    'a list that must not be empty: List is not a value type of ours';
const _cancel = 'the job ended cancelled: a lifecycle fact, not a value';

/// Throws by file name, then thrown type.
const throwExclusions = <String, Map<String, ThrowReason>>{
  'handle_backstop.dart': {
    'ArgumentError': (
      count: 1,
      reason: 'a test hook given an object that is not registered',
    ),
  },
  'pdf.dart': {
    'PdfCancelled': (count: 1, reason: _cancel),
    'StateError': (count: 1, reason: _disposed),
  },
  'pdf_builder.dart': {
    'StateError': (count: 1, reason: _disposed),
    'ArgumentError': (
      count: 3,
      reason:
          'a table whose columns, rows and options disagree: no one type '
          'holds all three',
    ),
  },
  'pdf_doc.dart': {'StateError': (count: 1, reason: _disposed)},
  'pdf_editor.dart': {
    'StateError': (count: 1, reason: _disposed),
    'ArgumentError': (count: 1, reason: _regions),
  },
  'pdf_edits.dart': {
    'ArgumentError': (count: 1, reason: _regions),
    'PdfNotFound': (
      count: 1,
      reason: 'a name only the document knows: the engine has no kind for it',
    ),
  },
  'pdf_jobs.dart': {
    'ArgumentError': (
      count: 1,
      reason: 'empty inputs: List is not a value type of ours',
    ),
    'PdfCancelled': (count: 1, reason: _cancel),
    'PdfIoFailed': (
      count: 1,
      reason: 'a source that ends before its declared length',
    ),
  },
  'pdf_reads.dart': {
    'PdfNotFound': (
      count: 1,
      reason: 'a name only the document knows: the engine has no kind for it',
    ),
    'StateError': (count: 1, reason: 'a one-shot reused'),
  },
};
