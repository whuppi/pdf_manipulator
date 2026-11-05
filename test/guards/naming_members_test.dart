// N2, N3, N4 (the List half), N10 and N17: how a member is named.
//
// - A task is a verb phrase; a value-returning task with no side effect is a
//   noun; a sync fact is a field or getter, never a PdfTask. No method starts
//   with `get`.
// - `add` puts a new element in, `set` replaces a property, `remove` takes an
//   element out and `delete` never, `clear` empties and takes no argument,
//   `mark` and `apply` exist only as the redaction pair.
// - A method returning a List is plural, or in pluralExceptions.
// - `To` names only where listed, `as` never, `to*` only toString.
// - On a value type the only verbs are the `take*` pair and `cancel`.
//
// io-exempt: the analyzer reads lib from disk.

@TestOn('vm')
library;

import 'package:test/test.dart';

import 'api_model.dart';
import 'naming_exceptions.dart';
import 'verbs.dart';

/// The only names that contain `To` followed by a capital.
const toNames = <String>{
  'convertToOffice',
  'convertToPdfA',
  'convertXfaToAcroForm',
  'imagesToPdf',
};

/// The I/O family and the task type: not value types.
const notValueTypes = <String>{
  'DataSink',
  'DataSource',
  'MemorySink',
  'MemorySource',
  'FileSink',
  'FileSource',
  'PdfTask',
};

/// Problems with member names in [sigs]; [valueTypes] names the value types.
List<String> memberProblems(List<Sig> sigs, Set<String> valueTypes) {
  final problems = <String>[];
  for (final s in sigs) {
    if (s.kind == 'constructor') continue;
    final name = s.name;
    final isTask = s.type.startsWith('PdfTask<');
    if (s.kind == 'getter' && isTask) {
      problems.add('${s.label}: a sync fact is never a PdfTask');
    }
    if (s.kind == 'method') {
      if (name == 'toString' || name == 'hashCode') continue;
      if (name.startsWith('get')) {
        problems.add('${s.label}: no method starts with get');
      }
      final void_ = s.type == 'PdfTask<void>';
      final report = RegExp(r'^PdfTask<Pdf\w+Report>$').hasMatch(s.type);
      if ((void_ || report) &&
          !contentNounOwners.containsKey(s.owner) &&
          !verbExceptions.containsKey(s.label) &&
          !startsWithVerb(name)) {
        problems.add('${s.label}: a task starts with a verb');
      }
      if (name.startsWith('delete')) {
        problems.add('${s.label}: remove, never delete');
      }
      if (name.startsWith('clear') && s.params.isNotEmpty) {
        problems.add('${s.label}: clear takes no argument');
      }
      if (name.startsWith('set') &&
          name != 'set' &&
          (s.params.isEmpty ||
              (s.params.where((p) => !p.isNamed).length > 1 &&
                  !setExceptions.containsKey(s.label)))) {
        problems.add('${s.label}: set takes the new value');
      }
      if ((name.startsWith('mark') || name.startsWith('apply')) &&
          !const {'markRedaction', 'applyRedactions'}.contains(name)) {
        problems.add('${s.label}: mark and apply are the redaction pair');
      }
      if (s.owner != 'PdfTask' && RegExp(r'^as[A-Z]').hasMatch(name)) {
        problems.add('${s.label}: no as names');
      }
      if (RegExp(r'^to[A-Z]').hasMatch(name)) {
        problems.add('${s.label}: to names are toString only');
      }
      if (RegExp(r'To[A-Z]').hasMatch(name) && !toNames.contains(name)) {
        problems.add('${s.label}: a To name outside the list');
      }
      if (RegExp(r'^PdfTask<List<').hasMatch(s.type) &&
          !name.endsWith('s') &&
          !pluralExceptions.containsKey(s.label)) {
        problems.add('${s.label}: a list is plural');
      }
      if (valueTypes.contains(s.owner) &&
          !const {
            '==',
            'toString',
            'takeBytes',
            'takeString',
            'cancel',
          }.contains(name) &&
          startsWithVerb(name)) {
        problems.add('${s.label}: a value type has no verb');
      }
    }
  }
  return problems;
}

void main() {
  late List<Sig> sigs;
  late Set<String> valueTypes;
  setUpAll(() async {
    final model = await ApiModel.load();
    sigs = signaturesOf(model);
    valueTypes = {
      for (final e in model.main.entries)
        if (e.value.library?.uri.path.contains('/src/types/') ?? false)
          if (!notValueTypes.contains(e.key)) e.key,
    };
  });

  test('member names follow N2, N3, N4, N10 and N17', () {
    expect(memberProblems(sigs, valueTypes), isEmpty);
  });

  test('every exception names a member that exists', () {
    final labels = {for (final s in sigs) s.label};
    final owners = {for (final s in sigs) s.owner};
    for (final key in [
      ...pluralExceptions.keys,
      ...setExceptions.keys,
      ...verbExceptions.keys,
    ]) {
      expect(labels, contains(key), reason: 'stale exception $key');
    }
    for (final key in contentNounOwners.keys) {
      expect(owners, contains(key));
    }
  });

  group('the wall sees a violation', () {
    List<String> check(Sig s) => memberProblems([s], {'V'});

    test('a delete, a get, an as, a to', () {
      for (final name in ['deletePages', 'getText', 'asBytes', 'toBytes']) {
        expect(
          check(Sig('X', name, 'method', type: 'PdfTask<void>')),
          isNotEmpty,
          reason: name,
        );
      }
    });

    test('a noun task, a task getter, a clear with an argument', () {
      expect(
        check(const Sig('X', 'pages', 'method', type: 'PdfTask<void>')),
        isNotEmpty,
      );
      expect(
        check(const Sig('X', 'pages', 'getter', type: 'PdfTask<int>')),
        isNotEmpty,
      );
      expect(
        check(const Sig('X', 'clearAll', 'method', params: [Par('a', 'int')])),
        isNotEmpty,
      );
    });

    test('a singular list and a verb on a value type', () {
      expect(
        check(const Sig('X', 'hit', 'method', type: 'PdfTask<List<A>>')),
        isNotEmpty,
      );
      expect(check(const Sig('V', 'save', 'method')), isNotEmpty);
      expect(check(const Sig('V', 'takeBytes', 'method')), isEmpty);
    });
  });
}
