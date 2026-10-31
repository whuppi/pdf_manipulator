// One-shot membership. Every public member of PdfEditor is declared on
// PdfEdits and every public member of PdfDoc on PdfReads, so a one-shot
// exists for it by construction, or it is listed in one_shot_exclusions.dart
// with its reason. A subclass that declares a public member of its own is
// one-shot-less by accident; this wall turns that into a failing test.
//
// VM-only: it reflects over the classes with dart:mirrors.

@TestOn('vm')
library;

import 'dart:mirrors';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import 'one_shot_exclusions.dart';

/// Public members a class declares itself, constructors left out.
Set<String> declaredPublic(Type type) => {
  for (final d in reflectClass(type).declarations.values)
    if (!(d is MethodMirror && d.isConstructor) &&
        !d.isPrivate &&
        !(d is MethodMirror && d.isStatic))
      MirrorSystem.getName(d.simpleName).replaceAll('=', ''),
};

/// Members of [type] that are neither on its base class nor excluded.
Set<String> unaccounted(Type type, Map<String, String> excluded) =>
    declaredPublic(type).difference(excluded.keys.toSet());

void main() {
  for (final (type, base, name) in [
    (PdfEditor, PdfEdits, 'PdfEditor'),
    (PdfDoc, PdfReads, 'PdfDoc'),
  ]) {
    group(name, () {
      final excluded = oneShotExclusions[name]!;

      test('declares no public member that is not excluded with a reason', () {
        expect(
          unaccounted(type, excluded),
          isEmpty,
          reason:
              'declare it on ${base.toString()} so its one-shot exists, or '
              'list it in one_shot_exclusions.dart with the reason',
        );
      });

      test('every exclusion names a member that exists and has a reason', () {
        final declared = declaredPublic(type);
        for (final MapEntry(key: member, value: reason) in excluded.entries) {
          expect(declared, contains(member), reason: 'stale exclusion');
          expect(reason.trim(), isNotEmpty);
        }
      });

      test('an excluded member is not also on the base class', () {
        final onBase = declaredPublic(base);
        expect(onBase.intersection(excluded.keys.toSet()), isEmpty);
      });
    });
  }

  test('the wall sees a member that is on neither list', () {
    expect(unaccounted(PdfEditor, const {}), isNotEmpty);
    expect(unaccounted(PdfDoc, const {}), isNotEmpty);
  });

  test('the base classes carry the first members', () {
    expect(
      declaredPublic(PdfEdits),
      containsAll(['addWatermark', 'rotatePage']),
    );
    expect(declaredPublic(PdfReads), containsAll(['search', 'render']));
  });
}
