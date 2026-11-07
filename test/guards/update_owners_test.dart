// Every package folder has exactly one update owner. Dependabot owns the pub
// deps of each folder (one `pub` entry per pubspec.yaml), and the reusable
// upgrade-check.yml owns the Flutter SDK pin, which every app folder carries
// in its own .fvmrc at the root's version:
//
//   - every tracked pubspec.yaml outside vendor/ has its folder listed as a
//     `pub` directory in .github/dependabot.yml;
//   - every tracked .fvmrc outside vendor/ names the root .fvmrc's Flutter
//     version.
//
// io-exempt: runs `git ls-files` and reads dependabot.yml and the .fvmrc files
// from disk.

@TestOn('vm')
library;

import 'dart:convert';
import 'dart:io';

import 'package:test/test.dart';
import 'package:yaml/yaml.dart';

List<String> _tracked(String name) {
  final result = Process.runSync('git', ['ls-files', '--', ':(glob)**/$name']);
  expect(result.exitCode, 0, reason: result.stderr as String);
  return (result.stdout as String)
      .split('\n')
      .where((p) => p.isNotEmpty && !p.startsWith('vendor/'))
      .toList();
}

String _folderOf(String path) {
  final slash = path.lastIndexOf('/');
  return slash < 0 ? '/' : '/${path.substring(0, slash)}';
}

String _flutterOf(String path) =>
    (jsonDecode(File(path).readAsStringSync())
            as Map<String, Object?>)['flutter']!
        as String;

void main() {
  test('every package folder has a pub entry in dependabot.yml', () {
    final updates =
        (loadYaml(File('.github/dependabot.yml').readAsStringSync())
                as YamlMap)['updates']
            as YamlList;
    final listed = {
      for (final update in updates)
        if ((update as YamlMap)['package-ecosystem'] == 'pub')
          update['directory'] as String,
    };
    final missing = {
      for (final pubspec in _tracked('pubspec.yaml')) _folderOf(pubspec),
    }.where((folder) => !listed.contains(folder));
    expect(
      missing,
      isEmpty,
      reason:
          'add a `pub` entry with `directory: "<folder>"` to '
          '.github/dependabot.yml for: ${missing.join(', ')}',
    );
  });

  test('every .fvmrc names the root Flutter version', () {
    final root = _flutterOf('.fvmrc');
    final stale = [
      for (final path in _tracked('.fvmrc'))
        if (_flutterOf(path) != root)
          '${_folderOf(path)} (${_flutterOf(path)})',
    ];
    expect(
      stale,
      isEmpty,
      reason: 'set "flutter": "$root" in the .fvmrc of: ${stale.join(', ')}',
    );
  });
}
