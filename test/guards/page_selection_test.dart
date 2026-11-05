// R2, R3 and the page half of N4. Several pages are PdfPages, an order is
// PdfPageOrder, one page is `int page`. A method whose only parameter is the
// selection takes it positional; every other method takes it as a named
// `pages` that defaults to `const PdfPages.all()` and is never required.
// Parameter names follow the type: `pages`, `order`, `page`.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';

/// Problems with the page-selection parameters of [sigs].
List<String> selectionProblems(List<Sig> sigs) {
  final problems = <String>[];
  for (final s in sigs) {
    if (s.kind == 'getter' || s.kind == 'field') continue;
    for (final p in s.params) {
      final where = '${s.label}(${p.name})';
      if (p.type == 'PdfPages?' ||
          p.type == 'List<int>' ||
          p.type == 'List<int>?') {
        problems.add('$where: ${p.type} is not a page selection');
      }
      if (p.type == 'int?' && p.name == 'page') {
        problems.add('$where: a nullable page');
      }
      if (p.type == 'PdfPages') {
        if (p.name != 'pages') {
          problems.add('$where: a PdfPages is named pages');
        }
        final only = s.params.length == 1;
        if (!only && !p.isNamed && s.kind == 'method') {
          problems.add('$where: with other parameters the selection is named');
        }
        if (p.isNamed && p.isRequired) problems.add('$where: never required');
        if (p.isNamed && p.defaultValue != 'const PdfPages.all()') {
          problems.add(
            '$where: default is const PdfPages.all(), not ${p.defaultValue}',
          );
        }
      }
      if (p.type == 'PdfPageOrder' && p.name != 'order') {
        problems.add('$where: a PdfPageOrder is named order');
      }
      if (p.type == 'int' &&
          p.name.toLowerCase().endsWith('page') &&
          p.name != 'page') {
        // from/to of movePage and the like are indices of two pages.
        if (!const {'from', 'to'}.contains(p.name)) {
          problems.add('$where: an int page is named page');
        }
      }
    }
  }
  return problems;
}

void main() {
  late List<Sig> sigs;
  setUpAll(() async => sigs = signaturesOf(await ApiModel.load()));

  test('page selections follow R2, R3 and N4', () {
    expect(
      selectionProblems([
        for (final s in sigs)
          if (s.kind != 'constructor') s,
      ]),
      isEmpty,
    );
  });

  group('the wall sees a violation', () {
    test('a nullable selection', () {
      expect(
        selectionProblems(const [
          Sig(
            'X',
            'f',
            'method',
            params: [Par('pages', 'PdfPages?', isNamed: true)],
          ),
        ]),
        isNotEmpty,
      );
    });

    test('a list of ints as pages', () {
      expect(
        selectionProblems(const [
          Sig('X', 'f', 'method', params: [Par('pages', 'List<int>')]),
        ]),
        isNotEmpty,
      );
    });

    test('a required or wrongly defaulted named selection', () {
      expect(
        selectionProblems(const [
          Sig(
            'X',
            'f',
            'method',
            params: [
              Par('by', 'int', isNamed: true),
              Par('pages', 'PdfPages', isNamed: true, isRequired: true),
            ],
          ),
          Sig(
            'X',
            'g',
            'method',
            params: [
              Par('by', 'int', isNamed: true),
              Par(
                'pages',
                'PdfPages',
                isNamed: true,
                defaultValue: 'const PdfPages.single(0)',
              ),
            ],
          ),
        ]),
        hasLength(3),
      );
    });

    test(
      'a selection that is positional beside other parameters, a selection named other than pages',
      () {
        expect(
          selectionProblems(const [
            Sig(
              'X',
              'f',
              'method',
              params: [
                Par('by', 'int', isNamed: true),
                Par('pages', 'PdfPages'),
              ],
            ),
            Sig('X', 'g', 'method', params: [Par('which', 'PdfPages')]),
          ]),
          hasLength(2),
        );
      },
    );
  });
}
