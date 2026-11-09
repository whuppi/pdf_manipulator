// N1: every exported type starts with `Pdf`, except the PDF-agnostic I/O
// family.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

/// The I/O family that carries no PDF in its name.
const ioFamily = <String>{
  'DataSource',
  'DataSink',
  'MemorySource',
  'MemorySink',
  'FileSource',
  'FileSink',
};

/// Names in [exported] that break N1.
List<String> prefixProblems(Iterable<String> exported) => [
  for (final name in exported)
    if (!name.startsWith('Pdf') && !ioFamily.contains(name)) name,
];

void main() {
  test('every exported name starts with Pdf or is in the I/O family', () async {
    final model = await ApiModel.load();
    expect(prefixProblems(model.all.keys), isEmpty);
  });

  test('the wall sees a name without the prefix', () {
    expect(prefixProblems(['Pdf', 'Document', 'MemorySink']), ['Document']);
  });
}
