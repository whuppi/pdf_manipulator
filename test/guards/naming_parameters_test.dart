// N13: parameter names by role. The document to open is `input` (a second one
// `from`); the DataSink written is `output` (or the factory `sinkFor`); a
// DataSource that is not a PDF is named for its content, never `source`.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

/// Problems with parameter names in [sigs].
List<String> parameterProblems(List<Sig> sigs) => [
  for (final s in sigs)
    if (s.kind != 'constructor' ||
        s.owner == 'PdfInput' ||
        s.owner == 'PdfFile')
      for (final p in s.params)
        if ((p.type == 'PdfInput' &&
                !const {'input', 'from'}.contains(p.name)) ||
            (p.type == 'DataSink' && p.name != 'output') ||
            (p.type.startsWith('FutureOr<DataSink> Function') &&
                p.name != 'sinkFor') ||
            (p.type == 'DataSource' &&
                p.name == 'source' &&
                !s.owner.contains('Source') &&
                s.owner != 'PdfInput'))
          '${s.label}(${p.name}: ${p.type})',
];

void main() {
  test('parameters are named by role', () async {
    expect(parameterProblems(signaturesOf(await ApiModel.load())), isEmpty);
  });

  test('the wall sees a misnamed parameter', () {
    expect(
      parameterProblems(const [
        Sig(
          'X',
          'f',
          'method',
          params: [
            Par('source', 'PdfInput'),
            Par('out', 'DataSink'),
            Par('source', 'DataSource'),
          ],
        ),
      ]),
      hasLength(3),
    );
  });
}
