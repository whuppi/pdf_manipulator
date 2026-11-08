// N15: an enum whose values name the things a builder creates equals that
// builder's method names. PdfFormFieldKind, less `listBox` and `unknown`, is
// the widget methods of PdfPageBuilder.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:analyzer/dart/element/element.dart';
import 'package:test/test.dart';

import 'api_model.dart';

/// Values of the kind enum no builder method creates.
const notCreated = {'listBox', 'unknown'};

void main() {
  test(
    'PdfFormFieldKind pairs with the PdfPageBuilder widget methods',
    () async {
      final model = await ApiModel.load();
      final kind = model.type('PdfFormFieldKind') as EnumElement;
      final values = {
        for (final f in kind.fields)
          if (f.isEnumConstant) f.name!,
      }.difference(notCreated);
      final widgets = {
        for (final m in model.membersOf(model.type('PdfPageBuilder')))
          if (m.isMethod && m.typeName == 'Future<void>' ||
              m.isMethod && m.typeName.startsWith('PdfTask'))
            m.name,
      };
      expect(widgets.intersection(values), values);
    },
  );

  test('the not-created values exist', () async {
    final model = await ApiModel.load();
    final kind = model.type('PdfFormFieldKind') as EnumElement;
    final names = {for (final f in kind.fields) f.name};
    expect(names, containsAll(notCreated));
  });
}
