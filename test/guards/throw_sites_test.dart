// R4: a value type is valid by construction, so no `throw` under lib/src/ops
// guards a fact a type carries. Every throw there is listed in
// throw_exclusions.dart with its count and the reason a type could not have
// refused the value.
//
// io-exempt: the source scan reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';
import 'throw_exclusions.dart';

final _throw = RegExp(r'\bthrow\s+(?:const\s+)?(\w+)');

/// Throws per file and type in [files] (file name to source).
Map<String, Map<String, int>> throwCounts(Map<String, String> files) => {
  for (final MapEntry(key: name, value: raw) in files.entries)
    name:
        {
          for (final m in _throw.allMatches(stripComments(raw))) m.group(1)!: 0,
        }.map(
          (type, _) => MapEntry(
            type,
            _throw
                .allMatches(stripComments(raw))
                .where((m) => m.group(1) == type)
                .length,
          ),
        ),
};

/// Differences between [found] and [listed].
List<String> throwProblems(
  Map<String, Map<String, int>> found,
  Map<String, Map<String, ThrowReason>> listed,
) {
  final problems = <String>[];
  for (final MapEntry(key: file, value: types) in found.entries) {
    for (final MapEntry(key: type, value: count) in types.entries) {
      final entry = listed[file]?[type];
      if (entry == null) {
        problems.add(
          '$file throws $type $count time(s): not in throw_exclusions.dart',
        );
      } else if (entry.count != count) {
        problems.add(
          '$file throws $type $count time(s), the list says ${entry.count}',
        );
      } else if (entry.reason.trim().isEmpty) {
        problems.add('$file $type has no reason');
      }
    }
  }
  for (final MapEntry(key: file, value: types) in listed.entries) {
    for (final type in types.keys) {
      if (found[file]?[type] == null) problems.add('stale entry $file $type');
    }
  }
  return problems;
}

void main() {
  test('every throw under lib/src/ops is listed with a reason', () {
    final files = {
      for (final f in Directory('lib/src/ops').listSync().whereType<File>())
        if (f.path.endsWith('.dart'))
          f.uri.pathSegments.last: f.readAsStringSync(),
    };
    expect(throwProblems(throwCounts(files), throwExclusions), isEmpty);
  });

  group('the wall sees a violation', () {
    test('a new throw', () {
      expect(
        throwProblems(
          throwCounts({'x.dart': "f() => throw ArgumentError('x');\n"}),
          const {},
        ),
        hasLength(1),
      );
    });

    test('a stale entry and a wrong count', () {
      final listed = {
        'x.dart': {'StateError': (count: 2, reason: 'r')},
        'y.dart': {'StateError': (count: 1, reason: 'r')},
      };
      expect(
        throwProblems(
          throwCounts({'x.dart': "f() => throw StateError('x');\n"}),
          listed,
        ),
        hasLength(2),
      );
    });
  });
}
